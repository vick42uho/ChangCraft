//! Same-page mixed selection in Edit a PDF. Source references belong to one document
//! generation; a drag previews geometry and commits one engine edit on release.

use std::collections::HashMap;
use std::sync::Arc;

use egui::{Color32, Pos2, Rect, Stroke};
use pdfcraft_engine::{Document, Edit, EditableObject, MAX_MOVE_OBJECTS, ObjectKind, ObjectTarget};
use pdfcraft_render::DocInfo;

use crate::canvas::{DocView, PageXform};

const BLUE: Color32 = Color32::from_rgb(0x14, 0x73, 0xE6);

#[derive(Default)]
pub(crate) struct ObjectSelection {
    enabled: bool,
    generation: Option<u64>,
    cache: HashMap<usize, Result<Vec<EditableObject>, String>>,
    page: Option<usize>,
    selected: Vec<EditableObject>,
    gesture: Option<Gesture>,
    /// Expected geometry after our own edit, for remapping indexes that grouping can change.
    pending: Option<Vec<EditableObject>>,
    pub notice: Option<String>,
    pub hold_editor: bool,
    blocked_page: Option<usize>,
    input_blocked: bool,
}

#[derive(Clone, Copy)]
struct Gesture {
    start: Pos2,
    page_origin: Pos2,
    now: Pos2,
    marquee: bool,
    additive: bool,
}

impl Gesture {
    fn anchor(self, xf: &PageXform) -> Pos2 {
        self.start + (xf.rect.min - self.page_origin)
    }
}

impl ObjectSelection {
    pub fn count(&self) -> usize {
        self.selected.len()
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn drag_page(&self) -> Option<usize> {
        self.page.filter(|_| self.gesture.is_some())
    }

    pub(crate) fn single_text_block(&self) -> Option<(usize, usize, [f32; 4])> {
        let page = self.page?;
        if self.selected.len() == 1 && self.selected[0].target.kind == ObjectKind::Text {
            Some((page, self.selected[0].target.index, self.selected[0].rect.map(|v| v as f32)))
        } else {
            None
        }
    }

    pub(crate) fn active_text_drag_block(&self, page: usize) -> Option<usize> {
        let g = self.gesture.filter(|g| !g.marquee)?;
        if self.page == Some(page)
            && self.selected.len() == 1
            && self.selected[0].target.kind == ObjectKind::Text
            && g.now.distance(g.start) > 1.0
        {
            Some(self.selected[0].target.index)
        } else {
            None
        }
    }

    pub fn selected_text(&self, doc: &Document) -> Option<(String, [f64; 4])> {
        let page = self.page?;
        let blocks = doc.text_blocks(page);
        let texts: Vec<String> = self
            .selected
            .iter()
            .filter_map(|o| if o.target.kind == ObjectKind::Text { blocks.get(o.target.index).map(|b| b.text.clone()) } else { None })
            .collect();
        if texts.is_empty() {
            return None;
        }
        let rect = self.selected.first().map(|o| [o.rect[0], o.rect[1], o.rect[2], o.rect[3]]).unwrap_or([100.0, 700.0, 250.0, 750.0]);
        Some((texts.join("\n"), rect))
    }

    /// The page and references of the objects selected together, if any: what Copy and Cut take.
    pub(crate) fn selection(&self) -> Option<(usize, Vec<ObjectTarget>)> {
        self.page.filter(|_| !self.selected.is_empty()).map(|page| (page, self.selected.iter().map(|o| o.target).collect()))
    }

    pub(crate) fn select_target(&mut self, doc: &Document, page: usize, target: ObjectTarget) {
        if let Ok(objects) = self.inventory(doc, page)
            && let Some(obj) = objects.into_iter().find(|o| o.target == target)
        {
            self.clear();
            self.page = Some(page);
            self.selected = vec![obj];
            return;
        }
        if target.kind == ObjectKind::Text
            && let Some(b) = doc.text_blocks(page).into_iter().nth(target.index)
        {
            self.clear();
            self.page = Some(page);
            self.selected = vec![EditableObject { target, rect: b.rect, content_type: "text", text: Some(b.text) }];
        }
    }

    /// Select what a paste added to `page`, as Acrobat selects pasted objects: the objects of
    /// `after` (the page's inventory with the paste) that `before` (without it) didn't have. They
    /// are taken at the next `prepare`, which sees the new document generation.
    pub(crate) fn select_pasted(&mut self, page: usize, before: &[EditableObject], after: Vec<EditableObject>) {
        let same = |a: &EditableObject, b: &EditableObject| {
            a.target.kind == b.target.kind
                && a.content_type == b.content_type
                && a.text == b.text
                && a.rect.iter().zip(b.rect).all(|(x, y)| (x - y).abs() < 0.05)
        };
        let mut old: Vec<&EditableObject> = before.iter().collect();
        let added: Vec<_> = after
            .into_iter()
            .filter(|o| match old.iter().position(|b| same(b, o)) {
                Some(i) => {
                    old.swap_remove(i);
                    false
                }
                None => true,
            })
            .collect();
        self.clear();
        if !added.is_empty() && added.len() <= MAX_MOVE_OBJECTS {
            self.page = Some(page);
            self.pending = Some(added);
        }
    }

    /// A preview belongs to the active, focused document. Returning from another tab or a
    /// modal must not turn a release that happened elsewhere into a document edit.
    pub fn set_input_blocked(&mut self, blocked: bool) {
        self.input_blocked = blocked;
        if blocked {
            self.gesture = None;
            self.blocked_page = None;
        }
    }

    fn clear(&mut self) {
        self.page = None;
        self.selected.clear();
        self.gesture = None;
        self.pending = None;
        self.blocked_page = None;
    }

    pub fn prepare(&mut self, doc: &Document, enabled: bool, ctx: &egui::Context) {
        self.enabled = enabled;
        self.hold_editor = false;
        if !enabled {
            self.clear();
            return;
        }
        let generation = doc.edit_generation();
        if self.generation != Some(generation) {
            self.cache.clear();
            self.gesture = None;
            let expected = self.pending.take();
            let remapped = self.page.zip(expected).and_then(|(page, expected)| {
                let inventory = doc.editable_objects(page).ok()?;
                let mut out = Vec::new();
                for old in expected {
                    let mut matches = inventory.iter().filter(|o| {
                        o.target.kind == old.target.kind
                            && o.content_type == old.content_type
                            && o.text == old.text
                            && o.rect.iter().zip(old.rect).all(|(a, b)| (a - b).abs() < 0.05)
                    });
                    let object = matches.next()?;
                    if matches.next().is_some() || out.iter().any(|o: &EditableObject| o.target == object.target) {
                        return None;
                    }
                    out.push(object.clone());
                }
                Some(out)
            });
            if let Some(objects) = remapped {
                self.selected = objects;
            } else {
                self.clear();
            }
            self.generation = Some(generation);
        } else {
            // A refused edit keeps the generation and the original selection.
            self.pending = None;
        }
        if self.page.is_some() && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.clear();
        }
    }

