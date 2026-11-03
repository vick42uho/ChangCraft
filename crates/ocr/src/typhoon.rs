//! SCB 10X Typhoon OCR API integration for Thai printed text and handwriting.
//!
//! Endpoint: https://api.opentyphoon.ai/v1/chat/completions
//! Model: typhoon-ocr
//! Supports printed Thai text, complex forms, tables, and handwritten notes.

use std::path::PathBuf;
use crate::{Line, OcrError, Word};

const TYPHOON_ENDPOINT: &str = "https://api.opentyphoon.ai/v1/chat/completions";
const MODEL_NAME: &str = "typhoon-ocr";

/// Candidate paths where `typhoon.key` might be stored.
pub fn key_candidates() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(exe) = std::env::current_exe().ok().and_then(|e| e.parent().map(std::path::Path::to_path_buf)) {
        dirs.push(exe.join("PdfCraftData").join("typhoon.key"));
        dirs.push(exe.join("typhoon.key"));
    }
    #[cfg(windows)]
    if let Some(appdata) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(appdata).join("PdfCraft").join("typhoon.key"));
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join("Library/Application Support/PdfCraft/typhoon.key"));
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            dirs.push(PathBuf::from(xdg).join("pdfcraft").join("typhoon.key"));
        }
        if let Some(home) = std::env::var_os("HOME") {
            dirs.push(PathBuf::from(home).join(".config/pdfcraft/typhoon.key"));
            dirs.push(PathBuf::from(home).join(".local/share/PdfCraft/typhoon.key"));
        }
    }
    dirs
}

/// Primary storage path for `typhoon.key`.
pub fn default_key_path() -> PathBuf {
    if let Some(exe) = std::env::current_exe().ok().and_then(|e| e.parent().map(std::path::Path::to_path_buf)) {
        let portable = exe.join("PdfCraftData");
        if portable.is_dir() {
            return portable.join("typhoon.key");
        }
    }
    #[cfg(windows)]
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return PathBuf::from(appdata).join("PdfCraft").join("typhoon.key");
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join("Library/Application Support/PdfCraft/typhoon.key");
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg).join("pdfcraft").join("typhoon.key");
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".config/pdfcraft/typhoon.key");
        }
    }
    PathBuf::from("typhoon.key")
}

/// Read the stored API key from `typhoon.key` if present.
pub fn read_stored_key() -> Option<String> {
    for p in key_candidates() {
        if let Ok(content) = std::fs::read_to_string(&p) {
            let key = content.trim();
            if !key.is_empty() {
                return Some(key.to_string());
            }
        }
    }
    None
}

/// Save API key into `typhoon.key`.
pub fn save_stored_key(key: &str) -> std::io::Result<()> {
    let path = default_key_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, key.trim())
}

/// Remove `typhoon.key` from storage.
pub fn remove_stored_key() -> std::io::Result<()> {
    for p in key_candidates() {
        if p.is_file() {
            let _ = std::fs::remove_file(&p);
        }
    }
    let default = default_key_path();
    if default.is_file() {
        let _ = std::fs::remove_file(default);
    }
    Ok(())
}

/// Test whether an OpenTyphoon API key is valid.
pub fn test_key(key: &str) -> Result<(), String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err("API key cannot be empty".into());
    }

    let payload = serde_json::json!({
        "model": MODEL_NAME,
        "messages": [
            {
                "role": "user",
                "content": "ping"
            }
        ],
        "max_tokens": 1
    });

    let payload_bytes = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
    let roots = os_roots().map_err(|e| format!("TLS root certificates error: {e}"))?;
    let agent = ureq::Agent::config_builder().tls_config(ureq::tls::TlsConfig::builder().root_certs(roots).build()).build().new_agent();

    let resp = agent
        .post(TYPHOON_ENDPOINT)
        .header("Authorization", &format!("Bearer {trimmed}"))
        .header("Content-Type", "application/json")
        .header("User-Agent", "PdfCraft-OCR/0.11.0")
        .send(&payload_bytes);

    match resp {
        Ok(r) => {
            let status = r.status().as_u16();
            if status == 200 {
                Ok(())
            } else if status == 401 {
                Err("the OpenTyphoon key is not valid (refused by server)".into())
            } else if status == 429 {
                Err("OpenTyphoon is busy (too many requests); try again in a minute".into())
            } else {
                Err(format!("OpenTyphoon returned HTTP status {status}"))
            }
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("401") {
                Err("the OpenTyphoon key is not valid".into())
            } else if msg.contains("429") {
                Err("OpenTyphoon is busy (too many requests); try again in a minute".into())
            } else {
                Err(format!("couldn't reach OpenTyphoon: {msg}"))
            }
        }
    }
}

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
        .or_else(read_stored_key)
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
