//! Clipboard and object commands shared by shortcuts and the canvas context menu (UI-15): cut, copy and paste of
//! text (through the OS clipboard) and of objects (through the session clipboard), select all, group, ungroup.

use crate::NewpubApp;
use newpub_engine::SessionAction;
use newpub_engine::core::{CharAttrs, Command, Rect};

impl NewpubApp {
    /// Copy (or cut) the selected text while editing, else the selected objects.
    pub(crate) fn copy(&mut self, ctx: &egui::Context, cut: bool) {
        if let Some(frame) = self.edit_target().filter(|f| self.caret_in(*f).is_some()) {
            if let Some(text) = self.selected_text() {
                ctx.copy_text(text);
                self.clip_objects = None;
                if cut {
                    self.delete_at_caret(frame, false);
                }
            }
            return;
        }
        let ids = self.selection.clone();
        if ids.is_empty() {
            return;
        }
        let action = if cut {
            SessionAction::CutObjects { ids: ids.clone() }
        } else {
            SessionAction::CopyObjects { ids: ids.clone() }
        };
        if self.act(action).is_some() {
            // The OS clipboard gets a marker, so a later paste knows nothing else was copied since.
            let marker = format!("newpub: {} object{}", ids.len(), if ids.len() == 1 { "" } else { "s" });
            ctx.copy_text(marker.clone());
            self.clip_objects = Some(marker);
            if cut {
                self.selection.clear();
            }
            let n = ids.len();
            self.status = format!("{} {n} object{}", if cut { "Cut" } else { "Copied" }, if n == 1 { "" } else { "s" });
        }
    }

    /// Paste: text into the edited story at the caret; objects from the session clipboard; otherwise plain text
    /// from the OS clipboard becomes a new text box. `os_text` is the OS clipboard text when the platform sent it.
    pub(crate) fn paste(&mut self, os_text: Option<&str>) {
        if let Some(frame) = self.edit_target().filter(|f| self.caret_in(*f).is_some()) {
            if let Some(t) = os_text.filter(|t| !t.is_empty()) {
                let t = t.replace("\r\n", "\n");
                self.type_at_caret(frame, &t);
            }
            return;
        }
        let ours = match (os_text, &self.clip_objects) {
            (None, _) => true,
            (Some(t), Some(marker)) => t == marker,
            (Some(_), None) => false,
        };
        if ours {
            let page = Some(self.page);
            if let Some(o) = self.act(SessionAction::PasteObjects { page, x: None, y: None }) {
                self.selection = o.created;
                self.caret = None;
            }
            return;
        }
        let Some(text) = os_text.filter(|t| !t.trim().is_empty()) else { return };
        // A new text box in the middle of the margins holding the pasted text.
        let d = self.session.doc();
        let (t, b, l, r) = d.page_margins(self.page);
        let (w, h) = (d.setup.width.0 - l - r, d.setup.height.0 - t - b);
        let rect = Rect::new(l + w * 0.15, t + h * 0.3, w * 0.7, h * 0.25);
        let page = Some(self.page);
        if let Some(id) = self
            .act(Command::AddTextFrame { page, master: None, rect, columns: None, gutter: None })
            .and_then(|o| o.created.first().copied())
        {
            self.act(Command::InsertText { target: id, at: None, text: text.replace("\r\n", "\n"), attrs: None });
            self.selection = vec![id];
        }
    }

    /// Select all: the whole story while editing, else every object on the page.
    pub(crate) fn select_all(&mut self) {
        if let Some(frame) = self.edit_target().filter(|f| self.caret_in(*f).is_some()) {
            let len = self.story_id(frame).and_then(|s| self.session.doc().story(s).ok()).map(|s| s.len());
            if let (Some(len), Some(c)) = (len, self.caret.as_mut()) {
                c.anchor = 0;
                c.pos = len;
            }
            return;
        }
        self.caret = None;
        self.selection = self.session.doc().pages.get(self.page).map(|p| p.objects.clone()).unwrap_or_default();
    }

    pub(crate) fn group_selection(&mut self) {
        if self.selection.len() < 2 {
            return;
        }
        let ids = self.selection.clone();
        if let Some(o) = self.act(Command::Group { ids }) {
            self.selection = o.created;
        }
    }

