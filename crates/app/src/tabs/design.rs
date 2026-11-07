//! The Page Design ribbon tab: page setup, schemes, background, masters, guides, layers, sections.

use crate::widgets::{self, group, ribbon_button, small_button};
use crate::{NewpubApp, fmt_num, icons as ic, labeled_field};
use egui::Color32;
use newpub_engine::SessionAction;
use newpub_engine::action::Units;
use newpub_engine::core::attrs::NumberFormat;
use newpub_engine::core::schemes::{ColorScheme, FontScheme};
use newpub_engine::core::units::{Insets, Length, parse_length};
use newpub_engine::core::{Color, Command, Id};
use newpub_engine::{Action, Query};

/// Which Page Design window is open (one at a time).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Win {
    #[default]
    None,
    PageSetup,
    ColorScheme,
    FontScheme,
    Background,
    Masters,
    Guides,
    Baseline,
    Layers,
    Section,
}

/// State of the Design tab's windows.
pub(crate) struct DesignState {
    win: Win,
    width: String,
    height: String,
    top: String,
    bottom: String,
    inside: String,
    outside: String,
    facing: bool,
    bleed: String,
    bg: String,
    bg_all: bool,
    master_name: String,
    master_sel: Option<Id>,
    cols: String,
    rows: String,
    gutter: String,
    base_spacing: String,
    base_offset: String,
    pub(crate) snapping: bool,
    layer_name: String,
    layer_sel: Option<Id>,
    sec_start: String,
    sec_format: NumberFormat,
}

impl Default for DesignState {
    fn default() -> Self {
        DesignState {
            win: Win::None,
            width: String::new(),
            height: String::new(),
            top: String::new(),
            bottom: String::new(),
            inside: String::new(),
            outside: String::new(),
            facing: false,
            bleed: String::new(),
            bg: "#ffffff".into(),
            bg_all: false,
            master_name: String::new(),
            master_sel: None,
            cols: "1".into(),
            rows: "1".into(),
            gutter: "12pt".into(),
            base_spacing: "12pt".into(),
            base_offset: "0pt".into(),
            snapping: true,
            layer_name: String::new(),
            layer_sel: None,
            sec_start: "1".into(),
            sec_format: NumberFormat::Decimal,
        }
    }
}

fn unit_suffix(u: Units) -> (&'static str, f64) {
    match u {
        Units::In => ("in", 72.0),
        Units::Cm => ("cm", 72.0 / 2.54),
        Units::Mm => ("mm", 72.0 / 25.4),
        Units::Pt => ("pt", 1.0),
        Units::Pi => ("pi", 12.0),
    }
}

fn to_color32(c: &Color) -> Color32 {
    let [r, g, b, _] = c.to_rgba8();
    Color32::from_rgb(r, g, b)
}

/// Where windows open: below the ribbon, so they never cover its buttons.
const WIN_POS: egui::Pos2 = egui::pos2(40.0, 200.0);

const BG_SWATCHES: [&str; 8] = ["#ffffff", "#fff8e7", "#ffeecc", "#e8f4f8", "#e9f5e9", "#f5e6f0", "#eeeeee", "#1f2a37"];

const PRESETS: [(&str, f64, f64); 4] =
    [("Letter", 612.0, 792.0), ("A4", 595.276, 841.89), ("A5", 419.528, 595.276), ("Tabloid", 792.0, 1224.0)];

const FORMATS: [(NumberFormat, &str); 5] = [
    (NumberFormat::Decimal, "1, 2, 3"),
    (NumberFormat::LowerAlpha, "a, b, c"),
    (NumberFormat::UpperAlpha, "A, B, C"),
    (NumberFormat::LowerRoman, "i, ii, iii"),
    (NumberFormat::UpperRoman, "I, II, III"),
];

impl NewpubApp {
    fn len_text(&self, pt: f64) -> String {
        let (s, per) = unit_suffix(self.view.units);
        format!("{}{s}", fmt_num(pt / per))
    }

