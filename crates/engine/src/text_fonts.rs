//! Explicit document-font discovery. Interface fallbacks never enter this catalogue.
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct FontFile {
    pub path: PathBuf,
    pub index: u32,
    pub family: String,
    pub face: String,
}

/// Standard user-installed font folders; discovery is performed only on request.
pub fn installed_directories() -> Vec<PathBuf> {
    pdfcraft_fonts::embedding::font_directories()
}

/// Bounded, symlink-free scan. Only standalone, openly licensed, embeddable faces are listed.
/// The caller controls roots (automation resolves confinement before calling this).
pub fn discover(roots: &[PathBuf]) -> Vec<FontFile> {
    use std::io::Read;
    let mut pending: Vec<_> = roots.iter().take(16).map(|r| (r.clone(), 0usize)).collect();
    let mut files = Vec::new();
    let mut entries = 0usize;
    let mut budget = 64u64 << 20;
    let mut seen = std::collections::HashSet::new();
    while let Some((path, depth)) = pending.pop() {
        if entries >= 4096 || budget == 0 {
            break;
        }
        entries += 1;
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        if meta.file_type().is_symlink() || !seen.insert(path.clone()) {
            continue;
        }
        if meta.is_dir() && depth < 4 {
            if let Ok(children) = std::fs::read_dir(&path) {
                let remaining = 4096usize.saturating_sub(entries.saturating_add(pending.len()));
                pending.extend(children.take(remaining).filter_map(Result::ok).map(|p| (p.path(), depth + 1)));
            }
        } else if meta.is_file() && meta.len() <= (16 << 20) && font_extension(&path) {
            let mut bytes = Vec::new();
            let read = std::fs::File::open(&path).and_then(|f| f.take((16 << 20) + 1).read_to_end(&mut bytes));
            if read.is_err() {
                continue;
            }
            let count = pdfcraft_fonts::embedding::ttc_face_count(&bytes);
            let mut matched = false;
            for idx in 0..count.min(16) {
                let Ok(face_bytes) = pdfcraft_fonts::embedding::extract_ttc_face(&bytes, idx) else { continue };
                if pdfcraft_fonts::embedding::validate_document_input(&face_bytes).is_err()
                    || pdfcraft_fonts::embedding::validate(&face_bytes).is_err()
                {
                    continue;
                }
                if let Ok((family, _, face)) = pdfcraft_fonts::embedding::names_with_full(&face_bytes) {
                    files.push(FontFile { path: path.clone(), index: idx, family, face });
                    matched = true;
                }
            }
            if matched {
                budget = budget.saturating_sub(meta.len());
                if budget == 0 {
                    break;
                }
            }
        }
    }
    files.sort_by(|a, b| a.face.cmp(&b.face).then(a.path.cmp(&b.path)).then(a.index.cmp(&b.index)));
    files
}
fn font_extension(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("ttf") || e.eq_ignore_ascii_case("otf") || e.eq_ignore_ascii_case("ttc"))
}
