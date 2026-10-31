//! The window frame around the canvas: gradient header with quick actions, tabbed ribbon, status bar.

use crate::widgets::{self, group, ribbon_button};
use crate::{Dialog, NewpubApp, Tool, icons as ic, print, theme};
use egui::{Align2, Color32, Sense, Stroke, Vec2};
use newpub_engine::SessionAction;
use newpub_engine::core::{Command, ZOp};

/// Ribbon tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RibbonTab {
    #[default]
    Home,
    Insert,
    PageDesign,
    Mailings,
    Review,
    View,
}

impl RibbonTab {
    pub const ALL: [(RibbonTab, &'static str); 6] = [
        (RibbonTab::Home, "Home"),
        (RibbonTab::Insert, "Insert"),
        (RibbonTab::PageDesign, "Page Design"),
        (RibbonTab::Mailings, "Mailings"),
        (RibbonTab::Review, "Review"),
        (RibbonTab::View, "View"),
    ];
}

impl NewpubApp {
    /// Opens the Save dialog with the current file name.
    pub(crate) fn open_save(&mut self) {
        let path = self
            .session
            .path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "publication.npub".into());
        self.dialog = Dialog::Save { path };
    }

    pub(crate) fn open_export_pdf(&mut self) {
        self.dialog = Dialog::ExportPdf { path: "publication.pdf".into(), crop_marks: false, booklet: false };
    }

