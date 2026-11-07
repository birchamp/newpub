//! Snapping on the canvas (GD-03 in the UI): while an object is moved or resized with the mouse, its edges and
//! centre snap to the page edges and centre, the margins, grid and ruler guides and the edges and centres of the
//! other objects on the page, and a line across the page shows what it snapped to. Alt (Option on a Mac) places
//! freely.
//!
//! The engine decides (`Query::Snap`, which also honours "Snap to Guides" off); this module only asks it with the
//! right rectangle, keeps the answer for the frame and draws it. Nudging with the arrow keys never snaps.

use crate::{NewpubApp, Tool, fmt_num};
use egui::{Color32, Pos2, Stroke, Vec2};
use newpub_engine::Query;
use newpub_engine::action::Units;
use newpub_engine::core::{Orientation, Rect};

/// Snap distance in screen pixels (ARCHITECTURE.md §12).
const SNAP_PX: f32 = 6.0;

/// Publisher-style alignment line, drawn over a white halo so it reads on the white page in both themes and
/// stays visible where it lies on a pink margin guide or a cyan ruler guide.
const LINE: Color32 = Color32::from_rgb(214, 51, 160);
const HALO: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 230);

/// The key that places freely while dragging, named as the keyboard labels it.
pub(crate) fn free_key() -> &'static str {
    if cfg!(target_os = "macos") { "Option" } else { "Alt" }
}

/// What the live drag snapped to this frame.
#[derive(Default, Clone, Debug, PartialEq)]
pub(crate) struct SnapState {
    /// Added to the raw pointer offset: the move of the whole selection, or of the dragged edges of a resize.
    pub delta: Vec2,
    /// The guide lines to draw, in page points.
    pub lines: Vec<(Orientation, f64)>,
}

impl NewpubApp {
    /// Recomputes the snap of the live move or resize from the raw pointer offset. `moving` is already the raw
    /// offset when this is called; it is corrected in place. Alt bypasses snapping for as long as it is held.
    pub(crate) fn update_snap(&mut self, alt: bool) {
        self.snap = SnapState::default();
        if alt || self.tool != Tool::Select {
            return;
        }
        let tolerance = f64::from(SNAP_PX / self.zoom.max(0.01));
        if let Some(m) = self.moving {
            // A press that barely moved commits nothing (see the release), so it must not snap onto a target.
            if m.length() <= 0.5 {
                return;
            }
            let Some(bounds) = self.selection_bounds() else { return };
            let proposed = Rect::new(bounds.x + f64::from(m.x), bounds.y + f64::from(m.y), bounds.w, bounds.h);
            let ignore = self.selection.clone();
            let Some((snapped, lines)) = self.ask_snap(proposed, ignore, tolerance) else { return };
            let d = Vec2::new((snapped.x - proposed.x) as f32, (snapped.y - proposed.y) as f32);
            if let Some(m) = self.moving.as_mut() {
                *m += d;
            }
            self.snap = SnapState { delta: d, lines };
        } else if let (Some((id, handle, orig)), Some(a), Some(b)) = (self.resizing, self.drag_start, self.drag_now) {
            // Only the dragged edges snap: ask about a point at the dragged corner (or edge) and keep the axis
            // that the handle moves. The edge comes from the original rect plus the raw pointer offset, so a
            // handle dragged past the opposite edge still snaps the edge under the pointer.
            let d = (b - a) / self.zoom;
            let (snaps_x, snaps_y) = handle_axes(handle);
            let ex = (if matches!(handle, 0 | 2 | 6) { orig.x } else { orig.right() }) + f64::from(d.x);
            let ey = (if matches!(handle, 0 | 1 | 4) { orig.y } else { orig.bottom() }) + f64::from(d.y);
            let probe = Rect::new(ex, ey, 0.0, 0.0);
            let Some((snapped, lines)) = self.ask_snap(probe, vec![id], tolerance) else { return };
            let dx = if snaps_x { (snapped.x - probe.x) as f32 } else { 0.0 };
            let dy = if snaps_y { (snapped.y - probe.y) as f32 } else { 0.0 };
            let lines = lines
                .into_iter()
                .filter(|(o, _)| match o {
                    Orientation::Vertical => snaps_x,
                    Orientation::Horizontal => snaps_y,
                })
                .collect();
            self.snap = SnapState { delta: Vec2::new(dx, dy), lines };
        }
    }