    fn toggle_win(&mut self, w: Win) {
        if self.design_ui.win == w {
            self.design_ui.win = Win::None;
            return;
        }
        self.design_ui.win = w;
        let doc = self.session.doc();
        match w {
            Win::PageSetup => {
                let s = doc.setup.clone();
                let ui = &mut self.design_ui;
                ui.facing = s.facing;
                let t = (
                    self.len_text(s.width.0),
                    self.len_text(s.height.0),
                    self.len_text(s.margins.top.0),
                    self.len_text(s.margins.bottom.0),
                    self.len_text(s.margins.left.0),
                    self.len_text(s.margins.right.0),
                    self.len_text(s.bleed.0),
                );
                let ui = &mut self.design_ui;
                (ui.width, ui.height, ui.top, ui.bottom, ui.inside, ui.outside, ui.bleed) = t;
            }
            Win::Background => {
                if let Some(c) = doc.pages.get(self.page).and_then(|p| p.background.clone()) {
                    let [r, g, b, _] = c.to_rgba8();
                    self.design_ui.bg = format!("#{r:02x}{g:02x}{b:02x}");
                }
            }
            Win::Guides => {
                if let Some(g) = &doc.guides.grid {
                    let (c, r, gu) = (g.columns, g.rows, g.gutter.0);
                    self.design_ui.cols = c.to_string();
                    self.design_ui.rows = r.to_string();
                    self.design_ui.gutter = self.len_text(gu);
                }
            }
            Win::Baseline => {
                if let Some(b) = &doc.baseline_grid {
                    let (s, o) = (b.spacing.0, b.offset.0);
                    self.design_ui.base_spacing = self.len_text(s);
                    self.design_ui.base_offset = self.len_text(o);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn design_tab(&mut self, ui: &mut egui::Ui) {
        let w = self.design_ui.win;
        group(ui, "Page Setup", |ui| {
            if ribbon_button(ui, ic::RULER, "Page Setup", w == Win::PageSetup, true).clicked() {
                self.toggle_win(Win::PageSetup);
            }
        });
        group(ui, "Scheme", |ui| {
            if ribbon_button(ui, ic::PALETTE, "Color Scheme", w == Win::ColorScheme, true).clicked() {
                self.toggle_win(Win::ColorScheme);
            }
            if ribbon_button(ui, ic::TEXT_AA, "Font Scheme", w == Win::FontScheme, true).clicked() {
                self.toggle_win(Win::FontScheme);
            }
            if ribbon_button(ui, ic::PAINT_BUCKET, "Background", w == Win::Background, true).clicked() {
                self.toggle_win(Win::Background);
            }
        });
        group(ui, "Masters", |ui| {
            if ribbon_button(ui, ic::SQUARES_FOUR, "Master Pages", w == Win::Masters, true).clicked() {
                self.toggle_win(Win::Masters);
            }
        });
        group(ui, "Guides", |ui| {
            if ribbon_button(ui, ic::GRID_FOUR, "Grid Guides", w == Win::Guides, true).clicked() {
                self.toggle_win(Win::Guides);
            }
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                if small_button(ui, ic::ROWS, "Baseline Grid", w == Win::Baseline, true).clicked() {
                    self.toggle_win(Win::Baseline);
                }
                let on = self.design_ui.snapping;
                if small_button(ui, ic::MAGNET, "Snap to Guides", on, true)
                    .on_hover_text(format!(
                        "Objects being moved or resized snap to the margins, guides, page centre and other \
                         objects. Hold {} while dragging to place freely.",
                        crate::snap_ui::free_key()
                    ))
                    .clicked()
                {
                    self.design_ui.snapping = !on;
                    self.act(SessionAction::SetSnapping { enabled: !on });
                    self.status =
                        if on { "Snap to Guides off: objects move freely".into() } else { "Snap to Guides on".into() };
                }
            });
        });
        group(ui, "Organize", |ui| {
            if ribbon_button(ui, ic::STACK, "Layers", w == Win::Layers, true).clicked() {
                self.toggle_win(Win::Layers);
            }
            if ribbon_button(ui, ic::LIST_NUMBERS, "Page Numbering", w == Win::Section, true).clicked() {
                self.toggle_win(Win::Section);
            }
        });
    }

    pub(crate) fn design_windows(&mut self, ctx: &egui::Context) {
        let mut cmds: Vec<Command> = vec![];
        let mut open = self.design_ui.win != Win::None;
        match self.design_ui.win {
            Win::None => {}
            Win::PageSetup => self.win_page_setup(ctx, &mut open, &mut cmds),
            Win::ColorScheme => self.win_color_scheme(ctx, &mut open, &mut cmds),
            Win::FontScheme => self.win_font_scheme(ctx, &mut open, &mut cmds),
            Win::Background => self.win_background(ctx, &mut open, &mut cmds),
            Win::Masters => self.win_masters(ctx, &mut open, &mut cmds),
            Win::Guides => self.win_guides(ctx, &mut open, &mut cmds),
            Win::Baseline => self.win_baseline(ctx, &mut open, &mut cmds),
            Win::Layers => self.win_layers(ctx, &mut open, &mut cmds),
            Win::Section => self.win_section(ctx, &mut open, &mut cmds),
        }
        for c in cmds {
            if let Some(o) = self.act(Action::from(c)) {
                // A freshly added master or layer becomes the selection.
                if self.design_ui.win == Win::Masters
                    && self.session.doc().masters.iter().any(|m| Some(m.id) == o.created.first().copied())
                {
                    self.design_ui.master_sel = o.created.first().copied();
                }
                if self.design_ui.win == Win::Layers
                    && self.session.doc().layers.iter().any(|l| Some(l.id) == o.created.first().copied())
                {
                    self.design_ui.layer_sel = o.created.first().copied();
                }
            }
        }
        if !open {
            self.design_ui.win = Win::None;
        }
    }

    fn win_page_setup(&mut self, ctx: &egui::Context, open: &mut bool, cmds: &mut Vec<Command>) {
        let (suffix, _) = unit_suffix(self.view.units);
        let status = &mut self.status;
        let d = &mut self.design_ui;
        egui::Window::new("Page Size and Margins").open(open).collapsible(false).default_pos(WIN_POS).show(ctx, |ui| {
            widgets::hint(ui, "Presets");
            ui.horizontal(|ui| {
                for (name, w, h) in PRESETS {
                    if widgets::secondary_button(ui, name).clicked() {
                        let (suf, per) = unit_suffix(self.view.units);
                        d.width = format!("{}{suf}", fmt_num(w / per));
                        d.height = format!("{}{suf}", fmt_num(h / per));
                    }
                }
            });
            labeled_field(ui, "Page width", &mut d.width);
            labeled_field(ui, "Page height", &mut d.height);
            if widgets::secondary_button(ui, "Swap Portrait / Landscape").clicked() {
                std::mem::swap(&mut d.width, &mut d.height);
            }
            ui.add_space(4.0);
            labeled_field(ui, "Top margin", &mut d.top);
            labeled_field(ui, "Bottom margin", &mut d.bottom);
            let (l, r) = if d.facing { ("Inside margin", "Outside margin") } else { ("Left margin", "Right margin") };
            labeled_field(ui, l, &mut d.inside);
            labeled_field(ui, r, &mut d.outside);
            labeled_field(ui, "Bleed", &mut d.bleed);
            ui.checkbox(&mut d.facing, "Facing pages");
            widgets::hint(
                ui,
                &format!("Lengths accept units, for example 8.5in, 210mm or 54pt; a bare number is read as {suffix}."),
            );
            ui.add_space(4.0);
            if widgets::primary_button(ui, "Apply Page Setup").clicked() {
                let p = |s: &str| {
                    let s = s.trim();
                    let n: Option<f64> = s.parse().ok();
                    match n {
                        Some(v) => Some(v * unit_suffix(self.view.units).1),
                        None => parse_length(s),
                    }
                };
                let vals = [&d.width, &d.height, &d.top, &d.bottom, &d.inside, &d.outside, &d.bleed]
                    .map(|s| if s.trim().is_empty() { Some(0.0) } else { p(s) });
                if let [Some(w), Some(h), Some(t), Some(b), Some(i), Some(o), Some(bl)] = vals {
                    if w <= 0.0 || h <= 0.0 {
                        *status = "Error: page width and height must be positive".into();
                    } else {
                        cmds.push(Command::SetPageSetup(newpub_engine::core::command::SetupPatch {
                            width: Some(Length(w)),
                            height: Some(Length(h)),
                            margins: Some(Insets {
                                top: Length(t),
                                bottom: Length(b),
                                left: Length(i),
                                right: Length(o),
                            }),
                            facing: Some(d.facing),
                            bleed: Some(Length(bl)),
                        }));
                    }
                } else {
                    *status = "Error: could not read one of the page setup lengths".into();
                }
            }
        });
    }

    fn win_color_scheme(&mut self, ctx: &egui::Context, open: &mut bool, cmds: &mut Vec<Command>) {
        let schemes: Vec<ColorScheme> = self
            .session
            .query(&Query::ColorSchemes)
            .ok()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        let current = self.session.doc().color_scheme.as_ref().map(|c| c.name.clone());
        egui::Window::new("Color Schemes").open(open).collapsible(false).default_pos(WIN_POS).show(ctx, |ui| {
            for s in &schemes {
                let on = current.as_deref() == Some(s.name.as_str());
                let colors = [&s.main, &s.accent1, &s.accent2, &s.accent3, &s.accent4, &s.accent5];
                let row = scheme_row(ui, &s.name, on, |painter, rects| {
                    for (c, r) in colors.iter().zip(rects) {
                        painter.rect_filled(*r, 3.0, to_color32(c));
                    }
                });
                if row.hovered() && !on {
                    self.preview_next = Some((s.name.clone(), Command::ApplyColorScheme { name: s.name.clone() }));
                }
                if row.clicked() {
                    cmds.push(Command::ApplyColorScheme { name: s.name.clone() });
                }
            }
        });
    }

    fn win_font_scheme(&mut self, ctx: &egui::Context, open: &mut bool, cmds: &mut Vec<Command>) {
        let schemes: Vec<FontScheme> = self
            .session
            .query(&Query::FontSchemes)
            .ok()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        let current = self.session.doc().font_scheme.as_ref().map(|c| c.name.clone());
        egui::Window::new("Font Schemes").open(open).collapsible(false).default_pos(WIN_POS).show(ctx, |ui| {
            for s in &schemes {
                let on = current.as_deref() == Some(s.name.as_str());
                let row = scheme_row(ui, &s.name, on, |_, _| {});
                if row.hovered() && !on {
                    self.preview_next = Some((s.name.clone(), Command::ApplyFontScheme { name: s.name.clone() }));
                }
                if row.clicked() {
                    cmds.push(Command::ApplyFontScheme { name: s.name.clone() });
                }
                widgets::hint(ui, &format!("{} / {}", s.major, s.minor));
            }
        });
    }

    fn win_background(&mut self, ctx: &egui::Context, open: &mut bool, cmds: &mut Vec<Command>) {
        let page = self.page;
        let pages = self.session.doc().pages.len();
        let d = &mut self.design_ui;
        egui::Window::new("Page Background").open(open).collapsible(false).default_pos(WIN_POS).show(ctx, |ui| {
            widgets::color_field(ui, "Background color", &mut d.bg);
            ui.horizontal(|ui| {
                for hex in BG_SWATCHES {
                    let c = Color::parse(hex).map(|c| to_color32(&c)).unwrap_or(Color32::WHITE);
                    if widgets::color_chip(ui, c, &format!("Swatch {hex}"), d.bg.eq_ignore_ascii_case(hex)).clicked() {
                        d.bg = hex.to_string();
                    }
                }
            });
            ui.checkbox(&mut d.bg_all, "Apply to all pages");
            ui.horizontal(|ui| {
                let color = Color::parse(&d.bg);
                let targets: Vec<usize> = if d.bg_all { (0..pages).collect() } else { vec![page] };
                if widgets::primary_button(ui, "Apply Background").clicked()
                    && let Some(c) = color
                {
                    for p in &targets {
                        cmds.push(Command::SetPageBackground { page: *p, color: Some(c.clone()) });
                    }
                }
                if widgets::secondary_button(ui, "Remove Background").clicked() {
                    for p in &targets {
                        cmds.push(Command::SetPageBackground { page: *p, color: None });
                    }
                }
            });
        });
    }

    fn win_masters(&mut self, ctx: &egui::Context, open: &mut bool, cmds: &mut Vec<Command>) {
        let masters: Vec<(Id, String)> = self.session.doc().masters.iter().map(|m| (m.id, m.name.clone())).collect();
        let (page, cur) = {
            let p = self.session.doc().pages.get(self.page);
            (self.page, p.map(|p| (p.master, p.ignore_master)).unwrap_or((None, false)))
        };
        let d = &mut self.design_ui;
        if d.master_sel.is_some_and(|s| !masters.iter().any(|m| m.0 == s)) {
            d.master_sel = None;
        }
        let target = d.master_sel.or(masters.last().map(|m| m.0));
        egui::Window::new("Manage Masters").open(open).collapsible(false).default_pos(WIN_POS).show(ctx, |ui| {
            if masters.is_empty() {
                widgets::hint(ui, "No master pages yet.");
            }
            for (id, name) in &masters {
                let on = Some(*id) == target;
                let used = cur.0 == Some(*id);
                let label = name.clone();
                if scheme_row(ui, &label, on, |_, _| {}).clicked() {
                    d.master_sel = Some(*id);
                    d.master_name = name.clone();
                }
                if used {
                    widgets::hint(ui, "Used by this page");
                }
            }
            ui.add_space(4.0);
            labeled_field(ui, "Master name", &mut d.master_name);
            let named = !d.master_name.trim().is_empty();
            ui.horizontal(|ui| {
                if widgets::primary_button(ui, "New Master").clicked() && named {
                    cmds.push(Command::AddMaster { name: d.master_name.trim().to_string() });
                }
                if widgets::secondary_button(ui, "Rename Master").clicked()
                    && named
                    && let Some(m) = target
                {
                    cmds.push(Command::RenameMaster { master: m, name: d.master_name.trim().to_string() });
                }
                if widgets::secondary_button(ui, "Delete Master").clicked()
                    && let Some(m) = target
                {
                    cmds.push(Command::DeleteMaster { master: m });
                }
            });
            ui.horizontal(|ui| {
                if widgets::primary_button(ui, "Apply Master").clicked()
                    && let Some(m) = target
                {
                    cmds.push(Command::ApplyMaster { pages: Some(vec![page]), master: Some(m) });
                }
                if widgets::secondary_button(ui, "Apply Master to All Pages").clicked()
                    && let Some(m) = target
                {
                    cmds.push(Command::ApplyMaster { pages: None, master: Some(m) });
                }
                if widgets::secondary_button(ui, "No Master").clicked() {
                    cmds.push(Command::ApplyMaster { pages: Some(vec![page]), master: None });
                }
            });
            let mut ignore = cur.1;
            if ui.checkbox(&mut ignore, "Ignore Master").changed() {
                cmds.push(Command::SetIgnoreMaster { page, ignore });
            }
        });
    }

    fn win_guides(&mut self, ctx: &egui::Context, open: &mut bool, cmds: &mut Vec<Command>) {
        let (_, per) = unit_suffix(self.view.units);
        let status = &mut self.status;
        let d = &mut self.design_ui;
        egui::Window::new("Guides and Grid Setup").open(open).collapsible(false).default_pos(WIN_POS).show(ctx, |ui| {
            labeled_field(ui, "Grid columns", &mut d.cols);
            labeled_field(ui, "Grid rows", &mut d.rows);
            labeled_field(ui, "Gutter", &mut d.gutter);
            ui.horizontal(|ui| {
                if widgets::primary_button(ui, "Apply Guides").clicked() {
                    let g = d.gutter.trim();
                    let gutter = if g.is_empty() {
                        Some(0.0)
                    } else {
                        g.parse::<f64>().ok().map(|v| v * per).or_else(|| parse_length(g))
                    };
                    match (d.cols.trim().parse::<u32>(), d.rows.trim().parse::<u32>(), gutter) {
                        (Ok(columns), Ok(rows), Some(g)) => {
                            cmds.push(Command::SetGridGuides { columns, rows, gutter: Length(g) });
                        }
                        _ => *status = "Error: columns and rows must be whole numbers; check the gutter".into(),
                    }
                }
                if widgets::secondary_button(ui, "Clear Guides").clicked() {
                    d.cols = "1".into();
                    d.rows = "1".into();
                    d.gutter = "12pt".into();
                    cmds.push(Command::SetGridGuides { columns: 1, rows: 1, gutter: Length(0.0) });
                }
            });
        });
    }

    fn win_baseline(&mut self, ctx: &egui::Context, open: &mut bool, cmds: &mut Vec<Command>) {
        let status = &mut self.status;
        let d = &mut self.design_ui;
        egui::Window::new("Baseline Grid Setup").open(open).collapsible(false).default_pos(WIN_POS).show(ctx, |ui| {
            labeled_field(ui, "Baseline spacing", &mut d.base_spacing);
            labeled_field(ui, "Baseline offset", &mut d.base_offset);
            ui.horizontal(|ui| {
                if widgets::primary_button(ui, "Apply Baseline Grid").clicked() {
                    match (parse_length(&d.base_spacing), parse_length(&d.base_offset)) {
                        (Some(s), Some(o)) if s > 0.0 => {
                            cmds.push(Command::SetBaselineGrid { spacing: Length(s), offset: Length(o) });
                        }
                        _ => *status = "Error: baseline spacing and offset need units, for example 12pt".into(),
                    }
                }
                if widgets::secondary_button(ui, "Clear Baseline Grid").clicked() {
                    cmds.push(Command::ClearBaselineGrid);
                }
            });
        });
    }

    fn win_layers(&mut self, ctx: &egui::Context, open: &mut bool, cmds: &mut Vec<Command>) {
        let layers: Vec<(Id, String, bool, bool)> =
            self.session.doc().layers.iter().map(|l| (l.id, l.name.clone(), l.visible, l.locked)).collect();
        let d = &mut self.design_ui;
        if d.layer_sel.is_some_and(|s| !layers.iter().any(|l| l.0 == s)) {
            d.layer_sel = None;
        }
        egui::Window::new("Layer List").open(open).collapsible(false).default_pos(WIN_POS).show(ctx, |ui| {
            if layers.is_empty() {
                widgets::hint(ui, "No layers yet.");
            }
            // Top of the list is the top layer.
            for (i, (id, name, vis, lock)) in layers.iter().enumerate().rev() {
                ui.horizontal(|ui| {
                    if widgets::icon_button(
                        ui,
                        if *vis { ic::EYE } else { ic::EYE_SLASH },
                        &format!("Toggle visibility of {name}"),
                        !*vis,
                        true,
                    )
                    .clicked()
                    {
                        cmds.push(Command::SetLayer { layer: *id, name: None, visible: Some(!*vis), locked: None });
                    }
                    if widgets::icon_button(
                        ui,
                        if *lock { ic::LOCK } else { ic::LOCK_OPEN },
                        &format!("Toggle lock of {name}"),
                        *lock,
                        true,
                    )
                    .clicked()
                    {
                        cmds.push(Command::SetLayer { layer: *id, name: None, visible: None, locked: Some(!*lock) });
                    }
                    if scheme_row(ui, name, d.layer_sel == Some(*id), |_, _| {}).clicked() {
                        d.layer_sel = Some(*id);
                        d.layer_name = name.clone();
                    }
                    if widgets::icon_button(ui, ic::ARROW_UP, &format!("Raise {name}"), false, i + 1 < layers.len())
                        .clicked()
                    {
                        cmds.push(Command::MoveLayer { layer: *id, to: i + 1 });
                    }
                    if widgets::icon_button(ui, ic::ARROW_DOWN, &format!("Lower {name}"), false, i > 0).clicked() {
                        cmds.push(Command::MoveLayer { layer: *id, to: i - 1 });
                    }
                });
            }
            ui.add_space(4.0);
            labeled_field(ui, "Layer name", &mut d.layer_name);
            let named = !d.layer_name.trim().is_empty();
            ui.horizontal(|ui| {
                if widgets::primary_button(ui, "New Layer").clicked() && named {
                    cmds.push(Command::AddLayer { name: d.layer_name.trim().to_string() });
                }
                if widgets::secondary_button(ui, "Rename Layer").clicked()
                    && named
                    && let Some(l) = d.layer_sel
                {
                    cmds.push(Command::SetLayer {
                        layer: l,
                        name: Some(d.layer_name.trim().to_string()),
                        visible: None,
                        locked: None,
                    });
                }
                if widgets::secondary_button(ui, "Delete Layer").clicked()
                    && let Some(l) = d.layer_sel
                {
                    cmds.push(Command::DeleteLayer { layer: l });
                }
            });
        });
    }

    fn win_section(&mut self, ctx: &egui::Context, open: &mut bool, cmds: &mut Vec<Command>) {
        let page = self.page;
        let status = &mut self.status;
        let d = &mut self.design_ui;
        egui::Window::new("Page Numbering Section").open(open).collapsible(false).default_pos(WIN_POS).show(
            ctx,
            |ui| {
                widgets::hint(ui, &format!("A new numbering section starts at page {}.", page + 1));
                labeled_field(ui, "Start at", &mut d.sec_start);
                egui::ComboBox::from_label("Number format")
                    .selected_text(FORMATS.iter().find(|f| f.0 == d.sec_format).map(|f| f.1).unwrap_or(""))
                    .show_ui(ui, |ui| {
                        for (f, name) in FORMATS {
                            ui.selectable_value(&mut d.sec_format, f, name);
                        }
                    });
                ui.horizontal(|ui| {
                    if widgets::primary_button(ui, "Apply Section").clicked() {
                        match d.sec_start.trim().parse::<u32>() {
                            Ok(start_at) => cmds.push(Command::SetSection { page, start_at, format: d.sec_format }),
                            Err(_) => *status = "Error: start number must be a whole number".into(),
                        }
                    }
                    if widgets::secondary_button(ui, "Remove Section").clicked() {
                        cmds.push(Command::RemoveSection { page });
                    }
                });
            },
        );
    }
}

/// A full-width selectable row showing `name`, with optional painted colour chips on the right. The name is its
/// accessible label.
fn scheme_row(
    ui: &mut egui::Ui,
    name: &str,
    selected: bool,
    paint: impl FnOnce(&egui::Painter, &[egui::Rect]),
) -> egui::Response {
    use egui::{CornerRadius, Sense, Stroke, StrokeKind, Vec2};
    let p = widgets::pal(ui);
    let w = ui.available_width().max(220.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 28.0), Sense::click());
    let kind = if selected { egui::WidgetType::SelectableLabel } else { egui::WidgetType::Button };
    let label = name.to_string();
    resp.widget_info(move || {
        let mut i = egui::WidgetInfo::labeled(kind, true, &label);
        i.selected = selected.then_some(true);
        i
    });
    if ui.is_rect_visible(rect) {
        let fill = if selected {
            p.primary_soft
        } else if resp.hovered() {
            p.surface_alt
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, CornerRadius::same(6), fill);
        if selected {
            ui.painter().rect_stroke(
                rect,
                CornerRadius::same(6),
                Stroke::new(1.0, p.primary.gamma_multiply(0.4)),
                StrokeKind::Inside,
            );
        }
        ui.painter().text(
            rect.left_center() + Vec2::new(10.0, 0.0),
            egui::Align2::LEFT_CENTER,
            name,
            crate::theme::medium(12.5),
            if selected { p.primary } else { p.text },
        );
        let chips: Vec<egui::Rect> = (0..6)
            .map(|i| {
                let x = rect.right() - 10.0 - (6 - i) as f32 * 18.0 + 4.0;
                egui::Rect::from_min_size(egui::pos2(x, rect.center().y - 7.0), Vec2::splat(14.0))
            })
            .collect();
        paint(ui.painter(), &chips);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}
