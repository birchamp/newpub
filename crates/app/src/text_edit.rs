//! Text editing on the canvas (UI-14): a caret and a selection inside a story, placed with the mouse, moved with
//! the keyboard, and drawn from the layout's glyph positions. Typing, deleting and formatting work at the caret.
//!
//! The caret lives in an *editable*: a text frame, a shape that holds text (UI-19), or a table cell, named by its
//! story id. Commands go to the story; the selection holds the object on the page (frame, shape or table).

use crate::NewpubApp;
use egui::{Color32, Pos2, Rect as ERect, Stroke};
use newpub_engine::SessionAction;
use newpub_engine::core::{Command, Id, ObjectKind};
use newpub_engine::layout::Line;
use std::ops::Range;

/// A caret in a story. `anchor` is the other end of the selection (equal to `pos` when none).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Caret {
    /// The editable the caret was placed in: a frame of the story, a shape, or a table cell's story id.
    pub frame: Id,
    pub pos: usize,
    pub anchor: usize,
    /// Horizontal position kept while moving up and down, page points.
    pub goal_x: Option<f64>,
}

impl Caret {
    pub fn at(frame: Id, pos: usize) -> Caret {
        Caret { frame, pos, anchor: pos, goal_x: None }
    }

    pub fn range(&self) -> Range<usize> {
        self.pos.min(self.anchor)..self.pos.max(self.anchor)
    }

    pub fn has_selection(&self) -> bool {
        self.pos != self.anchor
    }
}

/// One laid-out line of a story: the layout entry it belongs to (`key`), the object on the page that shows it
/// (`object`), and the page position line coordinates are relative to.
struct StoryLine {
    key: Id,
    object: Id,
    origin: (f64, f64),
    line: Line,
}

impl NewpubApp {
    /// The story of an editable (text frame, shape with text, or a story id itself).
    pub(crate) fn story_id(&self, id: Id) -> Option<Id> {
        let d = self.session.doc();
        if d.stories.contains_key(&id) {
            return Some(id);
        }
        match &d.objects.get(&id)?.kind {
            ObjectKind::Text(t) => Some(t.story),
            ObjectKind::Shape(s) => s.story,
            _ => None,
        }
    }

    /// The table holding a cell story, with the table's id.
    pub(crate) fn cell_table(&self, story: Id) -> Option<(Id, usize, usize)> {
        let d = self.session.doc();
        d.objects.values().find_map(|o| match &o.kind {
            ObjectKind::Table(t) => (0..t.rows()).find_map(|r| {
                (0..t.cols()).find_map(|c| {
                    t.cell(r, c).filter(|cell| cell.story == story && !cell.covered).map(|_| (o.id, r, c))
                })
            }),
            _ => None,
        })
    }

    /// The object on the page that shows an editable: the frame or shape itself, or a cell's table.
    pub(crate) fn holder(&self, id: Id) -> Id {
        if self.session.doc().objects.contains_key(&id) { id } else { self.cell_table(id).map_or(id, |t| t.0) }
    }

    fn story_len_of(&self, frame: Id) -> usize {
        let d = self.session.doc();
        self.story_id(frame).and_then(|s| d.story(s).ok()).map(|s| s.len()).unwrap_or(0)
    }

    fn story_text_of(&self, frame: Id) -> String {
        let d = self.session.doc();
        self.story_id(frame).and_then(|s| d.story(s).ok()).map(|s| s.text.clone()).unwrap_or_default()
    }

    /// The caret, if it belongs to `frame`'s story.
    pub(crate) fn caret_in(&self, frame: Id) -> Option<Caret> {
        let c = self.caret?;
        (self.story_id(c.frame)? == self.story_id(frame)?).then_some(c)
    }

    /// What typing and text commands act on: the caret's editable while editing, else the one selected text
    /// frame. With several objects selected and no caret, nothing: typing or Backspace must not reach a box
    /// the person did not pick out.
    pub(crate) fn edit_target(&self) -> Option<Id> {
        match self.caret {
            Some(c) if self.selection.contains(&self.holder(c.frame)) => Some(c.frame),
            _ if self.selection.len() == 1 => self.selected_text_frame(),
            _ => None,
        }
    }

    /// Range of the selected text in the edited story, for formatting commands: `(Some(a), Some(b))` with a
    /// selection, `(None, None)` (the whole story) without one.
    pub(crate) fn text_target_range(&self, frame: Id) -> (Option<usize>, Option<usize>) {
        match self.caret_in(frame) {
            Some(c) if c.has_selection() => (Some(c.range().start), Some(c.range().end)),
            _ => (None, None),
        }
    }

