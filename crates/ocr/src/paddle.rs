//! PaddleOCR offline Thai text detection and recognition using rten (Apache-2.0).
//!
//! Models:
//! - paddle-text-detection.onnx (PP-OCRv3 mobile DBNet text detection)
//! - paddle-thai-recognition.onnx (PP-OCRv5 mobile Thai SVTR text recognition)
//! - paddle-thai-dict.txt (PP-OCRv5 Thai alphabet dictionary)

use std::path::{Path, PathBuf};
use crate::{Line, OcrError, Word};

pub const DETECTION_MODEL: &str = "paddle-text-detection.onnx";
pub const RECOGNITION_MODEL: &str = "paddle-thai-recognition.onnx";
pub const DICTIONARY: &str = "paddle-thai-dict.txt";

#[derive(Clone, Debug, PartialEq)]
pub struct PaddleModels {
    pub detection: PathBuf,
    pub recognition: PathBuf,
    pub dict: PathBuf,
}

impl PaddleModels {
    pub fn in_dir(dir: &Path) -> Option<PaddleModels> {
        let m = PaddleModels {
            detection: dir.join(DETECTION_MODEL),
            recognition: dir.join(RECOGNITION_MODEL),
            dict: dir.join(DICTIONARY),
        };
        (m.detection.is_file() && m.recognition.is_file() && m.dict.is_file()).then_some(m)
    }

    pub fn find() -> Option<PaddleModels> {
        crate::Models::search_dirs().iter().find_map(|d| Self::in_dir(d))
    }
}

pub struct Paddle {
    pub detection: rten::Model,
    pub recognition: rten::Model,
    pub dict: Vec<String>,
}

impl Paddle {
    pub fn load(models: &PaddleModels) -> Result<Paddle, OcrError> {
        let load = |p: &Path| rten::Model::load_file(p).map_err(|e| OcrError::Load(p.display().to_string(), e.to_string()));
        let detection = load(&models.detection)?;
        let recognition = load(&models.recognition)?;

        let text = std::fs::read_to_string(&models.dict)
            .map_err(|e| OcrError::Load(models.dict.display().to_string(), e.to_string()))?;

        let mut dict = Vec::new();
        // Index 0 is CTC Blank in PaddleOCR.
        dict.push(String::new());
        for line in text.lines() {
            let trimmed = line.trim_end_matches(['\r', '\n']);
            dict.push(trimmed.to_string());
        }
        // Index 525 is space
        dict.push(" ".to_string());

        Ok(Paddle { detection, recognition, dict })
    }

    pub fn find() -> Option<Paddle> {
        let models = PaddleModels::find()?;
        Self::load(&models).ok()
    }

    /// Detect text regions and recognize Thai text offline.
    pub fn recognize(&self, pixels: &[u8], width: u32, height: u32) -> Result<Vec<Line>, OcrError> {
        if width == 0 || height == 0 || pixels.is_empty() {
            return Err(OcrError::EmptyImage);
        }

        let rgb: Vec<u8> = if pixels.len() == (width * height * 4) as usize {
            pixels.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect()
        } else if pixels.len() == (width * height * 3) as usize {
            pixels.to_vec()
        } else {
            return Err(OcrError::Recognize("invalid image dimensions".into()));
        };

        let boxes = self.detect(&rgb, width, height)?;
        if boxes.is_empty() {
            return Ok(Vec::new());
        }

        let mut words = Vec::new();
        for b in boxes {
            let [l, t, r, bot] = b;
            let crop_w = (r - l).max(2.0) as usize;
            let crop_h = (bot - t).max(2.0) as usize;

            if let Some(text) = self.read_box(&rgb, width, height, [l as usize, t as usize, crop_w, crop_h])? {
                if !text.trim().is_empty() {
                    words.push(Word { text, rect: [l, t, r, bot] });
                }
            }
        }

        Ok(group_lines(words))
    }