    /// Document name shown in the header.
    pub(crate) fn doc_title(&self) -> String {
        self.session
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().to_string())
            .or_else(|| Some(self.session.doc().meta.title.clone()).filter(|t| !t.trim().is_empty()))
            .unwrap_or_else(|| "Untitled publication".into())
    }

    /// The brand header: logo, document name, quick actions.
    pub(crate) fn header(&mut self, ui: &mut egui::Ui) {
        let p = widgets::pal(ui);
        let rect = ui.max_rect();
        theme::gradient_rect(ui.painter(), rect, p.grad_a, p.grad_b, 0.0);
        ui.horizontal_centered(|ui| {
            ui.add_space(12.0);
            // Logo: the page glyph in a soft white tile.
            let (logo, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
            ui.painter().rect_filled(logo, 7.0, Color32::from_white_alpha(40));
            ui.painter().text(
                logo.center(),
                Align2::CENTER_CENTER,
                ic::NEWSPAPER,
                theme::icon_font(18.0),
                Color32::WHITE,
            );
            ui.add_space(4.0);
            ui.label(egui::RichText::new("newpub").font(theme::semibold(15.0)).color(Color32::WHITE));
            ui.add_space(10.0);
            let (sep, _) = ui.allocate_exact_size(Vec2::new(1.0, 20.0), Sense::hover());
            ui.painter().rect_filled(sep, 0.0, Color32::from_white_alpha(70));
            ui.add_space(10.0);
            let title = self.doc_title();
            ui.label(egui::RichText::new(title).font(theme::medium(13.0)).color(Color32::from_white_alpha(235)));
            ui.add_space(14.0);
            if widgets::header_button(ui, ic::FILE_PLUS, "New", true).clicked() {
                self.guard(crate::guard::Pending::New);
            }
            if widgets::header_button(ui, ic::FOLDER_OPEN, "Open", true).clicked() {
                self.guard(crate::guard::Pending::OpenDialog);
            }
            if widgets::header_button(ui, ic::FLOPPY_DISK, "Save", true).clicked() {
                self.open_save();
            }
            ui.add_space(6.0);
            if widgets::header_button(ui, ic::ARROW_U_UP_LEFT, "Undo", self.session.can_undo()).clicked() {
                self.act(SessionAction::Undo);
            }
            if widgets::header_button(ui, ic::ARROW_U_UP_RIGHT, "Redo", self.session.can_redo()).clicked() {
                self.act(SessionAction::Redo);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(12.0);
                if widgets::header_pill(ui, ic::FILE_PDF, "Export PDF", true).clicked() {
                    self.open_export_pdf();
                }
                ui.add_space(4.0);
                if widgets::header_pill(ui, ic::EXPORT, "Export As", false).clicked() {
                    self.dialog = Dialog::Export(crate::files::ExportState::default());
                }
                ui.add_space(4.0);
                if widgets::header_pill(ui, ic::PRINTER, "Print", false).clicked() {
                    self.dialog = Dialog::Print(print::PrintState::new());
                }
            });
        });
    }

    /// Tab strip and the active tab's groups.
    pub(crate) fn ribbon(&mut self, ui: &mut egui::Ui) {
        let p = widgets::pal(ui);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.add_space(6.0);
            for (tab, label) in RibbonTab::ALL {
                let on = self.ribbon_tab == tab;
                let font = theme::medium(13.0);
                let galley = ui.painter().layout_no_wrap(label.to_string(), font.clone(), p.text);
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x + 22.0, 30.0), Sense::click());
                resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, label));
                if resp.hovered() && !on {
                    ui.painter().rect_filled(rect.shrink2(Vec2::new(0.0, 3.0)), 6.0, p.surface_alt);
                }
                let fg = if on { p.primary } else { p.text_muted };
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, font, fg);
                if on {
                    let bar = egui::Rect::from_min_max(
                        egui::pos2(rect.left() + 8.0, rect.bottom() - 3.0),
                        egui::pos2(rect.right() - 8.0, rect.bottom() - 0.5),
                    );
                    ui.painter().rect_filled(bar, 2.0, p.primary);
                }
                if resp.clicked() {
                    self.ribbon_tab = tab;
                }
            }
        });
        let (line, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
        ui.painter().hline(line.x_range(), line.center().y, Stroke::new(1.0, p.border));
        ui.add_space(4.0);
        egui::ScrollArea::horizontal().id_salt("ribbon-scroll").show(ui, |ui| {
            ui.horizontal(|ui| {
                // Every tab is the same height, so the canvas does not jump when switching tabs.
                ui.set_height(76.0);
                ui.add_space(6.0);
                match self.ribbon_tab {
                    RibbonTab::Home => self.home_tab(ui),
                    RibbonTab::Insert => self.insert_tab(ui),
                    RibbonTab::PageDesign => self.design_tab(ui),
                    RibbonTab::Mailings => self.mailings_tab(ui),
                    RibbonTab::Review => self.review_tab(ui),
                    RibbonTab::View => self.view_tab(ui),
                }
            });
        });
        ui.add_space(2.0);
    }

    fn home_tab(&mut self, ui: &mut egui::Ui) {
        group(ui, "Tools", |ui| {
            for (tool, icon, label) in [(Tool::Select, ic::CURSOR, "Select"), (Tool::TextBox, ic::TEXT_T, "Text Box")] {
                if ribbon_button(ui, icon, label, self.tool == tool, true).clicked() {
                    self.tool = tool;
                }
            }
            if ribbon_button(ui, ic::IMAGE, "Picture", false, true).clicked() {
                self.dialog = Dialog::InsertPicture { path: String::new() };
            }
        });
        group(ui, "Text", |ui| {
            if ribbon_button(ui, ic::TEXT_AA, "Styles", self.styles_ui.open, true)
                .on_hover_text("Make, apply and delete paragraph and character styles")
                .clicked()
            {
                self.styles_ui.open = !self.styles_ui.open;
            }
        });
        group(ui, "Shapes", |ui| {
            for (tool, icon, label) in [
                (Tool::Rectangle, ic::SQUARE, "Rectangle"),
                (Tool::Ellipse, ic::CIRCLE, "Ellipse"),
                (Tool::Line, ic::LINE_SEGMENT, "Line"),
                (Tool::Freeform, ic::PEN_NIB, "Freeform"),
            ] {
                if ribbon_button(ui, icon, label, self.tool == tool, true).clicked() {
                    self.tool = tool;
                }
            }
            self.freeform_controls(ui);
        });
        group(ui, "Pages", |ui| {
            if ribbon_button(ui, ic::FILE_PLUS, "Add Page", false, true).clicked() {
                let at = self.page + 1;
                if self.act(Command::InsertPages { at: Some(at), count: 1, master: None }).is_some() {
                    self.page = at;
                }
            }
            let many = self.session.doc().pages.len() > 1;
            if ribbon_button(ui, ic::FILE_MINUS, "Delete Page", false, many).clicked() {
                let p = self.page;
                self.act(Command::DeletePage { page: p });
            }
        });
        let has = !self.selection.is_empty();
        group(ui, "Arrange", |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                if widgets::small_button(ui, ic::STACK_PLUS, "Bring to Front", false, has).clicked() {
                    self.reorder(ZOp::Front);
                }
                if widgets::small_button(ui, ic::STACK_MINUS, "Send to Back", false, has).clicked() {
                    self.reorder(ZOp::Back);
                }
            });
            if ribbon_button(ui, ic::STACK, "Selection Pane", self.pane.open, true).clicked() {
                self.pane.open = !self.pane.open;
            }
            if has && ribbon_button(ui, ic::TRASH, "Delete Object", false, true).clicked() {
                let ids = std::mem::take(&mut self.selection);
                self.act(Command::DeleteObjects { ids });
            }
        });
    }

    fn view_tab(&mut self, ui: &mut egui::Ui) {
        group(ui, "Zoom", |ui| {
            if ribbon_button(ui, ic::ARROWS_OUT, "Whole Page", false, true).clicked() {
                self.fit_page();
            }
            if ribbon_button(ui, ic::FRAME_CORNERS, "100%", false, true).clicked() {
                self.actual_size();
            }
        });
        group(ui, "Show", |ui| {
            if ribbon_button(ui, ic::STACK, "Selection Pane", self.pane.open, true).clicked() {
                self.pane.open = !self.pane.open;
            }
            if ribbon_button(ui, ic::BOUNDING_BOX, "Boundaries", self.view.boundaries, true).clicked() {
                self.view.boundaries = !self.view.boundaries;
            }
        });
        group(ui, "Appearance", |ui| {
            let dark = ui.visuals().dark_mode;
            let (icon, label) = if dark { (ic::SUN, "Light Mode") } else { (ic::MOON, "Dark Mode") };
            if ribbon_button(ui, icon, label, false, true).clicked() {
                ui.ctx().set_theme(if dark { egui::Theme::Light } else { egui::Theme::Dark });
                self.recent.set_dark_mode(!dark);
                self.texture = None;
            }
        });
    }

    /// Status bar: page, message, units, spread, zoom.
    pub(crate) fn status_bar(&mut self, ui: &mut egui::Ui) {
        let p = widgets::pal(ui);
        ui.horizontal_centered(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(ic::FILE).font(theme::icon_font(14.0)).color(p.text_muted));
            ui.label(
                egui::RichText::new(format!("Page {} of {}", self.page + 1, self.session.doc().pages.len()))
                    .color(p.text_muted)
                    .size(12.0),
            );
            // Live count of a multiple selection (it never goes stale, unlike `status`).
            if self.selection.len() > 1 {
                ui.add_space(8.0);
                ui.label(egui::RichText::new(ic::SELECTION).font(theme::icon_font(14.0)).color(p.text_muted));
                ui.label(
                    egui::RichText::new(format!("{} objects selected", self.selection.len()))
                        .color(p.text_muted)
                        .size(12.0),
                );
            }
            if !self.status.is_empty() {
                ui.add_space(8.0);
                let err = self.status.starts_with("Error");
                let (icon, color) = if err { (ic::WARNING_CIRCLE, p.danger) } else { (ic::CHECK_CIRCLE, p.success) };
                ui.label(egui::RichText::new(icon).font(theme::icon_font(14.0)).color(color));
                ui.label(egui::RichText::new(&self.status).size(12.0).color(if err { p.danger } else { p.text_muted }));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                if widgets::icon_button(ui, ic::MAGNIFYING_GLASS_PLUS, "Zoom In", false, true).clicked() {
                    self.zoom_step(1);
                }
                ui.label(egui::RichText::new(format!("{:.0}%", self.zoom * 100.0)).size(12.0).color(p.text));
                if widgets::icon_button(ui, ic::MAGNIFYING_GLASS_MINUS, "Zoom Out", false, true).clicked() {
                    self.zoom_step(-1);
                }
                if widgets::icon_button(ui, ic::FRAME_CORNERS, "Actual size", false, true).clicked() {
                    self.actual_size();
                }
                if widgets::icon_button(ui, ic::ARROWS_OUT, "Fit page", self.fitted, true).clicked() {
                    self.fit_page();
                }
                if widgets::icon_button(ui, ic::BOOK_OPEN, "Two-page spread", self.view.spread, true).clicked() {
                    self.view.spread = !self.view.spread;
                    self.reset_scroll();
                }
                ui.add_space(6.0);
                self.units_combo(ui);
            });
        });
    }
}
