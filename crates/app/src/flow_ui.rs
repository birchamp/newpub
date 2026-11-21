//! Text-flow feedback on the canvas (UI-26): the chain of boxes a selected story runs through, Previous and
//! Next box buttons, and the "Link to Next Box" mode with its hint and the boxes the story may continue in.
//! Also (UI-30) the overflow badge as a widget with a tooltip and a right-click menu, and the "Text flow"
//! actions shared by that menu, the canvas menu and the Format panel's Text box card. Everything here is view
//! state; the document changes only through the existing commands and actions (`LinkFrames`, `UnlinkFrame`,
//! `SetTextFrame`, `Autoflow`), one undo step each.

use egui::{Align2, Color32, Pos2, Rect as ERect, Sense, Stroke, Vec2};
use newpub_engine::SessionAction;
use newpub_engine::core::{Autofit, Command, Document, Id, Object, ObjectKind, Rect, TextFramePatch};

use crate::{NewpubApp, Tool, dashed_rect, icons, theme, widgets};

/// Side of the red "+" overflow badge on the bottom edge of a text box.
const BADGE: f32 = 14.0;
/// Accessible name of the overflow badge; the tooltip says what to do about it.
const OVERFLOW_LABEL: &str = "Text overflow";
pub(crate) const OVERFLOW_TIP: &str = "Text overflow: more text than fits in this box. Click to continue the story in another box, or right-click for other ways to fit it.";

/// Radius of a Previous / Next chip on the selected box.
const CHIP: f32 = 9.0;
/// Smallest box (on screen) that shows its chips; a smaller one keeps the Format panel's buttons only.
const CHIP_MIN: Vec2 = Vec2::new(64.0, 44.0);
/// Outline and link icon of a box the story may continue in: readable on the white page in both themes.
const TARGET: Color32 = Color32::from_rgb(30, 142, 90);

impl NewpubApp {
    /// The chain of text box `frame`: its index and every box of its story in reading order. None unless the
    /// story runs through two or more boxes.
    pub(crate) fn chain_of(&self, frame: Id) -> Option<(usize, Vec<Id>)> {
        let doc = self.session.doc();
        let frames = doc.story_of(frame).ok().and_then(|s| doc.story(s).ok()).map(|s| s.frames.clone())?;
        if frames.len() < 2 {
            return None;
        }
        let i = frames.iter().position(|f| *f == frame)?;
        Some((i, frames))
    }

    /// The chain of the one selected text box (None for an empty or multiple selection).
    pub(crate) fn selected_chain(&self) -> Option<(usize, Vec<Id>)> {
        if self.selection.len() != 1 {
            return None;
        }
        self.chain_of(self.selected_text_frame()?)
    }

    /// Selects the box before (`forward == false`) or after `frame` in its story, turning to its page. At either
    /// end of the chain nothing happens, unless `wrap` (Tab) goes round to the other end.
    pub(crate) fn go_to_box(&mut self, frame: Id, forward: bool, wrap: bool) {
        let Some((i, frames)) = self.chain_of(frame) else { return };
        let n = frames.len();
        let j = match (forward, i) {
            (true, i) if i + 1 < n => i + 1,
            (false, i) if i > 0 => i - 1,
            (true, _) if wrap => 0,
            (false, _) if wrap => n - 1,
            _ => return,
        };
        let Some(&next) = frames.get(j) else { return };
        let Some(p) = self.session.doc().page_of(next) else {
            self.status = "That text box is on a master page".into();
            return;
        };
        self.end_text_edit();
        self.page = p;
        self.selection = vec![next];
        self.status = format!("Selected box {} of {} in this story", j + 1, frames.len());
    }

    /// True when `id` is a text box that "Link to Next Box" may continue a story in: empty and not yet linked.
    pub(crate) fn is_link_target(&self, id: Id) -> bool {
        let doc = self.session.doc();
        doc.objects.get(&id).is_some_and(|o| matches!(o.kind, ObjectKind::Text(_)))
            && doc
                .story_of(id)
                .ok()
                .and_then(|s| doc.story(s).ok())
                .is_some_and(|s| s.is_empty() && s.frames.len() <= 1)
    }

