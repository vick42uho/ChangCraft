//! Settings and OpenTyphoon API Key management for Thai handwriting & OCR.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use crate::theme::{self, Tokens};
use crate::{PdfCraftApp, widgets};

use pdfcraft_engine::ocr::typhoon;

static TESTING: AtomicBool = AtomicBool::new(false);
static TEST_RESULT: Mutex<Option<Result<String, String>>> = Mutex::new(None);

pub(crate) fn preferences_section(ui: &mut egui::Ui, app: &mut PdfCraftApp, t: &Tokens) {
    ui.label(egui::RichText::new(tl!("OpenTyphoon key")).font(theme::semibold(13.0)));

    egui::Frame::new().fill(t.hover).corner_radius(egui::CornerRadius::same(6)).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
        ui.set_width(ui.available_width());

        ui.label(
            egui::RichText::new(tl!(
                "Reading handwriting sends each page's picture to opentyphoon.ai. Don't use it on documents with personal or confidential information."
            ))
            .small()
            .color(t.text_muted),
        );
        ui.add_space(6.0);

        // Check if background test finished
        if let Ok(mut lock) = TEST_RESULT.try_lock()
            && let Some(res) = lock.take() {
            match res {
                Ok(k) => {
                    let _ = typhoon::save_stored_key(&k);
                    app.typhoon_key_draft.clear();
                    app.typhoon_status = Some(tl!("Key saved.").to_string());
                }
                Err(e) => {
                    app.typhoon_status = Some(e);
                }
            }
        }

        let stored = typhoon::read_stored_key();
        if let Some(ref key) = stored {
            ui.horizontal(|ui| {
                let tail = if key.len() > 6 {
                    &key[key.len() - 4..]
                } else {
                    "..."
                };
                let set_text = crate::i18n::fmt(tl!("Key set (ends in {tail})"), &[("tail", tail)]);
                ui.label(egui::RichText::new(set_text).color(egui::Color32::from_rgb(0x38, 0x8E, 0x3C)).strong());

                if widgets::pill_button(ui, tl!("Remove key"), false).clicked() {
                    let _ = typhoon::remove_stored_key();
                    app.typhoon_status = Some(tl!("Key removed.").to_string());
                }
            });
            ui.add_space(4.0);
        }

        ui.horizontal(|ui| {
            let label = ui.label(tl!("API key"));
            ui.add(
                egui::TextEdit::singleline(&mut app.typhoon_key_draft)
                    .password(true)
                    .desired_width(220.0)
                    .hint_text("sk-...")
            ).labelled_by(label.id);

            let is_testing = TESTING.load(Ordering::Relaxed);
            let btn_label = if is_testing {
                tl!("Testing…")
            } else {
                tl!("Save and test")
            };

            let can_test = !is_testing && !app.typhoon_key_draft.trim().is_empty();
            if ui.add_enabled(can_test, egui::Button::new(btn_label)).clicked() {
                TESTING.store(true, Ordering::Relaxed);
                app.typhoon_status = Some(tl!("Testing…").to_string());
                let key_to_test = app.typhoon_key_draft.trim().to_string();
                let ctx = ui.ctx().clone();

                std::thread::spawn(move || {
                    let res = typhoon::test_key(&key_to_test);
                    TESTING.store(false, Ordering::Relaxed);
                    if let Ok(mut lock) = TEST_RESULT.lock() {
                        *lock = Some(res.map(|()| key_to_test));
                    }
                    ctx.request_repaint();
                });
            }

            ui.hyperlink_to(tl!("Get a key"), "https://playground.opentyphoon.ai/api-key");
        });

        if let Some(ref status) = app.typhoon_status {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(status).small().color(t.text_muted));
        }
    });
}
