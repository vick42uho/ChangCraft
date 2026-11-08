---
name: changcraft-thai-edition
description: Comprehensive architecture guide, Thai language PDF engine specifications, font subsetting/embedding rules, OCR pipelines, and packaging instructions for ChangCraft (Thai Edition of PdfCraft). Activate whenever modifying Thai text processing, font management, HarfRust shaping, OCR (Paddle/Typhoon), or building ChangCraft release packages.
---

# ChangCraft (Thai Edition) Architecture & Engineering Guide

This skill documents the full architecture, subsystems, conventions, and instructions for **ChangCraft (Thai Edition)** — the clean-room open-source PDF application written in Rust, customized and optimized specifically for the Thai language and Southeast Asian typography.

---

## 1. Core Principles & Governance

All contributors and AI agents working on this codebase **must** adhere strictly to:
1. **AGENTS.md & Clean-Room**:
   - Zero Adobe assets, reverse engineering, or Adobe CMap/decompilation shortcuts (`AGENTS.md §1.1`).
   - Only open licenses: OFL-1.1, Apache-2.0, MIT, ISC (`AGENTS.md §1.2`).
   - `unsafe_code = "forbid"` across the workspace.
   - Never crash on malformed inputs: always return `Result`, never use `unwrap()`/`expect()` in production code.
2. **Side-by-Side Coexistence**:
   - ChangCraft has its own distinct product ID, GUIDs, shortcuts, and Registry entries (`ChangCraft`, `changcraft.exe`).
   - It installs alongside upstream PdfCraft without registry or file-association collisions.

---

## 2. Thai Language & PDF Typography Subsystem

### 2.1 Thai Shaping & Unicode Normalization
- **HarfRust (HarfBuzz port in Rust)**: All complex Thai script shaping (vowel repositioning, tone mark stacking like `น้ำ`, `ผู้ใหญ่`, `ปั้นจั่น`, `กี่ปี่`) is handled in `crates/fonts/src/shaping.rs`.
- **Sara Am Decomposition**:
  - In `crates/fonts/src/embedding.rs` and `crates/fonts/src/pdf.rs`, `ำ` (Sara Am, `U+0E33`) is decomposed into `ํ` (Nikhahit, `U+0E4D`) + `า` (Sara Aa, `U+0E32`) during PDF encoding and shaping.
  - This prevents caret misplacement, incorrect width calculation, and deletion bugs.
- **PUA Normalization**:
  - Legacy Windows-874 / MacThai fonts often use Unicode Private Use Area (`U+F700`..`U+F71A`) for shifted tone marks.
  - Handled in `crates/edit/src/text.rs` by mapping PUA codepoints back to standard Thai Unicode codepoints (`U+0E00`..`U+0E7F`).

### 2.2 Word-Breaking & Line Wrapping
- **ICU & Dictionary-Based Line Breaking**:
  - Thai does not use spaces between words. Line wrapping uses ICU dictionary break iterator (`crates/edit/src/linebreak.rs`).
  - Leading vowels (`เ`, `แ`, `โ`, `ใ`, `ไ`) are kept cohesive with their base consonants.
  - Line wraps prefer existing spaces when present; if an oversized Thai word exceeds line width, cluster-aware emergency wrap activates (`crates/edit/src/text_regions.rs`).

### 2.3 TrueType/OpenType Subsetting & Embedding (Type0 / CIDFontType2)
- **Files**: `crates/fonts/src/embedding.rs`, `crates/fonts/src/lib.rs`, `crates/engine/src/text_fonts.rs`.
- **TTC (TrueType Collection) Extraction for Angsana New, Cordia, Browallia**:
  - Windows packages classic Thai fonts in `.ttc` files (`angsana.ttc`, `cordia.ttc`, `browalia.ttc`).
  - `extract_ttc_face(bytes, index)` unpacks any specific face from a `.ttc` into a standalone TrueType font binary on-the-fly.
  - This preserves `fsType = 0x0008` (Editable Embedding) without violating copyright, since fonts are read from the user's local Windows installation at runtime.