    /// The move of the whole selection by `offset` (user space). The selection follows it.
    fn move_by(&mut self, page: usize, offset: [f64; 2]) -> Edit {
        let mut expected = self.selected.clone();
        for object in &mut expected {
            object.rect = [object.rect[0] + offset[0], object.rect[1] + offset[1], object.rect[2] + offset[0], object.rect[3] + offset[1]];
        }
        self.pending = Some(expected);
        Edit::MoveObjects { page, objects: self.selected.iter().map(|o| o.target).collect(), offset }
    }

    fn move_across_pages(&mut self, dst_page: usize, offset: [f64; 2]) {
        let mut expected = self.selected.clone();
        for object in &mut expected {
            object.rect = [object.rect[0] + offset[0], object.rect[1] + offset[1], object.rect[2] + offset[0], object.rect[3] + offset[1]];
            if object.target.kind == ObjectKind::Added {
                object.target.kind = if object.content_type == "image" { ObjectKind::Image } else { ObjectKind::Text };
            }
        }
        self.page = Some(dst_page);
        self.pending = Some(expected);
    }

    fn inventory(&mut self, doc: &Document, page: usize) -> Result<Vec<EditableObject>, String> {
        if self.cache.len() >= 8 && !self.cache.contains_key(&page) {
            self.cache.clear();
        }
        self.cache.entry(page).or_insert_with(|| doc.editable_objects(page)).clone()
    }
}

fn screen(xf: &PageXform, info: &DocInfo, page: usize, object: &EditableObject) -> Rect {
    xf.user_rect(info, page, object.rect.map(|v| v as f32))
}

fn modified(ui: &egui::Ui) -> bool {
    ui.input(|i| {
        let modifiers = i
            .events
            .iter()
            .find_map(|e| match e {
                egui::Event::PointerButton { button: egui::PointerButton::Primary, pressed: true, modifiers, .. } => Some(*modifiers),
                _ => None,
            })
            .unwrap_or(i.modifiers);
        modifiers.shift || modifiers.ctrl || modifiers.command
    })
}