    /// The boxes on the current page that the story being linked may continue in.
    pub(crate) fn link_targets(&self) -> Vec<Id> {
        let Some(from) = self.view.link_from else { return Vec::new() };
        let doc = self.session.doc();
        let Some(page) = doc.pages.get(self.page) else { return Vec::new() };
        page.objects.iter().copied().filter(|id| *id != from && self.is_link_target(*id)).collect()
    }

    /// Leaves "Link to Next Box" mode without linking.
    pub(crate) fn cancel_link(&mut self) {
        if self.view.link_from.take().is_some() {
            self.status = "Linking cancelled".into();
        }
    }

    /// Screen rect of a page rect on the current page.
    fn screen_rect(&self, r: Rect) -> ERect {
        ERect::from_min_max(self.page_to_screen(r.x, r.y), self.page_to_screen(r.right(), r.bottom()))
    }

    /// Canvas overlay drawn after the page: the link-mode hint and targets, then the chain of the selected box.
    /// Its buttons live in a child ui clipped to the canvas, so nothing of them reaches the panels around it.
    pub(crate) fn flow_overlay(&mut self, ui: &mut egui::Ui, canvas: ERect) {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(canvas));
        child.set_clip_rect(canvas);
        self.badge_overlay(&mut child, canvas);
        self.link_overlay(&mut child, canvas);
        self.chain_overlay(&mut child, canvas);
    }

    /// The red "+" badge on every text box of the page whose story has more text than fits (UI-30). With the
    /// Select tool it is a button: its tooltip says what the badge means, a click starts "Link to Next Box"
    /// (Publisher's pitcher) and a right-click opens the Text flow menu. Only clicks reach it; a drag that
    /// starts on it still moves or resizes the box through the canvas. In link mode the badge is drawn only, so
    /// a click on it reaches `finish_link` as any click on the box does.
    fn badge_overlay(&mut self, ui: &mut egui::Ui, canvas: ERect) {
        let p = theme::palette(ui.ctx());
        let painter = ui.painter().with_clip_rect(canvas);
        let badges: Vec<(Id, ERect)> = {
            let layout = self.session.layout();
            let doc = self.session.doc();
            let Some(page) = doc.pages.get(self.page) else { return };
            page.objects
                .iter()
                .copied()
                .filter(|id| layout.frames.get(id).is_some_and(|f| f.overflow))
                .filter_map(|id| {
                    let o = doc.objects.get(&id)?;
                    (matches!(o.kind, ObjectKind::Text(_)) && shown(doc, o))
                        .then(|| (id, badge_rect(self.screen_rect(o.rect))))
                })
                .collect()
        };
        // The page is always white and the dark theme's red is a light one, so the "+" is dark on it there
        // (about 5.5:1) and white on the light theme's deeper red (about 4.5:1).
        let ink = if p.dark { p.surface } else { Color32::WHITE };
        // A hover ring that shows on the white page in both themes.
        let ring = Color32::from_black_alpha(90);
        let interactive = self.tool == Tool::Select && self.view.link_from.is_none();
        for (id, r) in badges {
            if !canvas.intersects(r) {
                continue;
            }
            let mut hovered = false;
            if interactive {
                let resp = ui
                    .interact(r, ui.id().with(("overflow-badge", id.0)), Sense::click())
                    .on_hover_text(OVERFLOW_TIP)
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, OVERFLOW_LABEL));
                hovered = resp.hovered();
                if resp.clicked() {
                    self.selection = vec![id];
                    self.start_link(id);
                }
                if resp.secondary_clicked() {
                    // The menu acts on this one box, so a multiple selection collapses to it.
                    self.end_text_edit();
                    self.selection = vec![id];
                }
                resp.context_menu(|ui| {
                    if self.text_flow_menu(ui, id) {
                        ui.close();
                    }
                });
            }
            painter.rect_filled(r, 3.0, p.danger);
            if hovered {
                painter.rect_stroke(r, 3.0, Stroke::new(1.5, ring), egui::StrokeKind::Outside);
            }
            painter.text(r.center(), Align2::CENTER_CENTER, "+", egui::FontId::proportional(12.0), ink);
        }
    }

    /// While linking: the source box, every box the story may continue in, and a banner with a Cancel button.
    fn link_overlay(&mut self, ui: &mut egui::Ui, canvas: ERect) {
        let Some(from) = self.view.link_from else { return };
        let p = theme::palette(ui.ctx());
        let painter = ui.painter().with_clip_rect(canvas);
        let hover = ui
            .ctx()
            .pointer_hover_pos()
            .filter(|q| canvas.contains(*q))
            .map(|q| self.screen_to_page(q))
            .and_then(|(x, y)| self.hit(x, y));
        let doc = self.session.doc();
        if let Some(o) = doc.objects.get(&from).filter(|_| doc.page_of(from) == Some(self.page)) {
            dashed_rect(&painter, self.screen_rect(o.rect), Stroke::new(2.0, p.primary), 6.0);
        }
        let icon = theme::icon_font(16.0 * self.zoom.clamp(0.6, 1.6));
        for id in self.link_targets() {
            let Some(o) = doc.objects.get(&id) else { continue };
            let rr = self.screen_rect(o.rect);
            if hover == Some(id) {
                painter.rect_filled(rr, 0.0, TARGET.gamma_multiply(0.1));
                painter.rect_stroke(rr, 0.0, Stroke::new(2.0, TARGET), egui::StrokeKind::Outside);
            } else {
                dashed_rect(&painter, rr, Stroke::new(1.5, TARGET), 5.0);
            }
            painter.text(rr.center() - Vec2::new(0.0, 18.0), Align2::CENTER_CENTER, icons::LINK, icon.clone(), TARGET);
        }

        // Banner: what to do next, and a way out.
        let text = "Click an empty text box to continue the story there (Esc cancels)";
        let galley = painter.layout_no_wrap(text.to_string(), theme::medium(12.0), p.on_primary);
        let size = Vec2::new(galley.size().x + 70.0, 30.0);
        let r = ERect::from_center_size(Pos2::new(canvas.center().x, canvas.top() + 12.0 + size.y / 2.0), size);
        // Click-sensing (not hover), so a click on the banner never reaches the canvas underneath.
        let banner = ui.interact(r, ui.id().with("link-banner"), Sense::click());
        banner.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Linking text boxes"));
        let cr = ERect::from_center_size(r.right_center() - Vec2::new(17.0, 0.0), Vec2::splat(24.0));
        let cancel = ui
            .interact(cr, ui.id().with("link-cancel"), Sense::click())
            .on_hover_text("Stop linking (Esc)")
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        cancel.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Cancel linking"));
        painter.rect_filled(r, 15.0, p.primary);
        painter.text(
            r.left_center() + Vec2::new(16.0, 0.0),
            Align2::CENTER_CENTER,
            icons::LINK,
            theme::icon_font(16.0),
            p.on_primary,
        );
        painter.galley(r.left_center() + Vec2::new(30.0, -galley.size().y / 2.0), galley, p.on_primary);
        if cancel.hovered() {
            painter.circle_filled(cr.center(), 11.0, Color32::from_white_alpha(60));
        }
        painter.text(cr.center(), Align2::CENTER_CENTER, icons::X, theme::icon_font(14.0), p.on_primary);
        if cancel.clicked() {
            self.cancel_link();
        }
    }

    /// With one box of a linked story selected: a numbered tag on every box of the story on this page, a
    /// dashed outline on the other boxes, dashed connectors between consecutive boxes, and Previous / Next
    /// chips inside the selected box (when it is big enough for them and they are on the canvas).
    fn chain_overlay(&mut self, ui: &mut egui::Ui, canvas: ERect) {
        if self.tool != Tool::Select || self.view.link_from.is_some() {
            return;
        }
        let Some((i, frames)) = self.selected_chain() else { return };
        let p = theme::palette(ui.ctx());
        let painter = ui.painter().with_clip_rect(canvas);
        let n = frames.len();
        let rects: Vec<Option<ERect>> = {
            let doc = self.session.doc();
            frames
                .iter()
                .map(|f| {
                    doc.objects.get(f).filter(|_| doc.page_of(*f) == Some(self.page)).map(|o| self.screen_rect(o.rect))
                })
                .collect()
        };
        let faint = Stroke::new(1.0, p.primary.gamma_multiply(0.7));
        for (k, rr) in rects.iter().enumerate() {
            let Some(rr) = rr else { continue };
            if k != i {
                dashed_rect(&painter, *rr, faint, 4.0);
            }
            // Numbered tag inside the top-left corner, beside the Previous chip.
            let label = format!("{} of {}", k + 1, n);
            let galley = painter.layout_no_wrap(label, theme::medium(10.0), p.on_primary);
            let tag = ERect::from_min_size(rr.left_top() + Vec2::new(30.0, 9.0), galley.size() + Vec2::new(8.0, 3.0));
            painter.rect_filled(tag, 3.0, p.primary);
            painter.galley(tag.left_top() + Vec2::new(4.0, 1.5), galley, p.on_primary);
            // Connector to the next box when both are on this page.
            if let Some(Some(next)) = rects.get(k + 1) {
                let (a, b) = (chips(*rr).1, chips(*next).0);
                painter.extend(egui::Shape::dashed_line(&[a, b], faint, 4.0, 4.0));
            }
        }
        let Some(sel) = rects[i] else { return };
        if sel.width() < CHIP_MIN.x || sel.height() < CHIP_MIN.y {
            return;
        }
        let frame = frames[i];
        let (prev_c, next_c) = chips(sel);
        let mut go = None;
        for (c, forward, exists, icon, label, hint) in [
            (
                prev_c,
                false,
                i > 0,
                icons::CARET_LEFT,
                "Go to previous text box",
                "The box before this one in the story (Shift+Tab)",
            ),
            (
                next_c,
                true,
                i + 1 < n,
                icons::CARET_RIGHT,
                "Go to next text box",
                "The box the story continues in (Tab)",
            ),
        ] {
            if !exists || !canvas.contains(c) {
                continue;
            }
            let resp = ui
                .interact(
                    ERect::from_center_size(c, Vec2::splat(2.0 * CHIP)),
                    ui.id().with(("flow-chip", label)),
                    Sense::click(),
                )
                .on_hover_text(hint)
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
            painter.circle_filled(c, CHIP, if resp.hovered() { p.primary_hover } else { p.primary });
            painter.circle_stroke(c, CHIP, Stroke::new(1.5, p.on_primary));
            painter.text(c, Align2::CENTER_CENTER, icon, theme::icon_font(12.0), p.on_primary);
            if resp.clicked() {
                go = Some(forward);
            }
        }
        if let Some(forward) = go {
            self.go_to_box(frame, forward, false);
        }
    }

    /// Extra `view` query fields for UI journeys: link mode, its targets, the status message and the chain.
    pub(crate) fn flow_view(&self, v: &mut serde_json::Value) {
        v["link_mode"] = serde_json::json!(self.view.link_from);
        v["link_targets"] = serde_json::json!(self.link_targets());
        v["status"] = self.status.clone().into();
        v["story_nav"] = match self.selected_chain() {
            Some((i, frames)) => serde_json::json!({ "index": i, "count": frames.len(), "frames": frames }),
            None => serde_json::Value::Null,
        };
    }
}

