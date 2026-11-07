//! View features of the canvas: rulers, scrolling, two-page spread, units choice and the
//! keyboard-focusable canvas (GD-04, UI-06, AX-04). Logic lives here to keep `lib.rs` small.

use crate::{NewpubApp, Tool};
use egui::{Color32, FontId, Pos2, Rect as ERect, Sense, Stroke, TextureHandle, Vec2};
use newpub_engine::action::Units;
use newpub_engine::core::{Command, Id, Rect};
use newpub_engine::{Query, SessionAction};

/// Thickness of a ruler, in screen points.
const RULER: f32 = 20.0;
/// Space around the pages inside the canvas, in screen points.
const PAD: f32 = 20.0;
/// Gap between the two pages of a spread, in screen points.
const SPREAD_GAP: f32 = 16.0;
/// Default size of a text frame inserted from the keyboard: 3 in x 1.5 in.
const KEY_FRAME: (f64, f64) = (216.0, 108.0);

/// App-side view state that is not part of the document.
pub struct ViewState {
    /// Scroll offset of the canvas, in screen points.
    pub scroll: Vec2,
    /// Two-page spread mode.
    pub spread: bool,
    /// The session's measurement units (mirrors the engine; refreshed every frame).
    pub units: Units,
    /// Text frame being edited from the keyboard.
    pub editing: Option<Id>,
    /// "Link to Next Box" is waiting for a click on the box the story should continue in.
    pub link_from: Option<Id>,
    /// The far corner (table, row, col) of a cell selection that starts at the caret's cell.
    pub cell_extent: Option<(Id, usize, usize)>,
    /// A ruler guide being dragged.
    pub guide_drag: Option<crate::guides_ui::GuideDrag>,
    /// Screen rect of the page area (inside the rulers) from the last frame.
    pub canvas_area: ERect,
    /// Show text frame and shape boundaries on the canvas.
    pub boundaries: bool,
    /// Document (file, page size) the view was last fitted to.
    pub fit_key: Option<(Option<std::path::PathBuf>, i64, i64)>,
    /// Egui id of the canvas widget.
    pub canvas_id: Option<egui::Id>,
    /// Texture of the second page of a spread: (texture, revision, page, scale key).
    other_tex: Option<(TextureHandle, u64, usize, u32)>,
}

impl ViewState {
    /// Clears what only makes sense for the publication being shown (text editing, "Link to Next Box", a cell
    /// selection, a guide drag): called whenever another publication replaces it.
    pub(crate) fn reset_transient(&mut self) {
        self.link_from = None;
        self.editing = None;
        self.cell_extent = None;
        self.guide_drag = None;
    }
}

impl Default for ViewState {
    fn default() -> Self {
        ViewState {
            scroll: Vec2::ZERO,
            spread: false,
            units: Units::In,
            editing: None,
            link_from: None,
            cell_extent: None,
            guide_drag: None,
            canvas_area: ERect::NOTHING,
            boundaries: true,
            fit_key: None,
            canvas_id: None,
            other_tex: None,
        }
    }
}

pub(crate) fn units_code(u: Units) -> &'static str {
    match u {
        Units::In => "in",
        Units::Cm => "cm",
        Units::Mm => "mm",
        Units::Pt => "pt",
        Units::Pi => "pi",
    }
}

fn units_name(u: Units) -> &'static str {
    match u {
        Units::In => "Inches",
        Units::Cm => "Centimeters",
        Units::Mm => "Millimeters",
        Units::Pt => "Points",
        Units::Pi => "Picas",
    }
}

/// Points per unit.
pub(crate) fn units_points(u: Units) -> f64 {
    match u {
        Units::In => 72.0,
        Units::Cm => 72.0 / 2.54,
        Units::Mm => 72.0 / 25.4,
        Units::Pt => 1.0,
        Units::Pi => 12.0,
    }
}

const ALL_UNITS: [Units; 5] = [Units::In, Units::Cm, Units::Mm, Units::Pt, Units::Pi];

/// Candidate major tick steps (in the unit) with the number of minor divisions of each.
const STEPS: [(f64, i64); 14] = [
    (0.125, 1),
    (0.25, 2),
    (0.5, 2),
    (1.0, 4),
    (2.0, 2),
    (5.0, 5),
    (10.0, 5),
    (20.0, 2),
    (50.0, 5),
    (100.0, 5),
    (200.0, 2),
    (500.0, 5),
    (1000.0, 5),
    (5000.0, 5),
];