pub(crate) fn target_page_at(view: &DocView, src_page: usize, pointer: Pos2, center_hint: Pos2) -> usize {
    let rects = view.visible_page_rects();
    for p in [pointer, center_hint] {
        if let Some((page, _)) = rects.iter().find(|(_, r)| r.contains(p)) {
            return *page;
        }
    }
    src_page
}

pub(crate) fn cross_page_move_edit(
    view: &DocView,
    doc: &Document,
    src_page: usize,
    dst_page: usize,
    targets: &[ObjectTarget],
    src_rect: [f64; 4],
    drag_delta_screen: egui::Vec2,
) -> Result<(Edit, [f64; 2]), String> {
    let src_xf = view.page_xform(src_page).ok_or_else(|| "missing source page transform".to_string())?;
    let dst_xf = view.page_xform(dst_page).ok_or_else(|| "missing destination page transform".to_string())?;
    let dst_info = doc.info.pages.get(dst_page).ok_or_else(|| "invalid destination page".to_string())?;
    let src_screen = src_xf.user_rect(&doc.info, src_page, [src_rect[0] as f32, src_rect[1] as f32, src_rect[2] as f32, src_rect[3] as f32]);
    let dst_center_screen = src_screen.center() + drag_delta_screen;
    let (vx, vy) = dst_xf.screen_to_view(dst_center_screen);
    let [dst_ux, dst_uy] = dst_info.view_to_user(vx, vy);
    let cx = (src_rect[0] + src_rect[2]) * 0.5;
    let cy = (src_rect[1] + src_rect[3]) * 0.5;
    let offset = [f64::from(dst_ux) - cx, f64::from(dst_uy) - cy];
    if !offset.iter().all(|v| v.is_finite()) {
        return Err("invalid cross-page offset".to_string());
    }
    let clip = Arc::new(doc.copy_objects(src_page, targets).map_err(|e| e.to_string())?);
    let paste = Edit::PasteObjects { page: dst_page, clip, offset };
    let mut edits = vec![paste];
    if let Some(del) = Edit::delete_objects(src_page, targets.to_vec()) {
        edits.push(del);
    }
    let label = if targets.len() == 1 { "Move object" } else { "Move objects" }.into();
    Ok((Edit::Batch { label, edits }, offset))
}

pub(crate) fn single(view: &DocView, page: usize) -> Option<ObjectTarget> {
    if let Some((_, index)) = view.content.selected.filter(|(p, _)| *p == page) {
        Some(ObjectTarget { kind: ObjectKind::Added, index })
    } else if let Some(s) = view.image_selection.as_ref().filter(|s| s.page == page) {
        Some(ObjectTarget { kind: ObjectKind::Image, index: s.index })
    } else {
        view.line_editor.as_ref().filter(|e| e.page == page).map(|e| ObjectTarget { kind: ObjectKind::Text, index: e.block })
    }
}

fn leave_single(view: &mut DocView) {
    view.content.selected = None;
    view.image_selection = None;
    view.line_editor = None;
    view.block_drag = None;
}

/// ⌘A / Ctrl+A in Edit a PDF: every object on the current page selected together, as Acrobat selects
/// them (paragraphs, pictures and added content), instead of the page's text. A page whose
/// objects can't be selected together, or has too many, says so and keeps what was selected.
pub(crate) fn select_all(view: &mut DocView, doc: &Document) {
    let page = view.current;
    match view.objects.inventory(doc, page) {
        Ok(objects) if objects.len() > MAX_MOVE_OBJECTS => view.objects.notice = Some(tl!("Select at most 1000 objects at a time.").into()),
        Ok(objects) => {
            view.objects.clear();
            if !objects.is_empty() {
                leave_single(view);
                view.objects.page = Some(page);
                view.objects.selected = objects;
            }
        }
        Err(_) => view.objects.notice = Some(tl!("This page contains objects that cannot be selected together.").into()),
    }
}

/// Returns true while group selection owns the interaction. Ordinary clicks still reach the
/// single-object text/resize tools. A text draft is finished or discarded through its editor.
pub(crate) fn page_input(ui: &egui::Ui, resp: &egui::Response, xf: &PageXform, page: usize, doc: &Document, view: &mut DocView) -> bool {
    if view.objects.input_blocked || egui::Popup::is_any_open(ui.ctx()) {
        return true;
    }
    if view.content.draft.is_some() || view.line_editor.as_ref().is_some_and(|e| e.has_changes()) {
        let blocked = view.objects.blocked_page == Some(page);
        let modifier_press = ui.input(|i| i.pointer.primary_pressed())
            && modified(ui)
            && ui.input(|i| i.pointer.interact_pos()).is_some_and(|p| xf.rect.contains(p) && ui.clip_rect().contains(p));
        if blocked || modifier_press {
            view.objects.hold_editor = true;
            view.objects.blocked_page = ui.input(|i| i.pointer.primary_down()).then_some(page);
            if modifier_press {
                view.objects.notice = Some(tl!("Finish or discard the text being edited before selecting several objects.").into());
            }
            return true;
        }
        return false;
    }
    let mut selection = std::mem::take(&mut view.objects);
    let result = input(&mut selection, ui, resp, xf, page, doc, view);
    view.objects = selection;
    result
}