    /// DBNet text detection: returns bounding boxes `[left, top, right, bottom]`.
    pub fn detect(&self, rgb: &[u8], width: u32, height: u32) -> Result<Vec<[f32; 4]>, OcrError> {
        use rten_tensor::layout::Layout;

        let max_dim = 960.0f32;
        let scale = (max_dim / (width.max(height) as f32)).min(1.0);
        let target_w = (((width as f32 * scale).round() as usize / 32) * 32).max(32);
        let target_h = (((height as f32 * scale).round() as usize / 32) * 32).max(32);

        let scale_x = target_w as f32 / width as f32;
        let scale_y = target_h as f32 / height as f32;

        let mut tensor_data = vec![0.0f32; 3 * target_h * target_w];
        let mean = [0.485f32, 0.456, 0.406];
        let std = [0.229f32, 0.224, 0.225];

        for ty in 0..target_h {
            let sy = ((ty as f32 / scale_y) as usize).min(height as usize - 1);
            for tx in 0..target_w {
                let sx = ((tx as f32 / scale_x) as usize).min(width as usize - 1);
                let src_idx = (sy * width as usize + sx) * 3;
                let r = rgb[src_idx] as f32 / 255.0;
                let g = rgb[src_idx + 1] as f32 / 255.0;
                let b = rgb[src_idx + 2] as f32 / 255.0;

                let channel_size = target_h * target_w;
                let offset = ty * target_w + tx;
                tensor_data[offset] = (r - mean[0]) / std[0];
                tensor_data[channel_size + offset] = (g - mean[1]) / std[1];
                tensor_data[2 * channel_size + offset] = (b - mean[2]) / std[2];
            }
        }

        let input_tensor = rten_tensor::Tensor::from_data(&[1, 3, target_h, target_w], tensor_data);
        let in_id = self.detection.input_ids()[0];
        let out_id = self.detection.output_ids()[0];

        let out = self.detection
            .run(vec![(in_id, (&input_tensor).into())], &[out_id], None)
            .map_err(|e| OcrError::Recognize(format!("text detection inference failed: {e}")))?;

        let view = out[0].as_tensor_view::<f32>().ok_or_else(|| OcrError::Recognize("detection output is not float32".into()))?;
        let shape = view.shape();
        let (out_h, out_w) = (shape[2], shape[3]);

        // Binary threshold at 0.3
        let mut mask = vec![false; out_h * out_w];
        for y in 0..out_h {
            for x in 0..out_w {
                let prob = view[[0, 0, y, x]];
                if prob >= 0.3 {
                    mask[y * out_w + x] = true;
                }
            }
        }

        // Find connected components
        let mut visited = vec![false; out_h * out_w];
        let mut boxes = Vec::new();

        for y in 0..out_h {
            for x in 0..out_w {
                let idx = y * out_w + x;
                if mask[idx] && !visited[idx] {
                    let mut q = vec![(x, y)];
                    visited[idx] = true;
                    let mut min_x = x;
                    let mut max_x = x;
                    let mut min_y = y;
                    let mut max_y = y;
                    let mut count = 0;

                    let mut head = 0;
                    while head < q.len() {
                        let (cx, cy) = q[head];
                        head += 1;
                        count += 1;

                        if cx < min_x { min_x = cx; }
                        if cx > max_x { max_x = cx; }
                        if cy < min_y { min_y = cy; }
                        if cy > max_y { max_y = cy; }

                        for (dx, dy) in [(-1isize, 0), (1, 0), (0, -1), (0, 1)] {
                            let nx = cx as isize + dx;
                            let ny = cy as isize + dy;
                            if nx >= 0 && nx < out_w as isize && ny >= 0 && ny < out_h as isize {
                                let nidx = ny as usize * out_w + nx as usize;
                                if mask[nidx] && !visited[nidx] {
                                    visited[nidx] = true;
                                    q.push((nx as usize, ny as usize));
                                }
                            }
                        }
                    }

                    // Ignore tiny noise components
                    if count >= 16 && (max_x - min_x) >= 3 && (max_y - min_y) >= 3 {
                        // Unclip: expand box slightly by 20%
                        let bw = (max_x - min_x + 1) as f32;
                        let bh = (max_y - min_y + 1) as f32;
                        let pad_x = (bw * 0.15).max(1.0);
                        let pad_y = (bh * 0.15).max(1.0);

                        let orig_l = ((min_x as f32 - pad_x).max(0.0) / scale_x).max(0.0);
                        let orig_t = ((min_y as f32 - pad_y).max(0.0) / scale_y).max(0.0);
                        let orig_r = ((max_x as f32 + pad_x) / scale_x).min(width as f32);
                        let orig_b = ((max_y as f32 + pad_y) / scale_y).min(height as f32);

                        boxes.push([orig_l, orig_t, orig_r, orig_b]);
                    }
                }
            }
        }

        Ok(boxes)
    }