- **Key Mechanics**:
  - Embedded Thai fonts (`Sarabun`, `Noto Sans Thai`, `IBM Plex Sans Thai`, plus discovered system fonts like `Angsana New`) become PDF `Type0` composite fonts with `Identity-H` encoding and a `ToUnicode` CMap.
  - Edit paths embed the **complete** program (`embed`), so a later edit can add glyphs; generated appearances may map only the glyphs they use (`embed_mapping`) while keeping the full source program.
  - This ensures small PDF size, high fidelity across all PDF viewers (Edge, Acrobat, Preview), and copy/search capability.
  - **Bold/Italic Preservation**: When editing text in `crates/edit/src/text.rs`, if the original text was bold or italic, the engine maintains the bold/italic variant (`Sarabun-Bold`, `NotoSansThai-Bold`, `AngsanaNew-Bold`) rather than dropping to regular.
  - **Font Name Cleaning & Deduplication**:
    - Random 6-letter subset prefixes (`BAAAAA+`, `AAAAAA+`) are stripped when displaying font pickers in `crates/ui-egui/src/format_text_ui.rs`.
    - Duplicate font family entries in UI dropdowns are deduplicated by postscript/family name.
    - System Thai fonts (`Angsana New`, `Cordia New`, `Browallia New`, `Kanit`, `Prompt`, `TH SarabunPSK`) are pinned directly under `🇹🇭 ฟอนต์ไทยในเครื่อง (Installed Thai Fonts)` in the UI dropdown.

### 2.3.1 Editing keeps the document's exact font family
- **Identity**: only the 6-letter subset prefix (`ABCDEF+`) and the style suffix (`-Bold`, `-Italic`, `-BoldItalic`) are ignored. `family_key`/`same_family` therefore keep `THSarabunPSK` distinct from `Sarabun`, and from `Sarabun New`. The loose `matching_faces` (name similarity) stays for render-time fallbacks only.
- **Face chain when existing text is edited** (`crates/edit/src/text.rs`, `crates/edit/src/text_regions.rs`):
  1. the document's own embedded program for that family, when it is complete (a subset that never had a character cannot gain it);
  2. otherwise a bundled craft-fonts face of the **same** family (`matching_faces_exact`);
  3. otherwise, for Thai only, the same family installed on this computer (`pdfcraft_fonts::embedding::installed_face`), read at runtime and never bundled, committed or copied into the app.
- **No look-alike substitution**: Thai text with no complete same-family face is refused with the family, the character (`… has no glyph for … (U+0E…)`), and what to do (install the family, or *Load font…*). Latin, Cyrillic and Japanese keep their previous Type 3 / Helvetica fallbacks; the installed-font lookup is not consulted for them, so their behaviour is unchanged.
- **Installed lookup details**: `installed_face` scans `font_directories()`, uses the file name only as a cheap prefilter (`THSarabun.ttf` is opened for family `THSarabunPSK`), decides identity from the font's own family / PostScript / full names, rejects programs whose `fsType` forbids editable embedding, and caches per session.
- **Tests**: `crates/fonts/src/embedding.rs` (`family_keys_distinguish_lookalike_families`, `an_installed_file_is_only_used_for_the_family_it_actually_contains`, `a_known_bold_run_takes_the_bundled_bold_face_even_when_the_name_does_not_say_so`), `crates/edit/src/tests.rs` (`a_new_character_comes_from_the_source_fonts_own_embedded_program`), `crates/engine/tests/document_fonts.rs` (`editor_faces_are_cached_until_the_page_changes`).

### 2.4 Forms (AcroForms) & Annotations (FreeText, Watermarks, Header/Footer)
- **Files**: `crates/forms/src/appearance.rs`, `crates/annot/src/appearance.rs`.
- When users enter Thai text into a form field or add a text box/watermark:
  - If the existing font cannot show Thai glyphs, the engine dynamically falls back to embedded `Sarabun` or user-selected Thai fonts (`Noto Sans Thai`, `IBM Plex Sans Thai`).
  - The appearance stream `/AP` constructs a composite Type0 font entry with proper `/CIDInit /ProcSet findresource begin ...` ToUnicode mapping.

---

## 3. OCR Architecture (PaddleOCR & OpenTyphoon)

### 3.1 Local PaddleOCR Engine
- **File**: `crates/ocr/src/paddle.rs`.
- Completely on-device inference using ONNX models (`assets/models/`).
- Supports Thai, English, and numeric recognition simultaneously.
- Generates invisible text overlay aligned with the underlying bitmap, preserving word bounding boxes and search/copy functionality.

