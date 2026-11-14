//! Print as image (Acrobat's Advanced Print Setup ▸ Print As Image): every page
//! sent to the printer as a picture at a chosen resolution, for printers that skip text,
//! shading or transparency. The pages are rendered the way the viewer shows them (reading
//! colours off, the print's own colours), made into an image-only document, and that document
//! is imposed on the paper like any other. The print is no longer searchable text.

use pdfcraft_create::{TiledPage, TiledSlice, from_tiled_pages};
use pdfcraft_print::Raster;
use pdfcraft_render::{PRINT_TAG, PageRenderer, RenderRequest};

use crate::Document;
use crate::export::encode_png;

/// Windows GDI / Direct2D safe slice pixel budget. Keeping each slice under ~3.5M pixels
/// ensures 300 and 600 DPI print jobs do not hit Windows' 4.19Mpx bitmap downsampling limit (TH15.2).
const MAX_SLICE_PIXELS: u64 = 3_500_000;

/// `doc`'s pages `pages` (0-based, each once, in the order given) as an image-only PDF, one
/// page per entry, the size of the source page as displayed. Pages are sliced into horizontal
/// tiles when necessary to keep each image under the Windows Direct2D spooler bitmap limit.
pub fn rasterize(doc: &Document, pages: &[usize], raster: &Raster, hide_comments: bool) -> Result<Vec<u8>, String> {
    let dpi = raster.dpi.clamp(72.0, 600.0);
    let src = doc.export_source();
    let mut config = src.config.clone();
    config.hide_comments = hide_comments;
    let mut renderer = PageRenderer::new(src.bytes, config);
    let mut tiled_pages: Vec<TiledPage> = Vec::with_capacity(pages.len());
    for &page in pages {
        let (w, h) = src.sizes.get(page).copied().ok_or_else(|| format!("page {} does not exist", page + 1))?;
        let scale = pdfcraft_render::effective_scale(w, h, (dpi / 72.0) as f32);
        let r = renderer.render(RenderRequest { page, scale, tag: PRINT_TAG, ..Default::default() });
        if let Some(e) = r.error {
            return Err(format!("page {}: {e}", page + 1));
        }
        let mut rgba = r.rgba.to_vec();
        over_white(&mut rgba, raster.grayscale);
        let total_px = u64::from(r.width) * u64::from(r.height);
        let mut slices = Vec::new();
        if total_px <= MAX_SLICE_PIXELS {
            slices.push(TiledSlice {
                name: format!("page-{}.png", page + 1),
                bytes: encode_png(r.width, r.height, &rgba)?,
                y_pt: 0.0,
                h_pt: f64::from(h),
            });
        } else {
            let max_slice_h = (MAX_SLICE_PIXELS / u64::from(r.width)).max(64) as u32;
            let num_slices = r.height.div_ceil(max_slice_h);
            for k in 0..num_slices {
                let start_row = k * max_slice_h;
                let end_row = (start_row + max_slice_h).min(r.height);
                let slice_h = end_row - start_row;
                let byte_start = start_row as usize * r.width as usize * 4;
                let byte_end = end_row as usize * r.width as usize * 4;
                let slice_png = encode_png(r.width, slice_h, &rgba[byte_start..byte_end])?;
                // In image coordinates, row 0 is top of page.
                // In PDF coordinates, y=0 is bottom of page.
                let y_pt = (r.height - end_row) as f64 * f64::from(h) / (r.height as f64);
                let h_pt = (slice_h as f64) * f64::from(h) / (r.height as f64);
                slices.push(TiledSlice {
                    name: format!("page-{}-slice-{k}.png", page + 1),
                    bytes: slice_png,
                    y_pt,
                    h_pt,
                });
            }
        }
        tiled_pages.push(TiledPage {
            width: f64::from(w),
            height: f64::from(h),
            slices,
        });
    }
    let doc = from_tiled_pages(&tiled_pages).map_err(|e| e.to_string())?;
    pdfcraft_cos::write_full(&doc, &pdfcraft_cos::SaveOptions::default()).map_err(|e| e.to_string())
}

/// Premultiplied RGBA composed over white paper (opaque), in grey when asked (Rec. 601 luma).
fn over_white(rgba: &mut [u8], grayscale: bool) {
    for px in rgba.as_chunks_mut::<4>().0 {
        let gap = 255 - u32::from(px[3]);
        let (r, g, b) = ((u32::from(px[0]) + gap).min(255), (u32::from(px[1]) + gap).min(255), (u32::from(px[2]) + gap).min(255));
        let (r, g, b) = if grayscale {
            let y = ((299 * r + 587 * g + 114 * b) / 1000).min(255);
            (y, y, y)
        } else {
            (r, g, b)
        };
        px[0] = r as u8;
        px[1] = g as u8;
        px[2] = b as u8;
        px[3] = 255;
    }
}