fn tick_label(v: f64) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

impl NewpubApp {
    /// Page indices shown on the canvas: the current page, or the spread containing it
    /// (page 1 alone, then pairs 2-3, 4-5 ...; a last page without partner alone).
    pub fn visible_pages(&self) -> Vec<usize> {
        let n = self.session.doc().pages.len();
        if !self.view.spread || n == 0 {
            return vec![self.page];
        }
        if self.page == 0 {
            return vec![0];
        }
        let first = if self.page % 2 == 1 { self.page } else { self.page - 1 };
        if first + 1 < n { vec![first, first + 1] } else { vec![first] }
    }

    /// Size of everything shown, in document points (pages side by side).
    pub(crate) fn content_pts(&self) -> (f32, f32) {
        let s = &self.session.doc().setup;
        let (w, h) = (s.width.0 as f32, s.height.0 as f32);
        let k = self.visible_pages().len() as f32;
        (w * k + SPREAD_GAP * (k - 1.0) / self.zoom.max(0.01), h)
    }

    pub(crate) fn reset_scroll(&mut self) {
        self.view.scroll = Vec2::ZERO;
    }

    /// 100 %: one screen point per document point.
    pub(crate) fn actual_size(&mut self) {
        self.zoom = 1.0;
        self.fitted = false;
        self.reset_scroll();
    }

    /// Mirrors the session's units into the view state.
    pub(crate) fn sync_units(&mut self) {
        let u = self.session.query(&Query::Session).ok().and_then(|v| v.get("units").cloned());
        if let Some(u) = u.and_then(|v| serde_json::from_value::<Units>(v).ok()) {
            self.view.units = u;
        }
    }

    /// Extra fields of `view_state`.
    pub(crate) fn view_extra(&self, v: &mut serde_json::Value) {
        v["units"] = units_code(self.view.units).into();
        v["snapping"] = self.design_ui.snapping.into();
        v["scroll"] = serde_json::json!([self.view.scroll.x, self.view.scroll.y]);
        v["spread"] = self.view.spread.into();
        v["visible_pages"] = serde_json::json!(self.visible_pages());
        v["editing"] = self.editing_frame().is_some().into();
        v["window_closes"] = self.window_closes.into();
        self.flow_view(v);
        v["preview"] = self.preview.as_ref().map(|p| p.0.clone()).into();
        v["preview_rendered"] =
            self.preview_texture.as_ref().zip(self.preview.as_ref()).is_some_and(|(t, p)| t.1 == p.0).into();
        v["table_cell"] = self.current_cell().map(|(_, r, c)| serde_json::json!([r, c])).into();
        v["table_selection"] =
            self.cell_selection().map(|(_, r, c, rows, cols)| serde_json::json!([r, c, rows, cols])).into();
        v["text_selection"] = match &self.caret {
            Some(c) if self.editing_frame() == Some(c.frame) => serde_json::json!([c.range().start, c.range().end]),
            _ => serde_json::Value::Null,
        };
    }

    /// The frame being edited, if it is still selected.
    pub(crate) fn editing_frame(&self) -> Option<Id> {
        self.view.editing.filter(|id| self.selection.contains(id))
    }

    /// Escape: leaves text editing (keeping the selection), else clears the selection. ("Link to Next Box" takes
    /// Escape before this, in `shortcuts`.)
    pub(crate) fn escape(&mut self) {
        if self.editing_frame().is_some() {
            self.view.editing = None;
        } else {
            self.selection.clear();
        }
    }

    /// True while the canvas has keyboard focus (its keys are shortcuts, not widget input).
    pub(crate) fn canvas_focused(&self, ctx: &egui::Context) -> bool {
        self.view.canvas_id.is_some_and(|id| ctx.memory(|m| m.has_focus(id)))
    }