    /// The paragraphs paragraph formatting applies to: those the caret or selection touches while editing,
    /// else (None, None) for the whole story.
    pub(crate) fn para_target_range(&self, frame: Id) -> (Option<usize>, Option<usize>) {
        match self.caret_in(frame) {
            Some(c) => (Some(c.range().start), Some(c.range().end)),
            None => (None, None),
        }
    }

    /// Where inserted material (fields, special characters) goes: the caret, else the end of the story.
    pub(crate) fn insertion_point(&self, frame: Id) -> usize {
        self.caret_in(frame).map(|c| c.range().start).unwrap_or_else(|| self.story_len_of(frame))
    }

    /// Lines of the story of `frame`, in story order across its frames.
    fn story_lines(&mut self, frame: Id) -> Vec<StoryLine> {
        let layout = self.session.layout();
        let Some(sid) = self.story_id(frame) else { return vec![] };
        let d = self.session.doc();
        let Some(frames) = d.story(sid).ok().map(|s| s.frames.clone()) else { return vec![] };
        let mut out = vec![];
        if frames.is_empty() {
            // A table cell: laid out under its story id, in coordinates relative to the table.
            if let Some((table, _, _)) = self.cell_table(sid)
                && let (Some(fl), Some(o)) = (layout.frames.get(&sid), d.objects.get(&table))
            {
                for l in &fl.lines {
                    out.push(StoryLine { key: sid, object: table, origin: (o.rect.x, o.rect.y), line: l.clone() });
                }
            }
            return out;
        }
        for f in frames {
            let (Some(fl), Some(o)) = (layout.frames.get(&f), d.objects.get(&f)) else { continue };
            for l in &fl.lines {
                out.push(StoryLine { key: f, object: f, origin: (o.rect.x, o.rect.y), line: l.clone() });
            }
        }
        out
    }

    /// Frame-local x of the caret position `pos` on `line`.
    fn x_in_line(line: &Line, pos: usize) -> f64 {
        let mut x = None::<f64>;
        let mut end = line.x;
        for r in &line.runs {
            for g in r.glyphs.iter().filter(|g| !g.generated) {
                if g.char_index == pos && x.is_none_or(|v| g.x < v) {
                    x = Some(g.x);
                }
                if g.char_index < pos {
                    end = end.max(g.x + g.advance);
                }
            }
        }
        x.unwrap_or(end)
    }

    /// Index into `lines` of the line showing `pos`.
    fn line_of(lines: &[StoryLine], pos: usize) -> Option<usize> {
        lines
            .iter()
            .position(|l| l.line.char_range.start <= pos && pos < l.line.char_range.end)
            .or_else(|| lines.iter().rposition(|l| l.line.char_range.end == pos || l.line.char_range.start == pos))
    }

    /// Caret geometry for `pos`: frame, page x, top y, height (page points).
    fn caret_geom(&mut self, frame: Id, pos: usize) -> Option<(Id, f64, f64, f64)> {
        let lines = self.story_lines(frame);
        let i = Self::line_of(&lines, pos)?;
        let l = &lines[i];
        let x = Self::x_in_line(&l.line, pos);
        Some((l.object, l.origin.0 + x, l.origin.1 + l.line.top, l.line.height))
    }

    /// Story position nearest to a frame-local point on one line.
    fn pos_in_line(line: &Line, x: f64) -> usize {
        let mut glyphs: Vec<(f64, f64, usize)> = line
            .runs
            .iter()
            .flat_map(|r| r.glyphs.iter())
            .filter(|g| !g.generated)
            .map(|g| (g.x, g.advance, g.char_index))
            .collect();
        glyphs.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (gx, adv, ci) in &glyphs {
            if x < gx + adv / 2.0 {
                return *ci;
            }
        }
        // Past the last glyph: the end of the line.
        line.char_range.end
    }

    /// Story position under a page point inside `frame` (a text frame, a shape, or a cell's story).
    pub(crate) fn hit_text(&mut self, frame: Id, x: f64, y: f64) -> usize {
        let lines: Vec<StoryLine> = self.story_lines(frame).into_iter().filter(|l| l.key == frame).collect();
        if lines.is_empty() {
            return self.story_len_of(frame);
        }
        let (ox, oy) = lines[0].origin;
        let (lx, ly) = (x - ox, y - oy);
        // The row under the point (pieces of one row share their top), else the nearest row.
        let dist = |l: &Line| {
            if ly < l.top {
                l.top - ly
            } else if ly > l.top + l.height {
                ly - (l.top + l.height)
            } else {
                0.0
            }
        };
        let best = lines.iter().map(|l| dist(&l.line)).fold(f64::INFINITY, f64::min);
        let row: Vec<&StoryLine> = lines.iter().filter(|l| dist(&l.line) <= best + 1e-6).collect();
        let piece = row
            .iter()
            .min_by(|a, b| {
                let da = (lx - a.line.x).max(0.0).max(a.line.x - lx).min((lx - (a.line.x + a.line.width)).abs());
                let db = (lx - b.line.x).max(0.0).max(b.line.x - lx).min((lx - (b.line.x + b.line.width)).abs());
                da.total_cmp(&db)
            })
            .copied()
            .unwrap_or(row[0]);
        let pos = Self::pos_in_line(&piece.line, lx);
        pos.clamp(piece.line.char_range.start, piece.line.char_range.end.max(piece.line.char_range.start))
    }