#[allow(clippy::too_many_arguments)]
fn input(s: &mut ObjectSelection, ui: &egui::Ui, resp: &egui::Response, xf: &PageXform, page: usize, doc: &Document, view: &mut DocView) -> bool {
    let (pointer, pressed, down) =
        ui.input(|i| (i.pointer.interact_pos().or_else(|| i.pointer.latest_pos()), i.pointer.primary_pressed(), i.pointer.primary_down()));
    let info = &doc.info;
    if s.page == Some(page)
        && let Some(mut gesture) = s.gesture
    {
        if let Some(p) = pointer {
            gesture.now = p;
        }
        let anchor = gesture.anchor(xf);
        if !down {
            if !gesture.marquee && anchor.distance(gesture.now) <= 3.0 && resp.double_clicked() {
                s.clear();
                return false;
            }
            s.gesture = None;
            if gesture.marquee {
                let area = Rect::from_two_pos(anchor, gesture.now).intersect(xf.rect);
                if !gesture.additive {
                    s.selected.clear();
                }
                if area.width() > 3.0
                    && area.height() > 3.0
                    && let Ok(objects) = s.inventory(doc, page)
                {
                    let candidates: Vec<_> = objects.into_iter().filter(|o| area.intersects(screen(xf, info, page, o))).collect();
                    let mut selected = s.selected.clone();
                    for object in candidates {
                        if !selected.iter().any(|o| o.target == object.target) {
                            selected.push(object);
                        }
                    }
                    if selected.len() <= MAX_MOVE_OBJECTS {
                        s.selected = selected;
                    } else {
                        s.notice = Some(tl!("Select at most 1000 objects at a time.").into());
                    }
                }
                if s.selected.is_empty() {
                    s.clear();
                }
            } else if anchor.distance(gesture.now) > 3.0 && view.pending_edit.is_none() {
                let delta = gesture.now - anchor;
                let mut union_rect = s.selected.first().map_or([0.0; 4], |o| o.rect);
                let mut screen_union = Rect::NOTHING;
                for o in &s.selected {
                    union_rect =
                        [union_rect[0].min(o.rect[0]), union_rect[1].min(o.rect[1]), union_rect[2].max(o.rect[2]), union_rect[3].max(o.rect[3])];
                    screen_union = screen_union.union(screen(xf, info, page, o));
                }
                let dst_page = target_page_at(view, page, gesture.now, screen_union.translate(delta).center());
                if dst_page != page && !s.selected.is_empty() {
                    let targets: Vec<ObjectTarget> = s.selected.iter().map(|o| o.target).collect();
                    match cross_page_move_edit(view, doc, page, dst_page, &targets, union_rect, delta) {
                        Ok((edit, offset)) => {
                            s.move_across_pages(dst_page, offset);
                            view.current = dst_page;
                            view.pending_edit = Some(edit);
                        }
                        Err(_) => s.notice = Some(tl!("These objects could not be moved. Select them again and try a smaller move.").into()),
                    }
                } else {
                    let (a, b) = (xf.screen_to_view(anchor), xf.screen_to_view(gesture.now));
                    match doc.object_move_offset(page, [f64::from(b.0 - a.0), f64::from(b.1 - a.1)]) {
                        Ok(offset) => {
                            if s.selected.len() == 1 && s.selected[0].target.kind == ObjectKind::Text {
                                let obj = &s.selected[0];
                                let current_rect = obj.rect.map(|v| v as f32);
                                let raster_rect = view.text_background.raster_source_rect(
                                    page,
                                    current_rect,
                                    view.page_raster_out_of_date(page),
                                );
                                let to_rect = [
                                    (obj.rect[0] + offset[0]) as f32,
                                    (obj.rect[1] + offset[1]) as f32,
                                    (obj.rect[2] + offset[0]) as f32,
                                    (obj.rect[3] + offset[1]) as f32,
                                ];
                                view.text_background.remember_move(xf, page, obj.target.index, raster_rect, to_rect);
                            }
                            view.pending_edit = Some(s.move_by(page, offset));
                        }
                        Err(_) => s.notice = Some(tl!("These objects could not be moved. Select them again and try a smaller move.").into()),
                    }
                }
            }
        } else {
            s.gesture = Some(gesture);
        }
        ui.ctx().set_cursor_icon(if gesture.marquee { egui::CursorIcon::Crosshair } else { egui::CursorIcon::Move });
        return true;
    }
    let Some(p) = pointer.filter(|p| xf.rect.contains(*p) && ui.clip_rect().contains(*p) && resp.rect.contains(*p)) else {
        return pressed && s.page == Some(page) && !s.selected.is_empty();
    };
    if !pressed {
        if s.page == Some(page) && !s.selected.is_empty() {
            let union = s.selected.iter().fold(Rect::NOTHING, |r, o| r.union(screen(xf, info, page, o)));
            if union.expand(2.0).contains(p) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Move);
                return true;
            }
            return modified(ui)
                || ((resp.clicked() || resp.secondary_clicked())
                    && s.selected.iter().any(|o| screen(xf, info, page, o).expand(2.0).contains(p)));
        }
        if let Some(ed) = view.line_editor.as_ref().filter(|e| e.page == page) {
            let r = xf.user_rect(info, page, ed.source_rect);
            if on_frame(r, p) && !crate::edit_text_ui::grabs_open_edge(r.expand(2.0), p) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Move);
                return true;
            }
        }
        return false;
    }
    let additive = modified(ui);
    if additive {
        let objects = match s.inventory(doc, page) {
            Ok(objects) => objects,
            Err(_) => {
                s.notice = Some(tl!("This page contains objects that cannot be selected together.").into());
                return true;
            }
        };
        // A page-sized background image must not hide editable labels or added notes.
        // Within each kind, prefer the last object as the ordinary single-item tools do.
        let hit = [ObjectKind::Added, ObjectKind::Text, ObjectKind::Image]
            .into_iter()
            .find_map(|kind| objects.iter().rev().find(|o| o.target.kind == kind && screen(xf, info, page, o).expand(2.0).contains(p)));
        if s.page != Some(page) {
            s.clear();
            s.page = Some(page);
            if let Some(target) = single(view, page)
                && let Some(object) = objects.iter().find(|o| o.target == target)
            {
                s.selected.push(object.clone());
            }
        }
        if let Some(object) = hit {
            if let Some(index) = s.selected.iter().position(|o| o.target == object.target) {
                s.selected.remove(index);
            } else if s.selected.len() < MAX_MOVE_OBJECTS {
                s.selected.push(object.clone());
            } else {
                s.notice = Some(tl!("Select at most 1000 objects at a time.").into());
            }
        } else {
            s.gesture = Some(Gesture { start: p, page_origin: xf.rect.min, now: p, marquee: true, additive: true });
        }
        leave_single(view);
        view.current = page;
        return true;
    }
    if s.page == Some(page) && !s.selected.is_empty() {
        let union = s.selected.iter().fold(Rect::NOTHING, |r, o| r.union(screen(xf, info, page, o)));
        if union.expand(2.0).contains(p) && !resp.double_clicked() {
            s.gesture = Some(Gesture { start: p, page_origin: xf.rect.min, now: p, marquee: false, additive: false });
            return true;
        }
    }
    // The frame of the paragraph open for typing takes hold of the box, as in Acrobat: a click
    // selects it (for the arrow keys), a drag moves it. Its text keeps the caret; the width
    // handles keep rewrapping.
    if let Some(block) = view.line_editor.as_ref().filter(|e| e.page == page).and_then(|ed| {
        let r = xf.user_rect(info, page, ed.source_rect);
        (on_frame(r, p) && !crate::edit_text_ui::grabs_open_edge(r.expand(2.0), p)).then_some(ed.block)
    }) {
        let Ok(objects) = s.inventory(doc, page) else {
            return false;
        };
        if let Some(object) = objects.into_iter().find(|o| o.target == ObjectTarget { kind: ObjectKind::Text, index: block }) {
            s.clear();
            s.page = Some(page);
            s.selected.push(object);
            s.gesture = Some(Gesture { start: p, page_origin: xf.rect.min, now: p, marquee: false, additive: false });
            leave_single(view);
            view.current = page;
            return true;
        }
    }
    s.clear();
    // Avoid parsing the full page object inventory synchronously on an ordinary press when
    // cached single-tool inventories already know whether a text box, handle, image, or added
    // item is under the pointer.
    let generation = doc.edit_generation();
    let added = doc.added();
    let hits_added = crate::content_ui::hits_added(xf, info, page, added, p) || crate::content_ui::select_grabs(ui, xf, page, info, added, view);
    let hits_text = {
        let lines = match view.edit_lines.get(&page) {
            Some((g, l)) if *g == generation => l.clone(),
            _ => {
                let l = doc.text_blocks(page);
                view.edit_lines.insert(page, (generation, l.clone()));
                l
            }
        };
        lines.iter().any(|l| xf.user_rect(info, page, l.rect.map(|v| v as f32)).expand(2.0).contains(p))
            || crate::edit_text_ui::grabs_edge(xf, info, page, lines.iter().map(|l| l.rect), p)
    };
    let hits_image = {
        let images = match view.edit_images.get(&page) {
            Some((g, im)) if *g == generation => im.clone(),
            _ => {
                let im = doc.page_images(page);
                view.edit_images.insert(page, (generation, im.clone()));
                im
            }
        };
        images.iter().any(|im| {
            xf.user_rect(info, page, im.rect.map(|v| v as f32)).expand(2.0).contains(p)
                || im.visible_rect.is_some_and(|v| xf.user_rect(info, page, v.map(|c| c as f32)).expand(2.0).contains(p))
        })
    };
    if hits_added || hits_text || hits_image {
        return false;
    }
    leave_single(view);
    s.page = Some(page);
    s.gesture = Some(Gesture { start: p, page_origin: xf.rect.min, now: p, marquee: true, additive: false });
    view.current = page;
    true
}

