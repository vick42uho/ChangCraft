//! SCB 10X Typhoon OCR API integration for Thai printed text and handwriting.
//!
//! Endpoint: https://api.opentyphoon.ai/v1/chat/completions
//! Model: typhoon-ocr
//! Supports printed Thai text, complex forms, tables, and handwritten notes.

use crate::{Line, OcrError, Word};

const TYPHOON_ENDPOINT: &str = "https://api.opentyphoon.ai/v1/chat/completions";
const MODEL_NAME: &str = "typhoon-ocr";

/// Load system root certificates for TLS.
fn os_roots() -> Result<ureq::tls::RootCerts, String> {
    let found = rustls_native_certs::load_native_certs();
    let certs: Vec<ureq::tls::Certificate<'static>> = found.certs.iter().map(|c| ureq::tls::Certificate::from_der(c.as_ref()).to_owned()).collect();
    if certs.is_empty() {
        return Err("no trusted certificates found on this system".into());
    }
    Ok(ureq::tls::RootCerts::new_with_certs(&certs))
}

/// Convert RGBA pixels to JPEG bytes.
pub fn encode_jpeg(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>, OcrError> {
    let rgb: Vec<u8> = if pixels.len() == (width * height * 4) as usize {
        pixels.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect()
    } else if pixels.len() == (width * height * 3) as usize {
        pixels.to_vec()
    } else {
        return Err(OcrError::Recognize("invalid image buffer dimensions".into()));
    };

    let mut jpeg_bytes = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_bytes, 85);
    encoder
        .encode(&rgb, width, height, image::ExtendedColorType::Rgb8)
        .map_err(|e| OcrError::Recognize(format!("failed to encode image as JPEG: {e}")))?;
    Ok(jpeg_bytes)
}

/// Check if Typhoon OCR API key is configured.
pub fn api_key() -> Option<String> {
    std::env::var("TYPHOON_API_KEY")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| std::env::var("PDFCRAFT_TYPHOON_API_KEY").ok().filter(|s| !s.trim().is_empty()))
}

/// Call Typhoon OCR API to recognize Thai text and handwriting on the image.
pub fn recognize_typhoon(pixels: &[u8], width: u32, height: u32, explicit_key: Option<&str>) -> Result<Vec<Line>, OcrError> {
    if width == 0 || height == 0 || pixels.is_empty() {
        return Err(OcrError::EmptyImage);
    }

    let key = explicit_key
        .map(std::borrow::ToOwned::to_owned)
        .or_else(api_key)
        .ok_or_else(|| {
            OcrError::Recognize(
                "Typhoon OCR requires an API key. Please set the environment variable TYPHOON_API_KEY=<your-key> to use Thai OCR and handwriting recognition."
                    .into(),
            )
        })?;

    use base64::Engine as _;
    let jpeg = encode_jpeg(pixels, width, height)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&jpeg);
    let data_url = format!("data:image/jpeg;base64,{b64}");

    let prompt = "Extract all text from this document. Transcribe all Thai and English text, tables, forms, and handwriting line by line without omitting any word.";

    let payload = serde_json::json!({
        "model": MODEL_NAME,
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": prompt
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": data_url
                        }
                    }
                ]
            }
        ],
        "max_tokens": 4096,
        "temperature": 0.1
    });

    let payload_bytes = serde_json::to_vec(&payload).map_err(|e| OcrError::Recognize(e.to_string()))?;

    let roots = os_roots().map_err(|e| OcrError::Recognize(format!("TLS root certificates error: {e}")))?;
    let agent = ureq::Agent::config_builder().tls_config(ureq::tls::TlsConfig::builder().root_certs(roots).build()).build().new_agent();

    let mut resp = agent
        .post(TYPHOON_ENDPOINT)
        .header("Authorization", &format!("Bearer {}", key.trim()))
        .header("Content-Type", "application/json")
        .header("User-Agent", "PdfCraft-OCR/0.11.0")
        .send(&payload_bytes)
        .map_err(|e| OcrError::Recognize(format!("Typhoon OCR API request failed: {e}")))?;

    let status = resp.status().as_u16();
    if status != 200 {
        return Err(OcrError::Recognize(format!("Typhoon OCR API returned HTTP status {status}")));
    }

    let body_text = resp
        .body_mut()
        .with_config()
        .limit(1 << 22)
        .read_to_string()
        .map_err(|e| OcrError::Recognize(format!("failed to read Typhoon OCR response: {e}")))?;

    let body: serde_json::Value =
        serde_json::from_str(&body_text).map_err(|e| OcrError::Recognize(format!("failed to parse Typhoon OCR response JSON: {e}")))?;

    let content =
        body["choices"][0]["message"]["content"].as_str().ok_or_else(|| OcrError::Recognize("Typhoon OCR returned empty response".into()))?;

    // Parse lines and synthesize bounding boxes across the page
    let raw_lines: Vec<&str> = content.lines().map(str::trim).filter(|s| !s.is_empty()).collect();
    if raw_lines.is_empty() {
        return Ok(Vec::new());
    }

    let line_count = raw_lines.len() as f32;
    let margin_x = width as f32 * 0.05;
    let margin_y = height as f32 * 0.05;
    let available_w = width as f32 * 0.90;
    let available_h = height as f32 * 0.90;
    let line_height = (available_h / (line_count + 1.0)).min(32.0).max(12.0);

    let mut out_lines = Vec::new();
    for (idx, line_text) in raw_lines.iter().enumerate() {
        let top = margin_y + (idx as f32 * (available_h / line_count));
        let bottom = top + line_height;

        let words_text: Vec<&str> = line_text.split_whitespace().collect();
        if words_text.is_empty() {
            continue;
        }

        let total_chars: usize = words_text.iter().map(|w| w.chars().count()).sum();
        let total_chars = total_chars.max(1) as f32;
        let mut cur_x = margin_x;

        let mut words = Vec::new();
        for w in &words_text {
            let char_len = w.chars().count() as f32;
            let word_w = (char_len / total_chars) * (available_w * 0.85);
            let right = cur_x + word_w.max(10.0);
            words.push(Word { text: (*w).to_string(), rect: [cur_x, top, right, bottom] });
            cur_x = right + 6.0;
        }

        out_lines.push(Line { words });
    }

    Ok(out_lines)
}