    /// Read cropped box image and decode Thai text via CTC.
    pub fn read_box(&self, rgb: &[u8], img_w: u32, img_h: u32, [bx, by, bw, bh]: [usize; 4]) -> Result<Option<String>, OcrError> {
        use rten_tensor::layout::Layout;

        if bw < 2 || bh < 2 {
            return Ok(None);
        }

        let target_h = 48usize;
        let aspect = bw as f32 / bh as f32;
        let target_w = ((target_h as f32 * aspect).round() as usize).clamp(16, 960);

        let mut tensor_data = vec![0.0f32; 3 * target_h * target_w];

        for ty in 0..target_h {
            let sy = by + ((ty as f32 / target_h as f32) * bh as f32) as usize;
            let sy = sy.min(img_h as usize - 1);
            for tx in 0..target_w {
                let sx = bx + ((tx as f32 / target_w as f32) * bw as f32) as usize;
                let sx = sx.min(img_w as usize - 1);

                let src_idx = (sy * img_w as usize + sx) * 3;
                let r = rgb[src_idx] as f32 / 255.0;
                let g = rgb[src_idx + 1] as f32 / 255.0;
                let b = rgb[src_idx + 2] as f32 / 255.0;

                // PaddleOCR recognition normalization: (x - 0.5) / 0.5 = 2*x - 1
                let channel_size = target_h * target_w;
                let offset = ty * target_w + tx;
                tensor_data[offset] = r * 2.0 - 1.0;
                tensor_data[channel_size + offset] = g * 2.0 - 1.0;
                tensor_data[2 * channel_size + offset] = b * 2.0 - 1.0;
            }
        }

        let input_tensor = rten_tensor::Tensor::from_data(&[1, 3, target_h, target_w], tensor_data);
        let in_id = self.recognition.input_ids()[0];
        let out_id = self.recognition.output_ids()[0];

        let out = self.recognition
            .run(vec![(in_id, (&input_tensor).into())], &[out_id], None)
            .map_err(|e| OcrError::Recognize(format!("text recognition inference failed: {e}")))?;

        let view = out[0].as_tensor_view::<f32>().ok_or_else(|| OcrError::Recognize("recognition output is not float32".into()))?;
        let shape = view.shape();
        let (steps, num_classes) = (shape[1], shape[2]);

        let mut decoded = String::new();
        let mut last_idx = 0usize;

        for step in 0..steps {
            let mut best_idx = 0usize;
            let mut best_score = f32::NEG_INFINITY;

            for c in 0..num_classes {
                let score = view[[0, step, c]];
                if score > best_score {
                    best_score = score;
                    best_idx = c;
                }
            }

            if best_idx != 0 && best_idx != last_idx
                && let Some(ch) = self.dict.get(best_idx) {
                decoded.push_str(ch);
            }
            last_idx = best_idx;
        }

        let trimmed = decoded.trim().to_string();
        if trimmed.is_empty() {
            Ok(None)
        } else {
            Ok(Some(trimmed))
        }
    }
}

/// Group detected words into lines according to reading order.
pub fn group_lines(mut words: Vec<Word>) -> Vec<Line> {
    if words.is_empty() {
        return Vec::new();
    }

    // Sort words top-to-bottom, then left-to-right
    words.sort_by(|a, b| {
        let y_diff = a.rect[1] - b.rect[1];
        if y_diff.abs() > 8.0 {
            a.rect[1].partial_cmp(&b.rect[1]).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            a.rect[0].partial_cmp(&b.rect[0]).unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    let mut lines: Vec<Line> = Vec::new();
    for word in words {
        let mut added = false;
        for line in &mut lines {
            if let Some(first) = line.words.first() {
                let line_mid = (first.rect[1] + first.rect[3]) * 0.5;
                let word_mid = (word.rect[1] + word.rect[3]) * 0.5;
                let line_h = (first.rect[3] - first.rect[1]).abs();

                if (line_mid - word_mid).abs() < (line_h * 0.6).max(8.0) {
                    line.words.push(word.clone());
                    added = true;
                    break;
                }
            }
        }
        if !added {
            lines.push(Line { words: vec![word] });
        }
    }

    // Sort words in each line left-to-right
    for line in &mut lines {
        line.words.sort_by(|a, b| a.rect[0].partial_cmp(&b.rect[0]).unwrap_or(std::cmp::Ordering::Equal));
    }

    lines
}