    pub(crate) fn ungroup_selection(&mut self) {
        let Some(id) = self.selection.first().copied() else { return };
        if let Some(o) = self.act(Command::Ungroup { id }) {
            self.selection = o.created;
        }
    }

    /// Cmd+B / Cmd+I / Cmd+U while editing: toggles bold, italic or underline on the selection (or the story).
    pub(crate) fn toggle_char_style(&mut self, key: egui::Key) {
        let Some(frame) = self.edit_target() else { return };
        let Some(target) = self.story_id(frame) else { return };
        let (start, end) = self.text_target_range(frame);
        let at = start.unwrap_or(0);
        let Ok(v) = self.session.query(&newpub_engine::Query::CharAttrs { target, at }) else { return };
        let on = |k: &str| v.get(k).and_then(|b| b.as_bool()).unwrap_or(false);
        let mut attrs = CharAttrs::default();
        match key {
            egui::Key::B => attrs.bold = Some(!on("bold")),
            egui::Key::I => attrs.italic = Some(!on("italic")),
            _ => attrs.underline = Some(!on("underline")),
        }
        self.act(Command::FormatChars { target, start, end, attrs });
    }
}

impl NewpubApp {
    /// Right-click menu of the canvas.
    pub(crate) fn canvas_menu(&mut self, ui: &mut egui::Ui) {
        use crate::{icons as ic, widgets::small_button};
        let ctx = ui.ctx().clone();
        let has = !self.selection.is_empty();
        let editing = self.edit_target().filter(|f| self.caret_in(*f).is_some()).is_some();
        let mut close = false;
        if small_button(ui, ic::SCISSORS, "Cut", false, has).clicked() {
            self.copy(&ctx, true);
            close = true;
        }
        if small_button(ui, ic::COPY, "Copy", false, has).clicked() {
            self.copy(&ctx, false);
            close = true;
        }
        if small_button(ui, ic::CLIPBOARD, "Paste", false, true).clicked() {
            self.paste(None);
            close = true;
        }
        // The Text flow entries of the one selected text box (UI-30): the right-click handler has already made
        // the box under the pointer the selection.
        let only = if self.selection.len() == 1 { self.selection.first().copied() } else { None };
        let flow_box = only.filter(|id| self.text_flow_state(*id).is_some());
        if !editing {
            if small_button(ui, ic::COPY_SIMPLE, "Duplicate", false, has).clicked() {
                self.duplicate_selection();
                close = true;
            }
            if let Some(id) = flow_box {
                ui.separator();
                close |= self.text_flow_menu(ui, id);
            }
            ui.separator();
            if small_button(ui, ic::STACK_PLUS, "Bring to Front", false, has).clicked() {
                self.reorder(newpub_engine::core::ZOp::Front);
                close = true;
            }
            if small_button(ui, ic::STACK_MINUS, "Send to Back", false, has).clicked() {
                self.reorder(newpub_engine::core::ZOp::Back);
                close = true;
            }
            if self.selection.len() >= 2 && small_button(ui, ic::SELECTION_PLUS, "Group", false, true).clicked() {
                self.group_selection();
                close = true;
            }
            let grouped = self.selection.len() == 1
                && self
                    .selection
                    .first()
                    .and_then(|id| self.session.doc().objects.get(id))
                    .is_some_and(|o| matches!(o.kind, newpub_engine::core::ObjectKind::Group { .. }));
            if grouped && small_button(ui, ic::SELECTION_SLASH, "Ungroup", false, true).clicked() {
                self.ungroup_selection();
                close = true;
            }
            ui.separator();
            if small_button(ui, ic::SELECTION_ALL, "Select All", false, true).clicked() {
                self.select_all();
                close = true;
            }
            if has && small_button(ui, ic::TRASH, "Delete", false, true).clicked() {
                let ids = std::mem::take(&mut self.selection);
                self.act(Command::DeleteObjects { ids });
                close = true;
            }
        } else {
            if small_button(ui, ic::SELECTION_ALL, "Select All Text", false, true).clicked() {
                self.select_all();
                close = true;
            }
            if let Some(id) = flow_box {
                ui.separator();
                close |= self.text_flow_menu(ui, id);
            }
        }
        if close {
            ui.close();
        }
    }
}