    fn ask_snap(
        &mut self,
        rect: Rect,
        ignore: Vec<newpub_engine::core::Id>,
        tolerance: f64,
    ) -> Option<(Rect, Vec<(Orientation, f64)>)> {
        let v = self.session.query(&Query::Snap { page: self.page, rect, ignore, tolerance: Some(tolerance) }).ok()?;
        let snapped: Rect = serde_json::from_value(v.get("rect")?.clone()).ok()?;
        let lines = v
            .get("lines")
            .and_then(|l| l.as_array())
            .map(|l| {
                l.iter()
                    .filter_map(|e| {
                        let pos = e.get("pos")?.as_f64()?;
                        let o = match e.get("orientation")?.as_str()? {
                            "vertical" => Orientation::Vertical,
                            "horizontal" => Orientation::Horizontal,
                            _ => return None,
                        };
                        Some((o, pos))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some((snapped, lines))
    }

    /// Bounding rect of the selected objects, in page points.
    pub(crate) fn selection_bounds(&self) -> Option<Rect> {
        let doc = self.session.doc();
        let mut it = self.selection.iter().filter_map(|id| doc.objects.get(id)).map(|o| o.rect);
        let first = it.next()?;
        Some(it.fold(first, |a, r| {
            let (x0, y0) = (a.x.min(r.x), a.y.min(r.y));
            let (x1, y1) = (a.right().max(r.right()), a.bottom().max(r.bottom()));
            Rect::new(x0, y0, x1 - x0, y1 - y0)
        }))
    }

    /// The rect the status bar reports: the live move or resize, else the selection.
    pub(crate) fn readout_rect(&self) -> Option<Rect> {
        if let Some((_, r)) = self.resize_preview() {
            return Some(r);
        }
        let b = self.selection_bounds()?;
        let m = self.moving.unwrap_or(Vec2::ZERO);
        Some(Rect::new(b.x + f64::from(m.x), b.y + f64::from(m.y), b.w, b.h))
    }

    /// Lines across the page for what the live drag snapped to.
    pub(crate) fn draw_snap_lines(&self, painter: &egui::Painter) {
        if self.snap.lines.is_empty() || self.drag_start.is_none() {
            return;
        }
        let doc = self.session.doc();
        let (w, h) = (doc.setup.width.0, doc.setup.height.0);
        for (o, pos) in &self.snap.lines {
            let (a, b): (Pos2, Pos2) = match o {
                Orientation::Vertical => (self.page_to_screen(*pos, 0.0), self.page_to_screen(*pos, h)),
                Orientation::Horizontal => (self.page_to_screen(0.0, *pos), self.page_to_screen(w, *pos)),
            };
            painter.line_segment([a, b], Stroke::new(3.0, HALO));
            painter.line_segment([a, b], Stroke::new(1.5, LINE));
        }
    }

    /// "x, y · w × h" in the display units, for the status bar. Picas read as the Format panel's "1p6".
    pub(crate) fn readout_text(&self, r: Rect) -> String {
        let u = self.view.units;
        let f = |pt: f64| match u {
            Units::Pi => {
                let picas = (pt / 12.0).trunc();
                format!("{}p{}", fmt_num(picas), fmt_num((pt - picas * 12.0).abs()))
            }
            _ => fmt_num(pt / crate::view::units_points(u)),
        };
        let unit = match u {
            Units::Pi => "",
            _ => crate::view::units_code(u),
        };
        format!("{}, {} · {} × {} {}", f(r.x), f(r.y), f(r.w), f(r.h), unit).trim_end().to_string()
    }
}

/// Which axes a resize handle moves (0 TL, 1 TR, 2 BL, 3 BR, 4 top, 5 bottom, 6 left, 7 right).
fn handle_axes(handle: usize) -> (bool, bool) {
    match handle {
        0..=3 => (true, true),
        4 | 5 => (false, true),
        _ => (true, false),
    }
}