/// The overflow badge of a box: a 14 px square astride the bottom edge, inside the bottom-right corner and clear
/// of the Next chip (whose lowest point is 9 px above the edge).
fn badge_rect(rr: ERect) -> ERect {
    ERect::from_center_size(Pos2::new(rr.right() - 10.0, rr.bottom()), Vec2::splat(BADGE))
}

/// True when the object is drawn: not hidden, and not on a hidden layer.
fn shown(doc: &Document, o: &Object) -> bool {
    !o.hidden && o.layer.and_then(|l| doc.layers.iter().find(|x| x.id == l)).is_none_or(|l| l.visible)
}

/// What the Text flow entries may do for one text box, read afresh each frame from the document and the layout.
pub(crate) struct TextFlowState {
    /// The box's story has more text than its boxes show.
    pub overflow: bool,
    /// The story goes on in another box after this one.
    pub has_next: bool,
    /// Position in a story of two or more boxes: (index, count).
    pub chain: Option<(usize, usize)>,
    /// Autofit is off, so "Shrink Text on Overflow" would change something.
    pub autofit_none: bool,
    /// "Link to Next Box" is waiting for a click, started from this box.
    pub linking: bool,
}

/// One entry of the Text flow menu. Each runs one existing command or action: one undo step.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextFlowAction {
    /// Add pages with linked boxes until all the text fits (`SessionAction::Autoflow`).
    Autoflow,
    /// Autofit: shrink the text until it fits (`SetTextFrame { autofit: ShrinkOnOverflow }`).
    Shrink,
    /// Start (or cancel) "Link to Next Box".
    Link,
    /// `UnlinkFrame`: the following boxes no longer continue the story.
    BreakLink,
    Previous,
    Next,
}