    /// Starts editing `frame` with the caret at `pos` (selecting the frame).
    pub(crate) fn place_caret(&mut self, frame: Id, pos: usize, extend: bool) {
        let anchor = match self.caret_in(frame) {
            Some(c) if extend => c.anchor,
            _ => pos,
        };
        self.caret = Some(Caret { frame, pos, anchor, goal_x: None });
        self.view.cell_extent = None;
        let holder = self.holder(frame);
        self.selection = vec![holder];
        self.view.editing = Some(holder);
    }

    /// Ends text editing (the frame stays selected).
    pub(crate) fn end_text_edit(&mut self) {
        self.caret = None;
        self.view.editing = None;
    }

    /// Drops a caret that no longer fits its story or whose frame is gone.
    pub(crate) fn validate_caret(&mut self) {
        let Some(c) = self.caret else { return };
        if !self.selection.contains(&self.holder(c.frame)) || self.story_id(c.frame).is_none() {
            self.caret = None;
            return;
        }
        let len = self.story_len_of(c.frame);
        if c.pos > len || c.anchor > len {
            self.caret = Some(Caret { pos: c.pos.min(len), anchor: c.anchor.min(len), ..c });
        }
    }

    /// Selects the word around `pos`.
    pub(crate) fn select_word(&mut self, frame: Id, pos: usize) {
        let chars: Vec<char> = self.story_text_of(frame).chars().collect();
        let word = |c: char| c.is_alphanumeric() || c == '\'' || c == '’';
        let mut a = pos.min(chars.len());
        let mut b = a;
        while a > 0 && word(chars[a - 1]) {
            a -= 1;
        }
        while b < chars.len() && word(chars[b]) {
            b += 1;
        }
        self.caret = Some(Caret { frame, pos: b, anchor: a, goal_x: None });
        let holder = self.holder(frame);
        self.selection = vec![holder];
        self.view.editing = Some(holder);
    }

    /// Types `text` at the caret of `frame`, replacing the selection. Without a caret the text goes at the end.
    pub(crate) fn type_at_caret(&mut self, frame: Id, text: &str) {
        let c = self.caret_in(frame).unwrap_or_else(|| Caret::at(frame, self.story_len_of(frame)));
        let Some(target) = self.story_id(frame) else { return };
        let n = text.chars().count();
        let r = c.range();
        let ok = if c.has_selection() {
            self.act(Command::ReplaceText { target, start: r.start, end: r.end, text: text.to_string() })
        } else {
            self.act(SessionAction::TypeText { target, at: Some(r.start), text: text.to_string() })
        };
        if ok.is_some() {
            self.caret = Some(Caret::at(c.frame, r.start + n));
            self.view.editing = Some(self.holder(frame));
        }
    }

    /// Backspace (`forward` = false) or Delete at the caret.
    pub(crate) fn delete_at_caret(&mut self, frame: Id, forward: bool) {
        let len = self.story_len_of(frame);
        let c = self.caret_in(frame).unwrap_or_else(|| Caret::at(frame, len));
        let r = if c.has_selection() {
            c.range()
        } else if forward {
            c.pos..(c.pos + 1).min(len)
        } else {
            c.pos.saturating_sub(1)..c.pos
        };
        if r.is_empty() {
            return;
        }
        let Some(target) = self.story_id(frame) else { return };
        if self.act(Command::DeleteText { target, start: r.start, end: r.end }).is_some() {
            self.caret = Some(Caret::at(c.frame, r.start));
        }
    }

    /// The selected text of the edited story.
    pub(crate) fn selected_text(&self) -> Option<String> {
        let c = self.caret?;
        if !c.has_selection() {
            return None;
        }
        let r = c.range();
        Some(self.story_text_of(c.frame).chars().skip(r.start).take(r.len()).collect())
    }