/// Whether `p` is on the frame drawn around a paragraph open for typing (`r`, screen): just
/// outside the text or on the top move handle, where a press would otherwise close the paragraph.
fn on_frame(r: Rect, p: Pos2) -> bool {
    let frame = r.expand(2.0);
    crate::edit_text_ui::move_handle(frame).expand(5.0).contains(p) || (frame.expand(FRAME_REACH).contains(p) && !r.contains(p))
}

/// How far outside a paragraph's frame a press still grabs it (screen pixels).
const FRAME_REACH: f32 = 10.0;

/// The arrow keys move the selected image or objects a point, Shift and an arrow ten points,
/// as in Acrobat. They are taken before the view turns the page with them.
pub(crate) fn nudge_keys(ctx: &egui::Context, view: &mut DocView, doc: &Document) {
    if ctx.egui_wants_keyboard_input() || view.pending_edit.is_some() {
        return;
    }
    let image = view.image_selection.as_ref().filter(|s| !s.dragging()).map(|s| (s.page, s.index));
    let objects = view.objects.page.filter(|_| !view.objects.selected.is_empty() && view.objects.gesture.is_none());
    let Some(page) = objects.or(image.map(|(page, _)| page)) else { return };
    let Some(xf) = view.page_xform(page) else { return };
    use egui::{Key, Modifiers};
    let mut step = egui::Vec2::ZERO;
    ctx.input_mut(|i| {
        for (key, direction) in [
            (Key::ArrowLeft, egui::vec2(-1.0, 0.0)),
            (Key::ArrowRight, egui::vec2(1.0, 0.0)),
            (Key::ArrowUp, egui::vec2(0.0, -1.0)),
            (Key::ArrowDown, egui::vec2(0.0, 1.0)),
        ] {
            // Shift first: a plain key pattern also matches a Shift press.
            if i.consume_key(Modifiers::SHIFT, key) {
                step += direction * 10.0;
            } else if i.consume_key(Modifiers::NONE, key) {
                step += direction;
            }
        }
    });
    if step == egui::Vec2::ZERO {
        return;
    }
    // A point on screen, whatever the zoom and the view's rotation: one screen pixel's step in
    // view space, at unit length.
    let c = xf.rect.center();
    let unit = |d: egui::Vec2| {
        let (a, b) = (xf.screen_to_view(c), xf.screen_to_view(c + d));
        let v = egui::vec2(b.0 - a.0, b.1 - a.1);
        if v.length() > 0.0 { v / v.length() } else { egui::Vec2::ZERO }
    };
    let delta = unit(egui::vec2(1.0, 0.0)) * step.x + unit(egui::vec2(0.0, 1.0)) * step.y;
    let Ok(offset) = doc.object_move_offset(page, [f64::from(delta.x), f64::from(delta.y)]) else { return };
    if objects.is_some() {
        view.pending_edit = Some(view.objects.move_by(page, offset));
    } else if let Some((page, index)) = image
        && let Some(r) = doc.page_images(page).get(index).map(|im| im.rect)
    {
        let rect = [r[0] + offset[0], r[1] + offset[1], r[2] + offset[0], r[3] + offset[1]];
        view.pending_edit = Some(Edit::EditPageImage { page, index, change: pdfcraft_engine::ImageEdit::Move(rect) });
    }
}