/// A Text flow entry as a menu or the Format panel draws it: its button and both tooltips.
pub(crate) struct TextFlowEntry {
    pub action: TextFlowAction,
    pub icon: &'static str,
    pub label: &'static str,
    pub selected: bool,
    pub enabled: bool,
    /// Tooltip when enabled.
    pub tip: &'static str,
    /// Tooltip when greyed out: why the entry cannot apply.
    pub why_not: &'static str,
}

impl TextFlowEntry {
    /// Draws the entry as a small button; true when clicked.
    pub(crate) fn button(&self, ui: &mut egui::Ui) -> bool {
        // A greyed-out entry is still an egui widget that only hovers (see `small_button`), so its reason is
        // chosen here rather than left to `on_disabled_hover_text`, which never applies to it.
        widgets::small_button(ui, self.icon, self.label, self.selected, self.enabled)
            .on_hover_text(if self.enabled { self.tip } else { self.why_not })
            .clicked()
    }
}

/// The Text flow entries for a box in state `st`: the ways to deal with overflow first (Flow onto New Pages,
/// Shrink Text on Overflow), then linking and the chain. Every entry exists, greyed out with the reason when it
/// cannot apply, so a menu teaches what there is. This one table drives the badge menu, the canvas menu and the
/// Format panel's Text box card.
pub(crate) fn text_flow_entries(st: &TextFlowState) -> [TextFlowEntry; 6] {
    [
        TextFlowEntry {
            action: TextFlowAction::Autoflow,
            icon: icons::FILES,
            label: "Flow onto New Pages",
            selected: false,
            enabled: st.overflow,
            tip: "Add pages with linked boxes until all the text fits",
            why_not: "All the text fits",
        },
        TextFlowEntry {
            action: TextFlowAction::Shrink,
            icon: icons::ARROWS_IN_SIMPLE,
            label: "Shrink Text on Overflow",
            selected: false,
            enabled: st.overflow && st.autofit_none,
            tip: "Make the text smaller until it fits this box (the font size you set is kept)",
            why_not: if st.autofit_none {
                "All the text fits"
            } else {
                "Autofit is already on; change it in the Format panel"
            },
        },
        TextFlowEntry {
            action: TextFlowAction::Link,
            icon: icons::LINK,
            label: "Link to Next Box",
            selected: st.linking,
            enabled: !st.has_next,
            tip: "Then click an empty text box: the story continues there (click again or press Esc to stop)",
            why_not: "This box already continues in another box; break that link first",
        },
        TextFlowEntry {
            action: TextFlowAction::BreakLink,
            icon: icons::LINK_BREAK,
            label: "Break Link",
            selected: false,
            enabled: st.has_next,
            tip: "The following boxes no longer continue this story",
            why_not: "This box does not continue in another box",
        },
        TextFlowEntry {
            action: TextFlowAction::Previous,
            icon: icons::CARET_LEFT,
            label: "Previous Text Box",
            selected: false,
            enabled: st.chain.is_some_and(|(i, _)| i > 0),
            tip: "Select the box before this one (Shift+Tab)",
            why_not: if st.chain.is_some() { "This is the first box of the story" } else { "This story has one box" },
        },
        TextFlowEntry {
            action: TextFlowAction::Next,
            icon: icons::CARET_RIGHT,
            label: "Next Text Box",
            selected: false,
            enabled: st.chain.is_some_and(|(i, n)| i + 1 < n),
            tip: "Select the box the story continues in (Tab)",
            why_not: if st.chain.is_some() { "This is the last box of the story" } else { "This story has one box" },
        },
    ]
}