    /// Keyboard caret movement. Returns true when the key was used.
    pub(crate) fn caret_key(&mut self, key: egui::Key, m: egui::Modifiers) -> bool {
        use egui::Key;
        let Some(c) = self.caret else { return false };
        let len = self.story_len_of(c.frame);
        let chars: Vec<char> = self.story_text_of(c.frame).chars().collect();
        let word_left = |mut p: usize| {
            while p > 0 && !chars[p - 1].is_alphanumeric() {
                p -= 1;
            }
            while p > 0 && chars[p - 1].is_alphanumeric() {
                p -= 1;
            }
            p
        };
        let word_right = |mut p: usize| {
            while p < chars.len() && !chars[p].is_alphanumeric() {
                p += 1;
            }
            while p < chars.len() && chars[p].is_alphanumeric() {
                p += 1;
            }
            p
        };
        let by_word = m.alt || (m.ctrl && !m.mac_cmd);
        let mut goal = None;
        let pos = match key {
            Key::ArrowLeft if c.has_selection() && !m.shift => c.range().start,
            Key::ArrowRight if c.has_selection() && !m.shift => c.range().end,
            Key::ArrowLeft if by_word => word_left(c.pos),
            Key::ArrowRight if by_word => word_right(c.pos),
            Key::ArrowLeft if m.mac_cmd => self.line_edge(c, false),
            Key::ArrowRight if m.mac_cmd => self.line_edge(c, true),
            Key::ArrowLeft => c.pos.saturating_sub(1),
            Key::ArrowRight => (c.pos + 1).min(len),
            Key::Home if m.command => 0,
            Key::End if m.command => len,
            Key::Home => self.line_edge(c, false),
            Key::End => self.line_edge(c, true),
            Key::ArrowUp | Key::ArrowDown => {
                let (p, g) = self.vertical(c, key == Key::ArrowDown);
                goal = g;
                p
            }
            _ => return false,
        };
        let anchor = if m.shift { c.anchor } else { pos };
        self.caret = Some(Caret { frame: c.frame, pos, anchor, goal_x: goal });
        true
    }

    /// Start (`end` = false) or end of the caret's line.
    fn line_edge(&mut self, c: Caret, end: bool) -> usize {
        let lines = self.story_lines(c.frame);
        match Self::line_of(&lines, c.pos) {
            Some(i) => {
                let r = &lines[i].line.char_range;
                if end {
                    // Before the space that ends a wrapped line, so the caret stays on this line.
                    let text: Vec<char> = self.story_text_of(c.frame).chars().collect();
                    if r.end > r.start && text.get(r.end - 1).is_some_and(|ch| ch.is_whitespace()) {
                        r.end - 1
                    } else {
                        r.end
                    }
                } else {
                    r.start
                }
            }
            None => c.pos,
        }
    }

    /// Caret position one line down or up, keeping the goal x.
    fn vertical(&mut self, c: Caret, down: bool) -> (usize, Option<f64>) {
        let lines = self.story_lines(c.frame);
        let Some(i) = Self::line_of(&lines, c.pos) else { return (c.pos, c.goal_x) };
        let cur = &lines[i];
        let goal = c.goal_x.unwrap_or(cur.origin.0 + Self::x_in_line(&cur.line, c.pos));
        // The next row: the nearest line whose top differs (wrap pieces share a row).
        let target = if down {
            lines[i + 1..].iter().find(|l| (l.line.top - cur.line.top).abs() > 0.5 || l.key != cur.key)
        } else {
            lines[..i].iter().rev().find(|l| (l.line.top - cur.line.top).abs() > 0.5 || l.key != cur.key)
        };
        match target {
            Some(t) => (
                Self::pos_in_line(&t.line, goal - t.origin.0).clamp(t.line.char_range.start, t.line.char_range.end),
                Some(goal),
            ),
            None if down => (self.story_len_of(c.frame), Some(goal)),
            None => (0, Some(goal)),
        }
    }