/// Delete (or Backspace) removes the selected objects, as in Acrobat: a paragraph selected by its
/// frame, or everything an area selection took, goes at once instead of one typed character at a
/// time.
pub(crate) fn delete_keys(ctx: &egui::Context, view: &mut DocView) {
    if ctx.egui_wants_keyboard_input() || view.pending_edit.is_some() || view.objects.gesture.is_some() {
        return;
    }
    let Some(page) = view.objects.page.filter(|_| !view.objects.selected.is_empty()) else { return };
    use egui::{Key, Modifiers};
    if !ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Delete) || i.consume_key(Modifiers::NONE, Key::Backspace)) {
        return;
    }
    view.pending_edit = Edit::delete_objects(page, view.objects.selected.iter().map(|o| o.target).collect());
    view.objects.clear();
}

/// The edit that deletes `targets` (indexes into one inventory of `page`) as one undo step.
/// Each deletion renumbers only what follows it in its own inventory, so paragraphs and images
/// go from the last to the first, and added content, which the source inventories leave out,
/// after them.
#[allow(dead_code)]
fn delete_edit(page: usize, mut targets: Vec<ObjectTarget>) -> Option<Edit> {
    targets.sort_by_key(|t| {
        (
            match t.kind {
                ObjectKind::Text => 0,
                ObjectKind::Image => 1,
                ObjectKind::Added => 2,
            },
            std::cmp::Reverse(t.index),
        )
    });
    targets.dedup();
    let mut edits: Vec<Edit> = targets
        .into_iter()
        .map(|t| match t.kind {
            ObjectKind::Text => Edit::EditTextBlock { page, block: t.index, text: String::new(), style: pdfcraft_engine::BlockStyle::default() },
            ObjectKind::Image => Edit::EditPageImage { page, index: t.index, change: pdfcraft_engine::ImageEdit::Delete },
            ObjectKind::Added => Edit::DeleteContent { page, index: t.index },
        })
        .collect();
    if edits.len() > 1 {
        // "Delete content" is an edit label already, so the history reads the same in every language.
        Some(Edit::Batch { label: "Delete content".into(), edits })
    } else {
        edits.pop()
    }
}