/// The box that carries a story's autofit setting: the first of its chain (the one the engine's autofit fixup
/// and the layout read), or `id` itself for a box with no story.
pub(crate) fn autofit_frame(doc: &Document, id: Id) -> Id {
    doc.story_of(id).ok().and_then(|s| doc.story(s).ok()).and_then(|s| s.frames.first().copied()).unwrap_or(id)
}

impl NewpubApp {
    /// The Text flow state of `id`, or None unless it is a text box.
    pub(crate) fn text_flow_state(&mut self, id: Id) -> Option<TextFlowState> {
        let layout = self.session.layout();
        let doc = self.session.doc();
        let o = doc.objects.get(&id)?;
        let ObjectKind::Text(t) = &o.kind else { return None };
        let sid = doc.story_of(id).ok();
        let frames = sid.and_then(|s| doc.story(s).ok()).map(|s| s.frames.clone()).unwrap_or_default();
        // The story's overflow (what the Format panel and the `overflow` query report), not this box's alone.
        let overflow = sid.is_some_and(|s| layout.stories.get(&s).is_some_and(|l| l.overflow_at.is_some()));
        let chain =
            (frames.len() >= 2).then(|| frames.iter().position(|f| *f == id).map(|i| (i, frames.len()))).flatten();
        // Autofit is the story's, carried by its first box.
        let autofit = match frames.first().and_then(|f| doc.objects.get(f)).map(|o| &o.kind) {
            Some(ObjectKind::Text(f)) => f.autofit,
            _ => t.autofit,
        };
        Some(TextFlowState {
            overflow,
            has_next: frames.last().is_some_and(|l| *l != id),
            chain,
            autofit_none: autofit == Autofit::None,
            linking: self.view.link_from == Some(id),
        })
    }