    /// Paints the selection highlight and the blinking caret of the edited story on the current page.
    pub(crate) fn draw_text_edit(&mut self, painter: &egui::Painter) {
        let Some(c) = self.caret else { return };
        let page_objects: Vec<Id> =
            self.session.doc().pages.get(self.page).map(|p| p.objects.clone()).unwrap_or_default();
        let lines = self.story_lines(c.frame);
        let accent = crate::theme::VENICE;
        if c.has_selection() {
            let r = c.range();
            for l in lines.iter().filter(|l| page_objects.contains(&l.object)) {
                let cr = &l.line.char_range;
                let (a, b) = (r.start.max(cr.start), r.end.min(cr.end));
                if a >= b && !(cr.start == cr.end && r.contains(&cr.start)) {
                    continue;
                }
                let x0 = Self::x_in_line(&l.line, a);
                let x1 = if b >= cr.end {
                    Self::x_in_line(&l.line, cr.end).max(x0 + 4.0)
                } else {
                    Self::x_in_line(&l.line, b)
                };
                let p0 = self.page_to_screen(l.origin.0 + x0, l.origin.1 + l.line.top);
                let p1 = self.page_to_screen(l.origin.0 + x1, l.origin.1 + l.line.top + l.line.height);
                painter.rect_filled(ERect::from_min_max(p0, p1), 0.0, Color32::from_rgba_unmultiplied(8, 80, 120, 60));
            }
        }
        let blink = painter.ctx().input(|i| i.time);
        painter.ctx().request_repaint_after(std::time::Duration::from_millis(530));
        if (blink * 1000.0 / 530.0) as i64 % 2 == 1 && !c.has_selection() {
            return;
        }
        if let Some((f, x, y, h)) = self.caret_geom(c.frame, c.pos)
            && page_objects.contains(&f)
        {
            let a = self.page_to_screen(x, y);
            let b = self.page_to_screen(x, y + h);
            painter.line_segment([a, b], Stroke::new(1.6, accent));
        }
    }

    /// The cell of `table` under a page point: (cell story, row, column).
    pub(crate) fn cell_at(&self, table: Id, x: f64, y: f64) -> Option<(Id, usize, usize)> {
        let o = self.session.doc().objects.get(&table)?;
        let ObjectKind::Table(t) = &o.kind else { return None };
        let (lx, ly) = (x - o.rect.x, y - o.rect.y);
        for r in 0..t.rows() {
            for c in 0..t.cols() {
                let (Some(cell), Some(rect)) = (t.cell(r, c), t.cell_rect(r, c)) else { continue };
                if !cell.covered && rect.contains(lx, ly) {
                    return Some((cell.story, r, c));
                }
            }
        }
        None
    }

    /// The editable under a page point inside an edited object: the frame or shape itself, or a table's cell.
    pub(crate) fn editable_at(&self, holder: Id, x: f64, y: f64) -> Option<Id> {
        let o = self.session.doc().objects.get(&holder)?;
        match &o.kind {
            ObjectKind::Text(_) => o.rect.contains(x, y).then_some(holder),
            ObjectKind::Shape(s) => (s.story.is_some() && o.rect.contains(x, y)).then_some(holder),
            ObjectKind::Table(_) => self.cell_at(holder, x, y).map(|c| c.0),
            _ => None,
        }
    }

    /// Mouse handling inside the object being edited: press places the caret (Shift extends), drag selects,
    /// double-click selects a word. Returns true when the pointer interaction was a text interaction.
    pub(crate) fn text_pointer(&mut self, resp: &egui::Response, press: Option<Pos2>) -> bool {
        let Some(holder) = self.view.editing.filter(|f| self.selection.contains(f)) else { return false };
        let shift = resp.ctx.input(|i| i.modifiers.shift);
        if resp.double_clicked()
            && let Some(p) = resp.interact_pointer_pos()
        {
            let (x, y) = self.screen_to_page(p);
            if let Some(ed) = self.editable_at(holder, x, y) {
                // A quick Shift-click onto another cell still extends the cell selection.
                if !(shift && self.extend_cell_selection(ed)) {
                    let pos = self.hit_text(ed, x, y);
                    self.select_word(ed, pos);
                }
                return true;
            }
        }
        let Some(p) = press else { return false };
        let (px, py) = self.screen_to_page(p);
        let Some(ed) = self.editable_at(holder, px, py) else { return false };
        if resp.drag_started() || resp.clicked() || resp.is_pointer_button_down_on() {
            let pos = self.hit_text(ed, px, py);
            if (resp.drag_started() || resp.clicked()) && shift && self.extend_cell_selection(ed) {
                return true;
            }
            if resp.drag_started() || resp.clicked() {
                // Shift extends only within the same story.
                let extend = shift && self.caret_in(ed).is_some();
                self.place_caret(ed, pos, extend);
            }
        }
        if resp.dragged()
            && let Some(now) = resp.interact_pointer_pos()
        {
            let (x, y) = self.screen_to_page(now);
            let under = self.editable_at(holder, x, y);
            if under.is_some_and(|u| u != ed) && under.is_some_and(|u| self.extend_cell_selection(u)) {
                // Dragging from one cell into another selects a block of cells.
            } else if under == Some(ed) {
                let pos = self.hit_text(ed, x, y);
                if let Some(c) = self.caret.as_mut() {
                    c.pos = pos;
                }
            }
        }
        true
    }
}
