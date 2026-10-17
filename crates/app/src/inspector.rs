//! The right-hand inspector: sectioned cards for the formatting of the selected text and the properties of the
//! selected objects. Labels sit left in muted text, inputs share one right-aligned column.

use crate::{NewpubApp, fmt_len, fmt_num, icons, widgets};
use egui::{Align, Layout, RichText};
use newpub_engine::core::{
    self as core, Align as TextAlign, AlignEdge, AlignTo, Autofit, Axis, Baseline, Caps, CharAttrs, Color, Command,
    Dash, Fit, Id, Insets, Length, LineSpacing, ListStyle, Object, ObjectKind, ObjectPatch, ParaAttrs, ShapePatch,
    StyleRef, TextFramePatch, VAlign, Wrap, WrapMode, ZOp,
};
use std::collections::HashMap;

/// Text buffers of the inspector fields that are not geometry (owned by `Fields`, reset when the selection changes).
#[derive(Default)]
pub(crate) struct InspectorFields {
    bufs: HashMap<&'static str, String>,
}

fn buf<'a>(f: &'a mut InspectorFields, key: &'static str) -> &'a mut String {
    f.bufs.entry(key).or_default()
}

/// Width of the right-hand input column.
const INPUT_W: f32 = 84.0;

/// Labels of the Text box card's continued-notice checkboxes (Publisher's wording).
const CONTINUED_ON_LABEL: &str = "Include 'Continued on page…'";
const CONTINUED_FROM_LABEL: &str = "Include 'Continued from page…'";

/// A labelled row: muted label left, the input right-aligned. `add` receives the label's id for `labelled_by`.
fn row<R>(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui, egui::Id) -> R) -> R {
    let p = widgets::pal(ui);
    ui.horizontal(|ui| {
        ui.set_min_height(22.0);
        let l = ui.label(RichText::new(label).size(12.0).color(p.text_muted));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| add(ui, l.id)).inner
    })
    .inner
}

/// A single-line text field row. Returns the text when the user edited it this frame; while not focused it
/// shows `model`.
fn field(ui: &mut egui::Ui, text: &mut String, label: &str, model: &str) -> Option<String> {
    row(ui, label, |ui, id| {
        let r = ui.add(egui::TextEdit::singleline(text).desired_width(INPUT_W)).labelled_by(id);
        if r.changed() {
            Some(text.clone())
        } else {
            if !r.has_focus() {
                *text = model.to_string();
            }
            None
        }
    })
}

/// A drop-down row; returns the chosen index.
fn choice(ui: &mut egui::Ui, label: &str, current: &str, items: &[&str]) -> Option<usize> {
    row(ui, label, |ui, _| {
        let mut picked = None;
        let c = egui::ComboBox::from_id_salt(label).width(INPUT_W - 16.0).selected_text(current).show_ui(ui, |ui| {
            for (i, it) in items.iter().enumerate() {
                if ui.selectable_label(*it == current, *it).clicked() {
                    picked = Some(i);
                }
            }
        });
        let l = label.to_string();
        c.response.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, &l));
        picked
    })
}

/// A length field row kept in the panel's field buffers under `key`; returns the new length in points once it
/// parses.
pub(crate) fn length_field(
    app: &mut NewpubApp,
    ui: &mut egui::Ui,
    key: &'static str,
    label: &str,
    model: &str,
) -> Option<f64> {
    field(ui, buf(&mut app.fields.more, key), label, model).and_then(|t| core::units::parse_length(t.trim()))
}

pub(crate) fn choice_row(ui: &mut egui::Ui, label: &str, current: &str, items: &[&str]) -> Option<usize> {
    choice(ui, label, current, items)
}

/// A colour field row kept in the panel's field buffers under `key`.
pub(crate) fn color_row(
    app: &mut NewpubApp,
    ui: &mut egui::Ui,
    key: &'static str,
    label: &str,
    current: Option<&Color>,
) -> Option<Color> {
    let mut t = std::mem::take(buf(&mut app.fields.more, key));
    let r = color_field(ui, app.session.doc(), &mut t, label, current);
    *buf(&mut app.fields.more, key) = t;
    r
}

fn rgba_of(doc: &core::Document, c: &Color) -> [u8; 4] {
    doc.scheme_color(c).to_rgba8()
}