### 3.2 Cloud Typhoon OCR Engine (Optional)
- **File**: `crates/ocr/src/typhoon.rs`, `crates/ui-egui/src/handwriting.rs`.
- Used specifically for complex Thai handwriting.
- Requires user's own OpenTyphoon API key in Preferences. If no key is set, zero external network requests are made.

---

## 4. UI, Theming & Print Pipeline

### 4.1 System Font Fallback & Icon Texture Loading
- **Files**: `crates/ui-egui/src/system_fonts.rs`, `crates/ui-egui/src/widgets.rs`.
- **System Fonts**: On Windows, falls back to `Leelawadee UI` or `Tahoma` for glyphs not present in UI Inter/JetBrains fonts.
- **Icon Texture Loading**:
  - `egui_extras` is compiled with `features = ["svg"]` only (no image loaders).
  - Therefore, any embedded PNG (such as the ChangCraft elephant icon in the About dialog) **must** be loaded into an `egui::ColorImage` and registered via `ui.ctx().load_texture("changcraft-icon", ...)` rather than using `egui::Image::from_bytes("bytes://...")`.

### 4.2 Print Pipeline
- **Vector Mode (Default)**: Sends PDF vectors directly to the spooler (`spool::Scaling::Exact`). Native printer resolution (600/1200/2400 DPI) is preserved.
- **Raster Mode ("Print as image" in Advanced)**:
  - `crates/engine/src/print_raster.rs`: renders at `scale = (dpi / 72.0) as f32` clamped between 72 and 600 DPI.
  - Resolves at exactly 150, 300, or 600 DPI on standard paper sizes.

### 4.3 Multi-Signature Slots (Fill & Sign)
- **Capacity**: Expands saved signatures up to 5 slots (Slot 0: Primary signature, Slot 1: Initials, Slots 2–4: Extra signatures/stamps), matching PDFelement workflows.
- **Graphic Modes**: Each slot accepts Drawn (mouse/pen vector strokes), Typed (script typography), or Image/Scan (transparent PNG, JPG photo, or official stamp seal).
- **Persistence**: Persisted in application settings under `"signature"`, `"initials"`, and `"extra_signatures"`, with 5 individual texture preview caches.
- **Active Selection**: Clicking any signature card activates it immediately for placement on pages with real-time hover preview.

### 4.4 Object Editing & Delete Key
- **Delete / Backspace in Edit Mode**: Selected content objects (text blocks, page images, and added content) are deleted with Delete or Backspace key as a single atomic undo batch (`Edit::Batch` labeled "Delete content").
- **Inventory Ordering**: Target deletions order from last index to first within each inventory type (Text -> Image -> Added) to prevent renumbering invalidations.

### 4.5 Ortho Line & Arrow Snapping (Shift Key)
- **CAD Ortho Snapping**: Holding `Shift` while drawing new lines or arrows constrains them to exact level (horizontal 0°) or plumb (vertical 90°) orientation (`measure::snap::ortho`).
- **Reshaping**: Dragging endpoints or corners of lines, polylines, and polygons with `Shift` snaps to nearest ortho orientation.

---

## 5. Build, Test & Packaging Workflows

### 5.1 Environment Prerequisites
Always set `CRAFT_FONTS_DIR` to the local fonts repository:
```powershell
$env:CRAFT_FONTS_DIR = "D:\craft-fonts"
```

### 5.2 Compiling & Checking
```powershell
# Fast check
cargo check -p pdfcraft-ui-egui
cargo check -p pdfcraft-engine

# Run tests
cargo test -p pdfcraft-engine
cargo test -p pdfcraft-fonts

# Build release executable
cargo build --release -p pdfcraft
Copy-Item target\release\pdfcraft.exe target\release\changcraft.exe -Force
```

### 5.3 Building Windows Installer (MSI) & Portable ZIP
- **Script**: `packaging/windows/package-th.ps1`
- **WiX Templates**: `packaging/windows/changcraft.wxs`, `packaging/windows/changcraft-ui.wxs`
- **Output Artifacts**:
  - `dist\release\changcraft-<version>-windows-x64.msi`
  - `dist\release\changcraft-<version>-windows-x64-portable.zip`

Run packaging:
```powershell
powershell -File packaging\windows\package-th.ps1
```

### 5.4 Icon Generation
- **Script**: `packaging/icons-th.py`
- Generates all multi-size `.ico` (16 to 256px) and Linux freedesktop hicolor PNGs from `assets/app-icon/changcraft-master.png`.