pub(crate) fn paint(painter: &egui::Painter, xf: &PageXform, page: usize, info: &DocInfo, view: &DocView) {
    let selection = &view.objects;
    if selection.page != Some(page) {
        return;
    }
    // A moving selection's boxes and pixels glide with the pointer; for a single text block,
    // `TextBackground` hides the stationary source glyphs as soon as it is ready.
    let delta = selection.gesture.filter(|g| !g.marquee).map_or(egui::Vec2::ZERO, |g| g.now - g.anchor(xf));
    let moving = delta.length() > 1.0;
    let tex = moving.then(|| view.page_raster(page)).flatten();
    let single_text_ready =
        selection.selected.len() == 1 && selection.selected[0].target.kind == ObjectKind::Text && view.text_background.ready(page, selection.selected[0].target.index);
    let mut union = Rect::NOTHING;
    for object in &selection.selected {
        let src = screen(xf, info, page, object);
        let rect = src.translate(delta);
        if let Some(tex) = tex {
            let raster_src = if object.target.kind == ObjectKind::Text {
                let r = view.text_background.raster_source_rect(page, object.rect.map(|v| v as f32), view.page_raster_out_of_date(page));
                xf.user_rect(info, page, r)
            } else {
                src
            };
            let tint = if single_text_ready { Color32::WHITE } else { Color32::from_white_alpha(230) };
            crate::edit_text_ui::paint_raster_crop_tinted(painter, xf, tex, raster_src.expand(1.0), rect.expand(1.0), tint);
        }
        painter.rect_filled(rect, 0.0, BLUE.gamma_multiply(0.05));
        painter.rect_stroke(rect, 0.0, Stroke::new(1.5, BLUE), egui::StrokeKind::Outside);
        union = union.union(rect);
    }
    if selection.selected.len() > 1 {
        painter.rect_stroke(union.expand(4.0), 0.0, Stroke::new(1.0, BLUE.gamma_multiply(0.7)), egui::StrokeKind::Outside);
    }
    if let Some(g) = selection.gesture.filter(|g| g.marquee) {
        let area = Rect::from_two_pos(g.anchor(xf), g.now).intersect(xf.rect);
        painter.rect_filled(area, 0.0, BLUE.gamma_multiply(0.08));
        painter.rect_stroke(area, 0.0, Stroke::new(1.0, BLUE), egui::StrokeKind::Outside);
    }
}