fn hex_of(doc: &core::Document, c: &Color) -> String {
    let [r, g, b, _] = rgba_of(doc, c);
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn fmt_pt(v: f64) -> String {
    format!("{}pt", fmt_num(v))
}

/// Colour presets: scheme colours first (they follow the publication's scheme), then fixed colours.
fn presets() -> Vec<(&'static str, Color)> {
    use core::schemes::SchemeSlot as S;
    let s = |slot| Color::Scheme { slot, a: 1.0 };
    vec![
        ("Scheme text", s(S::Main)),
        ("Scheme accent 1", s(S::Accent1)),
        ("Scheme accent 2", s(S::Accent2)),
        ("Scheme accent 3", s(S::Accent3)),
        ("Black", Color::BLACK),
        ("White", Color::WHITE),
        ("Red", Color::rgb(0xcc, 0x00, 0x00)),
        ("Orange", Color::rgb(0xff, 0x88, 0x00)),
        ("Green", Color::rgb(0x2e, 0x9e, 0x4f)),
        ("Blue", Color::rgb(0x1f, 0x6f, 0xd0)),
    ]
}

/// A colour row (preview swatch + hex field) with a strip of preset swatches below. Returns a colour when the
/// user typed a valid one or clicked a preset.
fn color_field(
    ui: &mut egui::Ui,
    doc: &core::Document,
    text: &mut String,
    label: &str,
    current: Option<&Color>,
) -> Option<Color> {
    let model = current.map(|c| hex_of(doc, c)).unwrap_or_default();
    let mut out = None;
    row(ui, label, |ui, id| {
        let r = ui.add(egui::TextEdit::singleline(text).desired_width(INPUT_W).hint_text("none")).labelled_by(id);
        if r.changed() {
            out = Color::parse(text.trim());
        } else if !r.has_focus() {
            *text = model.clone();
        }
        let sw = widgets::swatch(ui, current.map(|c| rgba_of(doc, c)), 20.0, &format!("{label} presets"), false, true);
        egui::Popup::menu(&sw).show(|ui| {
            ui.set_min_width(150.0);
            let cur_hex = (!model.is_empty()).then_some(model.as_str());
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(5.0, 5.0);
                ui.set_max_width(150.0);
                for (name, c) in presets() {
                    let hex = hex_of(doc, &c);
                    let name = format!("{label} preset {name}");
                    let sel = cur_hex == Some(hex.as_str());
                    if widgets::swatch(ui, Some(rgba_of(doc, &c)), 22.0, &name, sel, true).clicked() {
                        out = Some(c);
                    }
                }
            });
        });
    });
    out
}

/// A `widgets::section` card with the compact 4px row rhythm inside.
fn card<R>(ui: &mut egui::Ui, icon: &str, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    widgets::section(ui, icon, title, |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        add(ui)
    })
}

fn toggle_row(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
        add(ui);
    });
}