    /// The measurement-units choice (status bar).
    pub(crate) fn units_combo(&mut self, ui: &mut egui::Ui) {
        let mut units = self.view.units;
        let combo = egui::ComboBox::from_id_salt("units").selected_text(units_name(units)).show_ui(ui, |ui| {
            for u in ALL_UNITS {
                ui.selectable_value(&mut units, u, units_name(u));
            }
        });
        combo.response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, "Measurement units"));
        if units != self.view.units {
            self.view.units = units;
            self.act(SessionAction::SetUnits { units });
        }
    }

    /// Renders (or reuses) the texture of a page other than the current one.
    fn other_texture(&mut self, ctx: &egui::Context, page: usize, ppp: f32) -> Option<TextureHandle> {
        let rev = self.session.revision();
        let scale_key = (self.zoom * ppp * 100.0) as u32;
        if let Some((t, r, p, s)) = &self.view.other_tex
            && (*r, *p, *s) == (rev, page, scale_key)
        {
            return Some(t.clone());
        }
        let pm = self.session.render_page(page, self.render_dpi(ctx, ppp)).ok()?;
        let img = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], pm.data());
        let tex = ctx.load_texture("page-other", img, egui::TextureOptions::LINEAR);
        self.view.other_tex = Some((tex.clone(), rev, page, scale_key));
        Some(tex)
    }

    /// The canvas with rulers, scrolling, spread and accessible object nodes.
    pub(crate) fn canvas_view(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let ppp = ctx.pixels_per_point();
        let full = ui.available_rect_before_wrap();
        let hr = ERect::from_min_max(full.min + Vec2::new(RULER, 0.0), Pos2::new(full.max.x, full.min.y + RULER));
        let vr = ERect::from_min_max(full.min + Vec2::new(0.0, RULER), Pos2::new(full.min.x + RULER, full.max.y));
        let cv = ERect::from_min_max(full.min + Vec2::splat(RULER), full.max);
        self.canvas_size = cv.size();

        // Scrolling: apply the wheel, then clamp to the content.
        let (cw0, ch0) = self.content_pts();
        let content0 = Vec2::new(cw0 * self.zoom + 2.0 * PAD, ch0 * self.zoom + 2.0 * PAD);
        let max_scroll = (content0 - cv.size()).max(Vec2::ZERO);
        if ctx.pointer_hover_pos().is_some_and(|p| cv.contains(p)) {
            let d = ctx.input(|i| i.smooth_scroll_delta);
            self.view.scroll -= d;
        }
        self.view.scroll = self.view.scroll.clamp(Vec2::ZERO, max_scroll);

        // A new or opened publication (another file or page size) starts fitted to the window.
        let key = (
            self.session.path.clone(),
            self.session.doc().setup.width.0 as i64,
            self.session.doc().setup.height.0 as i64,
        );
        if self.view.fit_key.as_ref() != Some(&key) && cv.width() > 100.0 && cv.height() > 100.0 {
            self.view.fit_key = Some(key);
            self.fit_page();
        }
        let (cw, ch) = self.content_pts();
        let content = Vec2::new(cw * self.zoom + 2.0 * PAD, ch * self.zoom + 2.0 * PAD);
        // Geometry of the shown pages: centred while they fit the canvas.
        let doc = self.session.doc();
        let (w, h) = (doc.setup.width.0 as f32, doc.setup.height.0 as f32);
        let pages = self.visible_pages();
        let step = w * self.zoom + SPREAD_GAP;
        let centre = ((cv.size() - content) / 2.0).max(Vec2::ZERO);
        let first_origin = cv.min + Vec2::splat(PAD) + centre - self.view.scroll;
        let idx = pages.iter().position(|p| *p == self.page).unwrap_or(0);
        self.page_origin = first_origin + Vec2::new(step * idx as f32, 0.0);
        let zoom = self.zoom;
        let page_rect = move |k: usize| {
            ERect::from_min_size(first_origin + Vec2::new(step * k as f32, 0.0), Vec2::new(w * zoom, h * zoom))
        };

        let id = ui.id().with("page-canvas");
        self.view.canvas_id = Some(id);

        // Object nodes for assistive technology: registered below the canvas so they never take pointer input.
        for (k, p) in pages.iter().enumerate() {
            self.object_nodes(ui, *p, page_rect(k).min);
        }

        self.view.canvas_area = cv;
        let resp = ui.interact(cv, id, Sense::click_and_drag());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Page canvas"));
        if resp.clicked() || resp.drag_started() {
            // Working on the page takes the keyboard from the panels.
            resp.request_focus();
        }
        if ctx.memory(|m| m.has_focus(id)) {
            // Arrow keys and Escape belong to the canvas while it is focused; so does Tab while typing in a table
            // cell (it moves to the next cell).
            let tab = self.current_cell().is_some();
            ctx.memory_mut(|m| {
                m.set_focus_lock_filter(
                    id,
                    egui::EventFilter { horizontal_arrows: true, vertical_arrows: true, escape: true, tab },
                )
            });
        }
        // A press on the other page of a spread makes it the current page.
        if resp.contains_pointer()
            && ctx.input(|i| i.pointer.primary_pressed())
            && let Some(pos) = ctx.pointer_latest_pos()
        {
            for (k, p) in pages.iter().enumerate() {
                if *p != self.page && page_rect(k).contains(pos) {
                    self.page = *p;
                    self.selection.clear();
                    self.page_origin = page_rect(k).min;
                }
            }
        }

        let tex = self.page_texture(&ctx, ppp);
        let painter = ui.painter().with_clip_rect(cv);
        for (k, p) in pages.iter().enumerate() {
            let rect = page_rect(k);
            let shadow =
                egui::epaint::Shadow { offset: [0, 6], blur: 22, spread: 0, color: Color32::from_black_alpha(46) };
            painter.add(shadow.as_shape(rect, 2.0));
            let t = if *p == self.page { tex.clone() } else { self.other_texture(&ctx, *p, ppp) };
            match t {
                Some(t) => {
                    painter.image(t.id(), rect, ERect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                }
                None => {
                    painter.rect_filled(rect, 0.0, Color32::WHITE);
                }
            }
        }
        self.draw_guides(&painter);
        self.draw_ruler_guides(&painter, cv, ctx.pointer_latest_pos().filter(|_| self.view.guide_drag.is_some()));
        self.draw_hover(&painter, ctx.pointer_hover_pos().filter(|p| cv.contains(*p)));
        // Under the selection, so its outline and handles stay on top (the same one-frame lag as the preview).
        self.draw_snap_lines(&painter);
        self.draw_selection(&painter);
        self.draw_cell_selection(&painter);
        self.draw_text_edit(&painter);
        if resp.has_focus() {
            let c = crate::theme::palette(&ctx).primary;
            painter.rect_stroke(cv.shrink(1.0), 0.0, Stroke::new(2.0, c), egui::StrokeKind::Inside);
        }
        self.handle_canvas_input(&resp);
        // Right-click selects what is under the pointer, then opens the context menu.
        if resp.secondary_clicked()
            && let Some(p) = resp.interact_pointer_pos()
        {
            let (x, y) = self.screen_to_page(p);
            match self.hit(x, y) {
                Some(id) if self.selection.contains(&id) => {}
                Some(id) => {
                    self.end_text_edit();
                    self.selection = vec![id];
                }
                None => {
                    self.end_text_edit();
                    self.selection.clear();
                }
            }
        }
        resp.context_menu(|ui| self.canvas_menu(ui));
        self.canvas_keys(&ctx, &resp);
        // The rectangle of a drawing tool, or the marquee of the Select tool.
        if let (Some(a), Some(b)) = (self.drag_start, self.drag_now)
            && (self.tool != Tool::Select || self.marquee.is_some())
        {
            let c = crate::theme::palette(&ctx).primary;
            painter.rect_filled(ERect::from_two_pos(a, b), 0.0, c.gamma_multiply(0.08));
            painter.rect_stroke(ERect::from_two_pos(a, b), 0.0, Stroke::new(1.0, c), egui::StrokeKind::Middle);
        }

        // Rulers.
        let hresp = ui.interact(hr, ui.id().with("hruler"), Sense::drag());
        hresp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Horizontal ruler"));
        let vresp = ui.interact(vr, ui.id().with("vruler"), Sense::drag());
        vresp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Vertical ruler"));
        // Drag a guide out of a ruler: the top ruler gives a horizontal guide, the left one a vertical guide.
        self.ruler_drag(&hresp, newpub_engine::core::Orientation::Horizontal, cv);
        self.ruler_drag(&vresp, newpub_engine::core::Orientation::Vertical, cv);
        let pointer = ctx.pointer_hover_pos();
        self.draw_ruler(ui, hr, true, first_origin.x, pointer.map(|p| p.x));
        self.draw_ruler(ui, vr, false, first_origin.y, pointer.map(|p| p.y));
    }

    /// Draws a ruler along `r`; `origin` is the screen coordinate of document 0 along its axis.
    fn draw_ruler(&self, ui: &egui::Ui, r: ERect, horizontal: bool, origin: f32, pointer: Option<f32>) {
        let painter = ui.painter().with_clip_rect(r);
        let pal = crate::theme::palette(ui.ctx());
        let (bg, fg) = (pal.surface, pal.text_muted);
        painter.rect_filled(r, 0.0, bg);
        let edge_line = if horizontal {
            [Pos2::new(r.min.x, r.max.y - 0.5), Pos2::new(r.max.x, r.max.y - 0.5)]
        } else {
            [Pos2::new(r.max.x - 0.5, r.min.y), Pos2::new(r.max.x - 0.5, r.max.y)]
        };
        painter.line_segment(edge_line, Stroke::new(1.0, pal.border));
        let (a, b) = if horizontal { (r.min.x, r.max.x) } else { (r.min.y, r.max.y) };
        let unit = units_points(self.view.units);
        let (step, subs) = STEPS
            .iter()
            .copied()
            .find(|(s, _)| (s * unit) as f32 * self.zoom >= 48.0)
            .unwrap_or(STEPS[STEPS.len() - 1]);
        let major = step * unit * self.zoom as f64;
        let minor = major / subs as f64;
        let k0 = ((a - origin) as f64 / minor).floor() as i64;
        let k1 = ((b - origin) as f64 / minor).ceil() as i64;
        let edge = if horizontal { r.max.y } else { r.max.x };
        let font = FontId::proportional(9.5);
        for k in k0..=k1 {
            let pos = origin + (k as f64 * minor) as f32;
            let is_major = k.rem_euclid(subs) == 0;
            let len = if is_major { RULER * 0.7 } else { RULER * 0.35 };
            let (p0, p1) = if horizontal {
                (Pos2::new(pos, edge), Pos2::new(pos, edge - len))
            } else {
                (Pos2::new(edge, pos), Pos2::new(edge - len, pos))
            };
            painter.line_segment([p0, p1], Stroke::new(1.0, fg.gamma_multiply(if is_major { 0.9 } else { 0.5 })));
            if is_major {
                let label = tick_label(k as f64 / subs as f64 * step);
                let at =
                    if horizontal { Pos2::new(pos + 2.0, r.min.y + 1.0) } else { Pos2::new(r.min.x + 1.0, pos + 1.0) };
                painter.text(at, egui::Align2::LEFT_TOP, label, font.clone(), fg);
            }
        }
        if let Some(p) = pointer.filter(|p| (a..=b).contains(p)) {
            let (p0, p1) = if horizontal {
                (Pos2::new(p, r.min.y), Pos2::new(p, r.max.y))
            } else {
                (Pos2::new(r.min.x, p), Pos2::new(r.max.x, p))
            };
            painter.line_segment([p0, p1], Stroke::new(1.0, pal.primary));
        }
    }

    /// Invisible, named rectangles over the objects of a page so screen readers can list them.
    fn object_nodes(&self, ui: &egui::Ui, page: usize, origin: Pos2) {
        let doc = self.session.doc();
        let Some(p) = doc.pages.get(page) else { return };
        for id in &p.objects {
            let Some(o) = doc.objects.get(id) else { continue };
            let name = crate::a11y::object_name(doc, o);
            let r = ERect::from_min_size(
                origin + Vec2::new(o.rect.x as f32, o.rect.y as f32) * self.zoom,
                Vec2::new(o.rect.w as f32, o.rect.h as f32) * self.zoom,
            );
            let resp = ui.interact(r, ui.id().with(("object", page, id.0)), Sense::hover());
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, &name));
        }
    }

    /// Keyboard operation of the focused canvas: Enter with the Text Box tool inserts a default frame
    /// centred in the margin area, selects it and starts text editing.
    fn canvas_keys(&mut self, ctx: &egui::Context, resp: &egui::Response) {
        if !resp.has_focus() || self.dialog != crate::Dialog::None {
            return;
        }
        let enter = ctx.input(|i| {
            i.events.iter().any(|e| matches!(e, egui::Event::Key { key: egui::Key::Enter, pressed: true, .. }))
        });
        if !enter || self.tool != Tool::TextBox {
            return;
        }
        let doc = self.session.doc();
        let (t, b, l, r) = doc.page_margins(self.page);
        let (pw, ph) = (doc.setup.width.0, doc.setup.height.0);
        let (aw, ah) = (pw - l - r, ph - t - b);
        let (w, h) = (KEY_FRAME.0.min(aw.max(1.0)), KEY_FRAME.1.min(ah.max(1.0)));
        let rect = Rect::new(l + (aw - w) / 2.0, t + (ah - h) / 2.0, w, h);
        let page = Some(self.page);
        let out = self.act(Command::AddTextFrame { page, master: None, rect, columns: None, gutter: None });
        if let Some(id) = out.and_then(|o| o.created.first().copied()) {
            self.selection = vec![id];
            self.view.editing = Some(id);
            self.tool = Tool::Select;
        }
    }
}