/// Keep the UI refusal actionable and translated; headless callers retain the detailed error.
pub(crate) fn move_error(error: &pdfcraft_engine::EditError) -> String {
    let detail = error.to_string();
    match detail.as_str() {
        "vertical text cannot be moved in a group" => tl!("Vertical text cannot be moved with this selection.").into(),
        "text used as a clipping path cannot be moved in a group" => tl!("Text used as a clipping path cannot be moved with this selection.").into(),
        _ => tl!("These objects could not be moved. Select them again and try a smaller move.").into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(kind: ObjectKind, index: usize) -> ObjectTarget {
        ObjectTarget { kind, index }
    }

    /// Deleting renumbers what follows in the same inventory, so the batch takes each inventory
    /// from its last item to its first, and added content (outside the source inventories) last.
    #[test]
    fn several_objects_delete_last_first_in_one_step() {
        let targets = vec![t(ObjectKind::Added, 0), t(ObjectKind::Text, 1), t(ObjectKind::Image, 0), t(ObjectKind::Text, 3), t(ObjectKind::Added, 2)];
        let Some(Edit::Batch { label, edits }) = delete_edit(0, targets) else { panic!("several objects make one batch") };
        assert_eq!(label, "Delete content");
        let order: Vec<(char, usize)> = edits
            .into_iter()
            .map(|e| match e {
                Edit::EditTextBlock { block, text, .. } => {
                    assert!(text.is_empty());
                    ('t', block)
                }
                Edit::EditPageImage { index, change: pdfcraft_engine::ImageEdit::Delete, .. } => ('i', index),
                Edit::DeleteContent { index, .. } => ('a', index),
                other => panic!("unexpected {}", other.label().wording),
            })
            .collect();
        assert_eq!(order, [('t', 3), ('t', 1), ('i', 0), ('a', 2), ('a', 0)]);
    }

    #[test]
    fn one_object_deletes_with_its_own_edit_and_none_with_nothing() {
        assert!(matches!(delete_edit(2, vec![t(ObjectKind::Image, 4)]), Some(Edit::EditPageImage { page: 2, index: 4, .. })));
        assert!(delete_edit(0, Vec::new()).is_none());
    }

    #[test]
    fn selection_labels_and_refusals_are_translated_in_every_catalog() {
        for language in crate::i18n::LANGUAGES.iter().filter(|l| !l.source.is_empty()) {
            let lang = crate::i18n::Lang::from_code(language.code).unwrap();
            for label in [
                "Drag the selection to move it. Esc clears the selection.",
                "Shift-click to select several objects. Drag empty space to select an area.",
                "Select at most 1000 objects at a time.",
                "Finish or discard the text being edited before selecting several objects.",
                "Move object",
                "Move objects",
                "Vertical text cannot be moved with this selection.",
                "Text used as a clipping path cannot be moved with this selection.",
                "These objects could not be moved. Select them again and try a smaller move.",
                "This page contains objects that cannot be selected together.",
            ] {
                assert_ne!(crate::i18n::tr(lang, label), label, "{}: {label}", language.code);
            }
            assert_ne!(crate::i18n::trn(lang, 2, "{n} object selected", "{n} objects selected"), "2 objects selected", "{}", language.code);
        }
    }

    #[test]
    fn object_selection_preserved_after_move() {
        let mut session = pdfcraft_engine::Session::new();
        let raw = session.create_from_text("move_sel", "Alpha").unwrap();
        let id = session.open("move_sel.pdf", None, raw, None).unwrap();

        let mut selection = ObjectSelection::default();
        let ctx = egui::Context::default();
        ctx.begin_pass(Default::default());
        selection.prepare(session.get(id).unwrap(), true, &ctx);
        ctx.end_pass().textures_delta.clear();

        let objects = session.get(id).unwrap().editable_objects(0).unwrap();
        let target = objects[0].target;
        selection.select_target(session.get(id).unwrap(), 0, target);
        assert_eq!(selection.selected.len(), 1);

        let edit = selection.move_by(0, [18.0, -12.0]);
        session.apply(id, edit).unwrap();

        ctx.begin_pass(Default::default());
        selection.prepare(session.get(id).unwrap(), true, &ctx);
        ctx.end_pass().textures_delta.clear();

        assert_eq!(selection.selected.len(), 1);
        assert_eq!(selection.page, Some(0));
        assert_eq!(selection.selected[0].text.as_deref(), Some("Alpha"));
    }
}