impl NewpubApp {
    pub(crate) fn format_panel(&mut self, ui: &mut egui::Ui) {
        let first = self.selection.first().copied();
        if self.fields.owner != first {
            self.fields = crate::Fields { owner: first, ..Default::default() };
        }
        let p = widgets::pal(ui);
        let obj = first.and_then(|id| self.session.doc().objects.get(&id).cloned());
        let obj_label = obj.as_ref().map(|o| {
            let kind = match &o.kind {
                ObjectKind::Text(_) => "Text box",
                ObjectKind::Shape(_) => "Shape",
                ObjectKind::Image(_) => "Picture",
                ObjectKind::Group { .. } => "Group",
                ObjectKind::Table(_) => "Table",
                ObjectKind::WordArt(_) => "WordArt",
            };
            if self.selection.len() > 1 {
                format!("{kind} + {} more", self.selection.len() - 1)
            } else {
                kind.to_string()
            }
        });
        let obj_label = obj_label.unwrap_or_default();
        let label = obj_label;
        ui.horizontal(|ui| {
            ui.heading("Format");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(label).size(12.0).color(p.text_muted));
            });
        });
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            let Some(obj) = obj else {
                self.empty_state(ui);
                return;
            };
            match &obj.kind {
                ObjectKind::Text(_) => {
                    card(ui, icons::TEXT_AA, "Text", |ui| self.text_section(ui, obj.id));
                    card(ui, icons::TEXT_ALIGN_LEFT, "Paragraph", |ui| self.paragraph_section(ui, obj.id));
                    card(ui, icons::TEXTBOX, "Text box", |ui| self.text_box_section(ui, &obj));
                }
                ObjectKind::Shape(sh) => {
                    // A shape that holds text gets the text cards too (they act on its story).
                    if let Some(story) = sh.story {
                        card(ui, icons::TEXT_AA, "Text", |ui| self.text_section(ui, story));
                        card(ui, icons::TEXT_ALIGN_LEFT, "Paragraph", |ui| self.paragraph_section(ui, story));
                    }
                    card(ui, icons::PAINT_BUCKET, "Shape", |ui| self.shape_section(ui, &obj));
                }
                ObjectKind::Image(_) => {
                    card(ui, icons::IMAGE, "Picture", |ui| self.picture_section(ui, &obj));
                }
                ObjectKind::Table(_) => {
                    // While typing in a cell, its text gets the text cards.
                    if let Some(c) = self.caret.filter(|_| self.current_cell().is_some()) {
                        card(ui, icons::TEXT_AA, "Text", |ui| self.text_section(ui, c.frame));
                        card(ui, icons::TEXT_ALIGN_LEFT, "Paragraph", |ui| self.paragraph_section(ui, c.frame));
                    }
                    card(ui, icons::TABLE, "Table", |ui| self.table_section(ui, obj.id));
                }
                _ => {}
            }
            card(ui, icons::RULER, "Position and size", |ui| self.object_panel(ui));
            card(ui, icons::STACK, "Arrange", |ui| self.arrange_section(ui));
        });
    }

    fn empty_state(&mut self, ui: &mut egui::Ui) {
        let p = widgets::pal(ui);
        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(icons::CURSOR_CLICK).font(crate::theme::icon_font(40.0)).color(p.primary));
            ui.add_space(6.0);
            ui.label(RichText::new("Select something on the page to see its properties").size(13.0).color(p.text));
        });
        ui.add_space(12.0);
        card(ui, icons::LIGHTBULB, "Quick tips", |ui| {
            for tip in [
                "Click an object to select it; Shift-click adds to the selection.",
                "Draw a Text Box, then type to fill it.",
                "Arrow keys nudge the selection; hold Shift for bigger steps.",
                "Drag a corner handle to resize.",
            ] {
                widgets::hint(ui, tip);
            }
        });
    }

    // ---- Text -----------------------------------------------------------------------------------------------

    fn text_section(&mut self, ui: &mut egui::Ui, frame: Id) {
        let doc = self.session.doc();
        let Ok(sid) = doc.story_of(frame) else { return };
        let Ok(story) = doc.story(sid) else { return };
        // Shows and edits the selected text while editing, else the whole story.
        let (start, end) = self.text_target_range(frame);
        let at = start.unwrap_or(0).min(story.len());
        let Some(para0) = story.paras.get(story.para_index_at(at)) else { return };
        let rc = doc.resolve_char(para0, &story.span_attrs_at(at));
        let style_name = para0.style.and_then(|id| doc.styles.para.get(&id)).map(|s| s.name.clone());
        let color_now = rc.color.clone();
        let color_edit = {
            let doc = self.session.doc();
            let mut t = std::mem::take(buf(&mut self.fields.more, "text_color"));
            let r = color_field(ui, doc, &mut t, "Text color", Some(&color_now));
            *buf(&mut self.fields.more, "text_color") = t;
            r
        };
        let styles: Vec<String> = self
            .session
            .query(&newpub_engine::Query::Styles)
            .ok()
            .and_then(|v| {
                v.get("para")
                    .and_then(|a| a.as_array())
                    .map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect())
            })
            .unwrap_or_default();

        let mut font = rc.font.clone();
        let families = self.session.fonts().families();
        let combo = egui::ComboBox::from_id_salt("font")
            .width(ui.available_width() - 8.0)
            .selected_text(&font)
            .show_ui(ui, |ui| {
                for f in &families {
                    ui.selectable_value(&mut font, f.clone(), f);
                }
            });
        combo.response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, "Font"));
        let mut patch = CharAttrs::default();
        if font != rc.font {
            patch.font = Some(font);
        }
        if let Some(c) = color_edit.filter(|c| *c != color_now) {
            let attrs = CharAttrs { color: Some(c), ..Default::default() };
            self.act_run(format!("tcolor:{frame}"), Command::FormatChars { target: frame, start, end, attrs });
        }
        let size_edit = field(ui, &mut self.fields.size, "Font size", &fmt_num(rc.size));
        toggle_row(ui, |ui| {
            if widgets::icon_button(ui, icons::TEXT_B, "Bold", rc.bold, true).clicked() {
                patch.bold = Some(!rc.bold);
            }
            if widgets::icon_button(ui, icons::TEXT_ITALIC, "Italic", rc.italic, true).clicked() {
                patch.italic = Some(!rc.italic);
            }
            if widgets::icon_button(ui, icons::TEXT_UNDERLINE, "Underline", rc.underline, true).clicked() {
                patch.underline = Some(!rc.underline);
            }
            if widgets::icon_button(ui, icons::TEXT_STRIKETHROUGH, "Strikethrough", rc.strike, true).clicked() {
                patch.strike = Some(!rc.strike);
            }
            let sup = rc.baseline == Baseline::Superscript;
            if widgets::icon_button(ui, icons::TEXT_SUPERSCRIPT, "Superscript", sup, true).clicked() {
                patch.baseline = Some(if sup { Baseline::Normal } else { Baseline::Superscript });
            }
            let sub = rc.baseline == Baseline::Subscript;
            if widgets::icon_button(ui, icons::TEXT_SUBSCRIPT, "Subscript", sub, true).clicked() {
                patch.baseline = Some(if sub { Baseline::Normal } else { Baseline::Subscript });
            }
            let sc = rc.caps == Caps::SmallCaps;
            if widgets::icon_button(ui, icons::TEXT_AA, "Small caps", sc, true).clicked() {
                patch.caps = Some(if sc { Caps::Normal } else { Caps::SmallCaps });
            }
        });
        if !patch.is_empty() {
            self.act(Command::FormatChars { target: frame, start, end, attrs: patch });
        }
        if widgets::small_button(ui, icons::ERASER, "Clear Formatting", false, true)
            .on_hover_text("Remove character formatting from the selected text (or the whole story)")
            .clicked()
        {
            self.act(Command::ClearCharFormat { target: frame, start, end });
        }
        if let Some(t) = size_edit
            && let Some(v) = core::units::parse_length(&t).filter(|v| (1.0..=999.0).contains(v))
            && (v - rc.size).abs() > 1e-9
        {
            let attrs = CharAttrs { size: Some(Length(v)), ..Default::default() };
            self.act_run(format!("size:{frame}"), Command::FormatChars { target: frame, start, end, attrs });
        }
        if !styles.is_empty() {
            let current = style_name.unwrap_or_else(|| "None".into());
            let mut items: Vec<&str> = vec!["None"];
            items.extend(styles.iter().map(String::as_str));
            if let Some(i) = choice(ui, "Style", &current, &items) {
                let style = (i > 0).then(|| StyleRef::Name(styles[i - 1].clone()));
                self.act(Command::ApplyParaStyle {
                    target: frame,
                    start: self.para_target_range(frame).0,
                    end: self.para_target_range(frame).1,
                    style,
                    clear_overrides: false,
                });
            }
        }
    }

    // ---- Paragraph ------------------------------------------------------------------------------------------

    fn paragraph_section(&mut self, ui: &mut egui::Ui, frame: Id) {
        let doc = self.session.doc();
        let Ok(sid) = doc.story_of(frame) else { return };
        let Ok(story) = doc.story(sid) else { return };
        // The paragraphs at the caret or selection while editing, else every paragraph.
        let (start, end) = self.para_target_range(frame);
        let at = start.unwrap_or(0).min(story.len());
        let Some(para0) = story.paras.get(story.para_index_at(at)) else { return };
        let rp = doc.resolve_para(para0);
        let mut patch = ParaAttrs::default();
        toggle_row(ui, |ui| {
            for (a, icon, label) in [
                (TextAlign::Left, icons::TEXT_ALIGN_LEFT, "Align Left"),
                (TextAlign::Center, icons::TEXT_ALIGN_CENTER, "Center"),
                (TextAlign::Right, icons::TEXT_ALIGN_RIGHT, "Align Right"),
                (TextAlign::Justify, icons::TEXT_ALIGN_JUSTIFY, "Justify"),
            ] {
                if widgets::icon_button(ui, icon, label, rp.align == a, true).clicked() {
                    patch.align = Some(a);
                }
            }
            ui.add_space(8.0);
            let bullet = matches!(rp.list, ListStyle::Bullet { .. });
            if widgets::icon_button(ui, icons::LIST_BULLETS, "Bulleted list", bullet, true).clicked() {
                patch.list = Some(if bullet {
                    ListStyle::None
                } else {
                    ListStyle::Bullet { bullet: '\u{2022}', indent: Length(18.0) }
                });
            }
            let numbered = matches!(rp.list, ListStyle::Numbered { .. });
            if widgets::icon_button(ui, icons::LIST_NUMBERS, "Numbered list", numbered, true).clicked() {
                patch.list = Some(if numbered {
                    ListStyle::None
                } else {
                    ListStyle::Numbered {
                        format: core::NumberFormat::Decimal,
                        start: 1,
                        suffix: ".".into(),
                        indent: Length(18.0),
                    }
                });
            }
        });
        if patch != ParaAttrs::default() {
            self.act(Command::FormatParas { target: frame, start, end, attrs: patch });
        }
        let ls_model = match rp.line_spacing {
            LineSpacing::Multiple(m) => fmt_num(m),
            LineSpacing::Exactly(l) | LineSpacing::AtLeast(l) => fmt_pt(l.0),
        };
        let ls = field(ui, buf(&mut self.fields.more, "line_spacing"), "Line spacing", &ls_model);
        let before = field(ui, buf(&mut self.fields.more, "space_before"), "Space before", &fmt_pt(rp.space_before));
        let after = field(ui, buf(&mut self.fields.more, "space_after"), "Space after", &fmt_pt(rp.space_after));
        let left = field(ui, buf(&mut self.fields.more, "indent_left"), "Indent left", &fmt_pt(rp.indent_left));
        let firstl = field(ui, buf(&mut self.fields.more, "indent_first"), "First line", &fmt_pt(rp.indent_first));
        let mut attrs = ParaAttrs::default();
        let mut key = "";
        if let Some(t) = ls {
            let t = t.trim();
            let v = if let Ok(m) = t.parse::<f64>() {
                (0.1..=10.0).contains(&m).then_some(LineSpacing::Multiple(m))
            } else {
                core::units::parse_length(t).filter(|v| *v > 0.0).map(|v| LineSpacing::Exactly(Length(v)))
            };
            if let Some(v) = v
                && v != rp.line_spacing
            {
                attrs.line_spacing = Some(v);
                key = "linespacing";
            }
        }
        let len = |t: Option<String>, min: f64, cur: f64| {
            t.and_then(|t| core::units::parse_length(&t)).filter(|v| *v >= min && (v - cur).abs() > 1e-9)
        };
        if let Some(v) = len(before, 0.0, rp.space_before) {
            attrs.space_before = Some(Length(v));
            key = "spacebefore";
        }
        if let Some(v) = len(after, 0.0, rp.space_after) {
            attrs.space_after = Some(Length(v));
            key = "spaceafter";
        }
        if let Some(v) = len(left, 0.0, rp.indent_left) {
            attrs.indent_left = Some(Length(v));
            key = "indentleft";
        }
        if let Some(v) = len(firstl, -1000.0, rp.indent_first) {
            attrs.indent_first = Some(Length(v));
            key = "indentfirst";
        }
        if attrs != ParaAttrs::default() {
            self.act_run(format!("{key}:{frame}"), Command::FormatParas { target: frame, start, end, attrs });
        }
    }

    // ---- Text box -------------------------------------------------------------------------------------------

    fn text_box_section(&mut self, ui: &mut egui::Ui, obj: &Object) {
        let ObjectKind::Text(t) = &obj.kind else { return };
        let id = obj.id;
        let cols = field(ui, &mut self.fields.columns, "Columns", &t.columns.to_string());
        let gutter = field(ui, buf(&mut self.fields.more, "gutter"), "Gutter", &fmt_len(t.gutter.0));
        let ins = t.insets;
        let uniform = ins.left == ins.right && ins.left == ins.top && ins.left == ins.bottom;
        let margins_model = if uniform { fmt_len(ins.left.0) } else { "mixed".into() };
        let margins = field(ui, buf(&mut self.fields.more, "insets"), "Inner margins", &margins_model);
        let mut patch = TextFramePatch::default();
        let mut key = "";
        if let Some(n) = cols.and_then(|t| t.trim().parse::<u32>().ok()).filter(|n| (1..=20).contains(n))
            && n != t.columns
        {
            patch.columns = Some(n);
            key = "columns";
        }
        if let Some(v) = gutter.and_then(|t| core::units::parse_length(&t)).filter(|v| *v >= 0.0)
            && (v - t.gutter.0).abs() > 1e-9
        {
            patch.gutter = Some(Length(v));
            key = "gutter";
        }
        if let Some(v) = margins.and_then(|t| core::units::parse_length(&t)).filter(|v| *v >= 0.0)
            && !(uniform && (v - ins.left.0).abs() < 1e-9)
        {
            patch.insets = Some(Insets::uniform(v));
            key = "insets";
        }
        row(ui, "Vertical align", |ui, _| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (v, icon, label) in [
                (VAlign::Bottom, icons::ALIGN_BOTTOM, "Bottom"),
                (VAlign::Middle, icons::ALIGN_CENTER_VERTICAL, "Middle"),
                (VAlign::Top, icons::ALIGN_TOP, "Top"),
            ] {
                if widgets::icon_button(ui, icon, label, t.valign == v, true).clicked() && t.valign != v {
                    patch.valign = Some(v);
                    key = "valign";
                }
            }
        });
        let fits = ["No autofit", "Shrink on overflow", "Best fit", "Grow frame"];
        let cur = match t.autofit {
            Autofit::None => 0,
            Autofit::ShrinkOnOverflow => 1,
            Autofit::BestFit => 2,
            Autofit::GrowFrame => 3,
        };
        if let Some(i) = choice(ui, "Autofit", fits[cur], &fits)
            && i != cur
        {
            patch.autofit = Some([Autofit::None, Autofit::ShrinkOnOverflow, Autofit::BestFit, Autofit::GrowFrame][i]);
            key = "autofit";
        }
        if patch != TextFramePatch::default() {
            self.act_run(format!("{key}:{id}"), Command::SetTextFrame { id, patch });
        }
        let overflow = self
            .session
            .query(&newpub_engine::Query::Overflow { target: id })
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if overflow {
            let p = widgets::pal(ui);
            ui.horizontal(|ui| {
                ui.label(RichText::new(icons::WARNING).font(crate::theme::icon_font(14.0)).color(p.danger));
                ui.label(RichText::new("Text overflow").size(12.0).color(p.danger));
            });
        }
        let doc = self.session.doc();
        let chain = doc.story_of(id).ok().and_then(|s| doc.story(s).ok()).map(|s| s.frames.clone()).unwrap_or_default();
        let has_next = chain.last().is_some_and(|l| *l != id);
        let linking = self.view.link_from == Some(id);
        // Where this box sits in its story, with Previous / Next (also Shift+Tab / Tab on the canvas).
        if let Some((i, chain)) = self.chain_of(id) {
            let mut place = format!("Box {} of {} in this story", i + 1, chain.len());
            if let Some(pg) = chain.get(i + 1).and_then(|f| doc.page_of(*f)).filter(|pg| *pg != self.page) {
                place.push_str(&format!(", continues on page {}", pg + 1));
            }
            widgets::hint(ui, &place);
            let mut go = None;
            toggle_row(ui, |ui| {
                if widgets::small_button(ui, icons::CARET_LEFT, "Previous Text Box", false, i > 0)
                    .on_hover_text("Select the box before this one (Shift+Tab)")
                    .clicked()
                {
                    go = Some(false);
                }
                if widgets::small_button(ui, icons::CARET_RIGHT, "Next Text Box", false, i + 1 < chain.len())
                    .on_hover_text("Select the box the story continues in (Tab)")
                    .clicked()
                {
                    go = Some(true);
                }
            });
            if let Some(forward) = go {
                self.go_to_box(id, forward, false);
            }
        }
        self.continued_toggles(ui, id, t.continued_on, t.continued_from);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            if widgets::small_button(ui, icons::LINK, "Link to Next Box", linking, !has_next)
                .on_hover_text(
                    "Then click an empty text box: the story continues there (click again or press Esc to stop)",
                )
                .on_disabled_hover_text("This box already continues in another box; break that link first")
                .clicked()
            {
                if linking {
                    self.cancel_link();
                } else {
                    self.start_link(id);
                }
            }
            if widgets::small_button(ui, icons::LINK_BREAK, "Break Link", false, has_next)
                .on_hover_text("The following boxes no longer continue this story")
                .clicked()
            {
                self.act(Command::UnlinkFrame { frame: id });
            }
            if widgets::small_button(ui, icons::FILES, "Flow onto New Pages", false, overflow)
                .on_hover_text("Add pages with linked boxes until all the text fits")
                .on_disabled_hover_text("All the text fits")
                .clicked()
            {
                self.act(newpub_engine::SessionAction::Autoflow { frame: id });
            }
        });
    }

    /// The Text box card's "Include 'Continued on page…'" and "Include 'Continued from page…'" checkboxes
    /// (Publisher: Format Text Box > Text Box). The layout writes "(Continued on page N)" at the bottom of a box
    /// whose story goes on to a box on another page, and "(Continued from page N)" at the top of a box whose
    /// story comes from a box on another page: generated, never typed, one line of the box each. A toggle
    /// applies to every selected text box as one undo step. Where a notice can never appear (an unlinked box, the
    /// last or first box, a master page) the checkbox is greyed out and its tooltip says why; a flag that is
    /// already on stays clickable so it can be cleared.
    fn continued_toggles(&mut self, ui: &mut egui::Ui, id: Id, on: bool, from: bool) {
        let doc = self.session.doc();
        let pos = self.chain_of(id);
        let here = doc.page_of(id);
        let on_page = here.is_some();
        // The neighbouring box's page: its label for the tooltip, and whether it is this page (nothing is drawn then).
        let page_of = |f: Option<&Id>| f.and_then(|f| doc.page_of(*f)).map(|p| (doc.page_label(p), Some(p) == here));
        let next = pos.as_ref().and_then(|(i, c)| page_of(c.get(i + 1)));
        let prev = pos.as_ref().and_then(|(i, c)| page_of(i.checked_sub(1).and_then(|k| c.get(k))));
        // Every selected text box with its two flags; the first selected box is the one the card shows.
        let targets: Vec<(Id, bool, bool)> = self
            .selection
            .iter()
            .filter_map(|s| match &doc.objects.get(s)?.kind {
                ObjectKind::Text(tf) => Some((*s, tf.continued_on, tf.continued_from)),
                _ => None,
            })
            .collect();
        let mut toggles: Vec<(bool, bool)> = Vec::new();
        for (is_on, cur, label, page, end_reason, place) in [
            (
                true,
                on,
                CONTINUED_ON_LABEL,
                &next,
                "This is the last box of the story",
                "at the bottom of this box when the story goes on to a box on another page",
            ),
            (
                false,
                from,
                CONTINUED_FROM_LABEL,
                &prev,
                "This is the first box of the story",
                "at the top of this box when the story comes from a box on another page",
            ),
        ] {
            let applicable = on_page && page.is_some();
            let mixed = targets.iter().any(|(_, o, f)| if is_on { *o != cur } else { *f != cur });
            let word = if is_on { "on" } else { "from" };
            let why = if !on_page {
                "Boxes on a master page have no page number"
            } else if pos.is_none() {
                "Link this box to another box first (Link to Next Box)"
            } else {
                end_reason
            };
            let hover = match page {
                Some((label, false)) => format!(
                    "Writes \"(Continued {word} page {label})\" in small italics {place}. The notice is generated, \
                     not typed: it takes one line of the box and its page number keeps itself up to date."
                ),
                Some((_, true)) => format!(
                    "Nothing is shown yet: the {} box is on this page too. The notice appears {place}; it takes one \
                     line of the box.",
                    if is_on { "next" } else { "previous" }
                ),
                // Enabled only so a flag set earlier can be cleared.
                None => format!("No notice is shown now: {}. Untick to clear the setting.", why.to_lowercase()),
            };
            let mut v = cur;
            let r = ui
                .add_enabled(
                    applicable || cur,
                    egui::Checkbox::new(&mut v, RichText::new(label).size(12.0)).indeterminate(mixed),
                )
                .on_hover_text(hover)
                .on_disabled_hover_text(why);
            if r.changed() {
                // A mixed selection resolves to "on", as checkboxes do elsewhere.
                toggles.push((is_on, mixed || !cur));
            }
        }
        if toggles.is_empty() {
            return;
        }
        let group = targets.len() > 1 || toggles.len() > 1;
        if group {
            self.act(newpub_engine::SessionAction::BeginGroup);
        }
        for (is_on, v) in toggles {
            for (tid, ..) in &targets {
                let mut patch = TextFramePatch::default();
                if is_on {
                    patch.continued_on = Some(v);
                } else {
                    patch.continued_from = Some(v);
                }
                self.act(Command::SetTextFrame { id: *tid, patch });
            }
        }
        if group {
            self.act(newpub_engine::SessionAction::EndGroup);
        }
    }

    // ---- Shape ----------------------------------------------------------------------------------------------

    fn shape_section(&mut self, ui: &mut egui::Ui, obj: &Object) {
        let ObjectKind::Shape(s) = &obj.kind else { return };
        let id = obj.id;
        let doc = self.session.doc();
        let mut ft = std::mem::take(buf(&mut self.fields.more, "fill"));
        let fill = color_field(ui, doc, &mut ft, "Fill color", s.fill.as_ref());
        *buf(&mut self.fields.more, "fill") = ft;
        let mut no_fill = false;
        ui.horizontal(|ui| {
            if widgets::small_button(ui, icons::PROHIBIT, "No fill", s.fill.is_none(), true).clicked() {
                no_fill = true;
            }
        });
        let doc = self.session.doc();
        let stroke = s.stroke.clone();
        let mut lt = std::mem::take(buf(&mut self.fields.more, "line"));
        let line = color_field(ui, doc, &mut lt, "Line color", stroke.as_ref().map(|s| &s.color));
        *buf(&mut self.fields.more, "line") = lt;
        let width_model = stroke.as_ref().map(|s| fmt_num(s.width.0)).unwrap_or_default();
        let width = field(ui, buf(&mut self.fields.more, "line_width"), "Line width", &width_model);
        let dashes = ["Solid", "Dash", "Dot", "Dash dot", "Long dash"];
        let dash_all = [Dash::Solid, Dash::Dash, Dash::Dot, Dash::DashDot, Dash::LongDash];
        let cur_dash = stroke.as_ref().map(|s| s.dash).unwrap_or_default();
        let dash_i = dash_all.iter().position(|d| *d == cur_dash).unwrap_or(0);
        let dash = choice(ui, "Line style", dashes[dash_i], &dashes);
        let mut no_line = false;
        ui.horizontal(|ui| {
            if widgets::small_button(ui, icons::PROHIBIT, "No line", stroke.is_none(), true).clicked() {
                no_line = true;
            }
        });

        let mut patch = ShapePatch::default();
        let mut key = "";
        if let Some(c) = fill
            && s.fill.as_ref() != Some(&c)
        {
            patch.fill = Some(c);
            key = "fill";
        }
        if no_fill && s.fill.is_some() {
            patch.no_fill = true;
            key = "nofill";
        }
        let base = stroke.clone().unwrap_or(core::Stroke {
            color: Color::BLACK,
            width: Length(0.75),
            dash: Dash::Solid,
            cap: Default::default(),
            join: Default::default(),
        });
        let mut new_stroke = base.clone();
        if let Some(c) = line {
            new_stroke.color = c;
            key = "line";
        }
        if let Some(w) = width.and_then(|t| core::units::parse_length(&t)).filter(|v| (0.0..=500.0).contains(v)) {
            new_stroke.width = Length(w);
            key = "linewidth";
        }
        if let Some(i) = dash {
            new_stroke.dash = dash_all[i];
            key = "linedash";
        }
        if new_stroke != base || (stroke.is_none() && ["line", "linewidth", "linedash"].contains(&key)) {
            patch.stroke = Some(new_stroke);
        }
        if no_line && stroke.is_some() {
            patch.no_stroke = true;
            key = "noline";
        }
        if patch != ShapePatch::default() {
            self.act_run(format!("{key}:{id}"), Command::SetShape { id, patch });
        }
    }

    // ---- Picture --------------------------------------------------------------------------------------------

    fn picture_section(&mut self, ui: &mut egui::Ui, obj: &Object) {
        let ObjectKind::Image(img) = &obj.kind else { return };
        let id = obj.id;
        let p = widgets::pal(ui);
        let l = ui.label(RichText::new("Alt text").size(12.0).color(p.text_muted));
        let model = obj.alt_text.clone().unwrap_or_default();
        let text = buf(&mut self.fields.more, "alt");
        let r = ui
            .add(
                egui::TextEdit::multiline(text)
                    .desired_rows(2)
                    .desired_width(f32::INFINITY)
                    .hint_text("Describe the picture"),
            )
            .labelled_by(l.id);
        let alt = if r.changed() {
            Some(text.clone())
        } else {
            if !r.has_focus() {
                *text = model.clone();
            }
            None
        };
        let mut deco = obj.decorative;
        let deco_changed = ui.checkbox(&mut deco, "Decorative").changed();
        let fits = ["Stretch", "Fit", "Fill"];
        let all = [Fit::Stretch, Fit::Fit, Fit::Fill];
        let cur = all.iter().position(|f| *f == img.fit).unwrap_or(0);
        let fit = choice(ui, "Picture fit", fits[cur], &fits);
        if let Some(a) = alt
            && a != model
        {
            let patch = ObjectPatch { alt_text: Some(a), ..Default::default() };
            self.act_run(format!("alt:{id}"), Command::SetObject { id, patch });
        }
        if deco_changed {
            self.act(Command::SetObject { id, patch: ObjectPatch { decorative: Some(deco), ..Default::default() } });
        }
        if let Some(i) = fit
            && i != cur
        {
            let patch = core::ImagePatch { fit: Some(all[i]), ..Default::default() };
            self.act(Command::SetImage { id, patch });
        }
    }

    // ---- Position and size ----------------------------------------------------------------------------------

    /// Position, size and rotation of the first selected object.
    fn object_panel(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.selection.first().copied() else { return };
        let Some(obj) = self.session.doc().objects.get(&id) else { return };
        let (rect, rotation) = (obj.rect, obj.rotation);
        let mut new_rect = rect;
        let mut edited = false;
        let model = [rect.x, rect.y, rect.w, rect.h];
        let mut bufs = [
            std::mem::take(&mut self.fields.x),
            std::mem::take(&mut self.fields.y),
            std::mem::take(&mut self.fields.w),
            std::mem::take(&mut self.fields.h),
        ];
        for (i, label) in ["X position", "Y position", "Width", "Height"].into_iter().enumerate() {
            if let Some(t) = field(ui, &mut bufs[i], label, &fmt_len(model[i])) {
                // Any unit is accepted; partial input such as "2i" is ignored until it parses.
                if let Some(v) = core::units::parse_length(&t).filter(|v| *v >= if i < 2 { f64::MIN } else { 0.1 }) {
                    match i {
                        0 => new_rect.x = v,
                        1 => new_rect.y = v,
                        2 => new_rect.w = v,
                        _ => new_rect.h = v,
                    }
                    edited = true;
                }
            }
        }
        let [x, y, w, h] = bufs;
        (self.fields.x, self.fields.y, self.fields.w, self.fields.h) = (x, y, w, h);
        let rot =
            field(ui, buf(&mut self.fields.more, "rotation"), "Rotation", &format!("{}\u{b0}", fmt_num(rotation)));
        if edited && new_rect != rect {
            let patch = ObjectPatch { rect: Some(new_rect), ..Default::default() };
            self.act_run(format!("geometry:{id}"), Command::SetObject { id, patch });
        }
        if let Some(t) = rot
            && let Ok(v) = t.trim().trim_end_matches('\u{b0}').trim().parse::<f64>()
            && (v - rotation).abs() > 1e-9
        {
            let patch = ObjectPatch { rotation: Some(v), ..Default::default() };
            self.act_run(format!("rotation:{id}"), Command::SetObject { id, patch });
        }
    }

    // ---- Arrange --------------------------------------------------------------------------------------------

    fn arrange_section(&mut self, ui: &mut egui::Ui) {
        let ids = self.selection.clone();
        let Some(&first) = ids.first() else { return };
        let doc = self.session.doc();
        let Some(obj) = doc.objects.get(&first) else { return };
        let locked = obj.locked;
        let is_group = matches!(obj.kind, ObjectKind::Group { .. });
        let multi = ids.len() >= 2;
        let mut cmds: Vec<Command> = vec![];
        toggle_row(ui, |ui| {
            for (op, icon, label) in [
                (ZOp::Forward, icons::ARROW_FAT_UP, "Bring forward"),
                (ZOp::Backward, icons::ARROW_FAT_DOWN, "Send backward"),
                (ZOp::Front, icons::ARROW_FAT_LINES_UP, "Move to front"),
                (ZOp::Back, icons::ARROW_FAT_LINES_DOWN, "Move to back"),
            ] {
                if widgets::icon_button(ui, icon, label, false, true).clicked() {
                    cmds.push(Command::SetZ { id: first, op });
                }
            }
        });
        let relative = if multi { AlignTo::Selection } else { AlignTo::Page };
        toggle_row(ui, |ui| {
            for (edge, icon, label) in [
                (AlignEdge::Left, icons::ALIGN_LEFT, "Align objects left"),
                (AlignEdge::Center, icons::ALIGN_CENTER_HORIZONTAL, "Align objects center"),
                (AlignEdge::Right, icons::ALIGN_RIGHT, "Align objects right"),
                (AlignEdge::Top, icons::ALIGN_TOP, "Align objects top"),
                (AlignEdge::Middle, icons::ALIGN_CENTER_VERTICAL, "Align objects middle"),
                (AlignEdge::Bottom, icons::ALIGN_BOTTOM, "Align objects bottom"),
            ] {
                if widgets::icon_button(ui, icon, label, false, !locked).clicked() {
                    cmds.push(Command::AlignObjects { ids: ids.clone(), edge, relative });
                }
            }
        });
        if !multi {
            widgets::hint(ui, "Align to the page. Select several objects to align them to each other.");
        }
        toggle_row(ui, |ui| {
            let dist = ids.len() >= 3;
            if widgets::icon_button(ui, icons::ARROWS_HORIZONTAL, "Distribute horizontally", false, dist).clicked() {
                cmds.push(Command::DistributeObjects { ids: ids.clone(), axis: Axis::Horizontal });
            }
            if widgets::icon_button(ui, icons::ARROWS_VERTICAL, "Distribute vertically", false, dist).clicked() {
                cmds.push(Command::DistributeObjects { ids: ids.clone(), axis: Axis::Vertical });
            }
            ui.add_space(8.0);
            if widgets::icon_button(ui, icons::SELECTION_PLUS, "Group selection", false, multi).clicked() {
                cmds.push(Command::Group { ids: ids.clone() });
            }
            if widgets::icon_button(ui, icons::SELECTION_SLASH, "Ungroup selection", false, is_group).clicked() {
                cmds.push(Command::Ungroup { id: first });
            }
            if widgets::icon_button(ui, icons::LOCK, "Lock object", locked, true).clicked() {
                for id in &ids {
                    cmds.push(Command::SetObject {
                        id: *id,
                        patch: ObjectPatch { locked: Some(!locked), ..Default::default() },
                    });
                }
            }
        });
        // Text wrap: how text in boxes behind this object flows around it.
        let wrap = obj.wrap;
        let modes = [
            (WrapMode::None, "None"),
            (WrapMode::Square, "Square"),
            (WrapMode::Tight, "Tight"),
            (WrapMode::TopBottom, "Top and bottom"),
            (WrapMode::Through, "Through"),
        ];
        let labels: Vec<&str> = modes.iter().map(|m| m.1).collect();
        let cur = modes.iter().position(|m| m.0 == wrap.mode).unwrap_or(0);
        if let Some(i) = choice(ui, "Wrap text", labels[cur], &labels)
            && i != cur
        {
            // Turning wrap on with no gap would let text touch the object; start from Publisher's 0.1 in.
            let distance =
                if wrap.mode == WrapMode::None && wrap.distance.0 == 0.0 { Length(7.2) } else { wrap.distance };
            let w = Wrap { mode: modes[i].0, distance };
            for id in &ids {
                cmds.push(Command::SetObject { id: *id, patch: ObjectPatch { wrap: Some(w), ..Default::default() } });
            }
        }
        if wrap.mode != WrapMode::None {
            let model = fmt_len(wrap.distance.0);
            if let Some(v) = length_field(self, ui, "wrap_distance", "Distance from text", &model)
                .filter(|v| *v >= 0.0 && (v - wrap.distance.0).abs() > 1e-9)
            {
                let w = Wrap { mode: wrap.mode, distance: Length(v) };
                for id in &ids {
                    self.act_run(
                        format!("wrapdist:{id}"),
                        Command::SetObject { id: *id, patch: ObjectPatch { wrap: Some(w), ..Default::default() } },
                    );
                }
            }
        } else {
            widgets::hint(ui, "Text in boxes behind this object runs underneath it.");
        }
        for c in cmds {
            self.act(c);
        }
    }
}