    /// Runs one Text flow action on `id`. Each is one undo step (or none: linking and navigation change the
    /// view only until the link is made).
    pub(crate) fn run_text_flow(&mut self, id: Id, action: TextFlowAction) {
        match action {
            TextFlowAction::Autoflow => {
                self.act(SessionAction::Autoflow { frame: id });
            }
            TextFlowAction::Shrink => {
                // From any box of the chain, the setting goes to the box the layout reads it from.
                let id = autofit_frame(self.session.doc(), id);
                let patch = TextFramePatch { autofit: Some(Autofit::ShrinkOnOverflow), ..Default::default() };
                self.act(Command::SetTextFrame { id, patch });
            }
            TextFlowAction::Link => {
                if self.view.link_from == Some(id) {
                    self.cancel_link();
                } else {
                    self.start_link(id);
                }
            }
            TextFlowAction::BreakLink => {
                self.act(Command::UnlinkFrame { frame: id });
            }
            TextFlowAction::Previous => self.go_to_box(id, false, false),
            TextFlowAction::Next => self.go_to_box(id, true, false),
        }
    }

    /// The "Text flow" section of a right-click menu for text box `id`. Returns true when an entry was chosen
    /// and the menu should close.
    pub(crate) fn text_flow_menu(&mut self, ui: &mut egui::Ui, id: Id) -> bool {
        let Some(st) = self.text_flow_state(id) else { return false };
        widgets::hint(ui, "Text flow");
        let chosen = text_flow_entries(&st).iter().find(|e| e.button(ui)).map(|e| e.action);
        if let Some(action) = chosen {
            self.run_text_flow(id, action);
        }
        chosen.is_some()
    }
}

/// Centres of the Previous (inside the top-left corner) and Next (inside the bottom-right corner) chips of a box,
/// clear of its corner resize handles and of the overflow badge. They stay inside the box so they never cover a
/// neighbouring object.
fn chips(rr: ERect) -> (Pos2, Pos2) {
    (Pos2::new(rr.left() + 18.0, rr.top() + 18.0), Pos2::new(rr.right() - 18.0, rr.bottom() - 18.0))
}
