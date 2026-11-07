//! The start screen, "Choose a template" (UI-08): a blank publication in a chosen size, a built-in template
//! shown as a rendered preview, or a publication type (business cards, labels, envelopes…).

use crate::{NewpubApp, PRESETS, icons as ic, labeled_field, parse_len, theme, widgets};
use egui::{Align2, Color32, CornerRadius, Sense, Stroke, StrokeKind, Vec2};
use newpub_engine::core::Insets;
use newpub_engine::{Query, Session, SessionAction};

/// What the start screen has selected.
#[derive(Clone, Debug, PartialEq)]
enum Choice {
    Blank,
    Template(String),
    /// One of the user's own templates (its file).
    Mine(String),
    PubType(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PickerState {
    choice: Choice,
    preset: usize,
    width: String,
    height: String,
    /// Built-in templates (id, name, pages), read once when the picker opens.
    templates: Vec<(String, String, u64)>,
    /// Publication types (id, name).
    types: Vec<(String, String)>,
    /// The user's own templates (name, file), from "Save as Template".
    mine: Vec<(String, String)>,
}

impl PickerState {
    pub fn new(app: &mut NewpubApp) -> PickerState {
        let list = |app: &mut NewpubApp, q: Query| -> Vec<serde_json::Value> {
            app.session.query(&q).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default()
        };
        let templates = list(app, Query::BuiltinTemplates)
            .iter()
            .filter_map(|t| {
                Some((
                    t.get("id")?.as_str()?.to_string(),
                    t.get("name")?.as_str()?.to_string(),
                    t.get("pages").and_then(|p| p.as_u64()).unwrap_or(0),
                ))
            })
            .collect();
        let types = list(app, Query::PublicationTypes)
            .iter()
            .filter_map(|t| Some((t.get("id")?.as_str()?.to_string(), t.get("name")?.as_str()?.to_string())))
            .collect();
        let mut mine: Vec<(String, String)> = app
            .user_templates
            .as_ref()
            .and_then(|d| std::fs::read_dir(d).ok())
            .into_iter()
            .flatten()
            .filter_map(|e| {
                let p = e.ok()?.path();
                if p.extension()? != "npubt" {
                    return None;
                }
                Some((p.file_stem()?.to_string_lossy().to_string(), p.to_string_lossy().to_string()))
            })
            .collect();
        mine.sort();
        PickerState {
            mine,
            choice: Choice::Blank,
            preset: 0,
            width: PRESETS[0].1.into(),
            height: PRESETS[0].2.into(),
            templates,
            types,
        }
    }
}

/// Renders the first page of a built-in template into a texture (cached on the app).
fn preview(app: &mut NewpubApp, ctx: &egui::Context, id: &str) -> Option<egui::TextureHandle> {
    if let Some(t) = app.template_previews.get(id) {
        return Some(t.clone());
    }
    let mut s = Session::bundled();
    let action = match id.strip_prefix("file:") {
        Some(path) => SessionAction::NewFromTemplate { path: path.to_string() },
        None => SessionAction::NewFromBuiltin { id: id.to_string() },
    };
    s.run(&action.into()).ok()?;
    let w = s.doc().setup.width.0;
    let pm = s.render_page(0, 72.0 * 300.0 / w.max(1.0)).ok()?;
    let img = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], pm.data());
    let tex = ctx.load_texture(format!("template-{id}"), img, egui::TextureOptions::LINEAR);
    app.template_previews.insert(id.to_string(), tex.clone());
    Some(tex)
}

/// A selectable card; `label` is its accessible name.
fn card(ui: &mut egui::Ui, size: Vec2, label: &str, selected: bool) -> (egui::Response, egui::Rect) {
    let p = widgets::pal(ui);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, label));
    let fill = if selected {
        p.primary_soft
    } else if resp.hovered() {
        p.surface_alt
    } else {
        p.surface
    };
    ui.painter().rect_filled(rect, CornerRadius::same(12), fill);
    let stroke = if selected { Stroke::new(2.0, p.primary) } else { Stroke::new(1.0, p.border) };
    ui.painter().rect_stroke(rect, CornerRadius::same(12), stroke, StrokeKind::Inside);
    (resp.on_hover_cursor(egui::CursorIcon::PointingHand), rect)
}

/// Draws the start screen; returns true when it should close.
pub fn show(app: &mut NewpubApp, ctx: &egui::Context, st: &mut PickerState) -> bool {
    let mut close = false;
    let p = theme::palette(ctx);
    let frame =
        egui::Frame::new().fill(p.surface).corner_radius(CornerRadius::same(16)).stroke(Stroke::new(1.0, p.border));
    // A fixed position: re-centring on the measured size each frame would move widgets between frames.
    let screen = ctx.content_rect();
    let pos = egui::pos2(
        (screen.center().x - 454.0).max(screen.left() + 8.0),
        (screen.center().y - 330.0).max(screen.top() + 8.0),
    );
    let id = egui::Id::new("start-screen");
    egui::Modal::new(id)
        .area(
            egui::Area::new(id)
                .kind(egui::UiKind::Modal)
                .sense(Sense::hover())
                .order(egui::Order::Foreground)
                .interactable(true)
                .fixed_pos(pos),
        )
        .frame(frame)
        .backdrop_color(Color32::from_black_alpha(110))
        .show(ctx, |ui| {
            ui.set_width(908.0);
            // Hero band.
            let (hero, _) = ui.allocate_exact_size(Vec2::new(908.0, 92.0), Sense::hover());
            // Rounded top corners only: a rounded gradient, then square corners over its bottom edge.
            theme::gradient_rect(ui.painter(), hero, p.grad_a, p.grad_b, 16.0);
            theme::gradient_rect(
                ui.painter(),
                egui::Rect::from_min_max(egui::pos2(hero.left(), hero.bottom() - 16.0), hero.max),
                p.grad_a,
                p.grad_b,
                0.0,
            );
            ui.painter().text(
                hero.left_top() + Vec2::new(28.0, 22.0),
                Align2::LEFT_TOP,
                ic::SPARKLE,
                theme::icon_font(26.0),
                Color32::WHITE,
            );
            let title = ui.painter().text(
                hero.left_top() + Vec2::new(64.0, 20.0),
                Align2::LEFT_TOP,
                "Choose a template",
                theme::semibold(21.0),
                Color32::WHITE,
            );
            let r = ui.interact(title, ui.id().with("title"), Sense::hover());
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, "Choose a template"));
            ui.painter().text(
                hero.left_top() + Vec2::new(64.0, 52.0),
                Align2::LEFT_TOP,
                "Start from a ready-made design, or with a blank page in the size you need.",
                egui::FontId::proportional(13.0),
                Color32::from_white_alpha(225),
            );
            egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 18)).show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    // Left column: blank publication and publication types.
                    ui.vertical(|ui| {
                        ui.set_width(250.0);
                        section_title(ui, "Blank");
                        let blank = st.choice == Choice::Blank;
                        let (resp, rect) = card(ui, Vec2::new(250.0, 64.0), "Blank publication", blank);
                        let painter = ui.painter();
                        let page =
                            egui::Rect::from_min_size(rect.left_top() + Vec2::new(14.0, 10.0), Vec2::new(34.0, 44.0));
                        painter.rect_filled(page, 3.0, Color32::WHITE);
                        painter.rect_stroke(page, 3.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
                        painter.text(page.center(), Align2::CENTER_CENTER, ic::PLUS, theme::icon_font(16.0), p.primary);
                        painter.text(
                            rect.left_top() + Vec2::new(60.0, 16.0),
                            Align2::LEFT_TOP,
                            "Blank publication",
                            theme::medium(13.5),
                            p.text,
                        );
                        painter.text(
                            rect.left_top() + Vec2::new(60.0, 36.0),
                            Align2::LEFT_TOP,
                            format!("{} × {}", st.width, st.height),
                            egui::FontId::proportional(12.0),
                            p.text_muted,
                        );
                        if resp.clicked() {
                            st.choice = Choice::Blank;
                        }
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            for (i, (name, w, h)) in PRESETS.iter().enumerate() {
                                let on = st.choice == Choice::Blank && st.preset == i;
                                if widgets::small_button(ui, ic::FILE, name, on, true).clicked() {
                                    st.choice = Choice::Blank;
                                    st.preset = i;
                                    if !w.is_empty() {
                                        st.width = w.to_string();
                                        st.height = h.to_string();
                                    }
                                }
                            }
                        });
                        ui.add_space(4.0);
                        labeled_field(ui, "Page width", &mut st.width);
                        labeled_field(ui, "Page height", &mut st.height);
                        ui.add_space(10.0);
                        section_title(ui, "More publication types");
                        egui::ScrollArea::vertical().id_salt("types").max_height(170.0).show(ui, |ui| {
                            for (id, name) in &st.types {
                                let on = st.choice == Choice::PubType(id.clone());
                                if widgets::small_button(ui, ic::CARDS, name, on, true).clicked() {
                                    st.choice = Choice::PubType(id.clone());
                                }
                            }
                        });
                    });
                    ui.add_space(22.0);
                    // Right: template gallery.
                    ui.vertical(|ui| {
                        let mine: Vec<(String, String, String, Choice)> = st
                            .mine
                            .iter()
                            .map(|(name, path)| {
                                (format!("file:{path}"), name.clone(), "My template".into(), Choice::Mine(path.clone()))
                            })
                            .collect();
                        if !mine.is_empty() {
                            section_title(ui, "My templates");
                            close |= gallery(app, ui, st, "mine", &mine);
                            ui.add_space(10.0);
                        }
                        section_title(ui, "Templates");
                        let builtin: Vec<(String, String, String, Choice)> = st
                            .templates
                            .iter()
                            .map(|(id, name, pages)| {
                                let sub = format!("{pages} page{}", if *pages == 1 { "" } else { "s" });
                                (id.clone(), name.clone(), sub, Choice::Template(id.clone()))
                            })
                            .collect();
                        close |= gallery(app, ui, st, "templates", &builtin);
                    });
                });
                ui.add_space(14.0);
                ui.separator();
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    widgets::hint(ui, "Tip: double-click a template to start right away.");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::primary_button(ui, "Create").clicked() {
                            close = create(app, st);
                        }
                        if widgets::secondary_button(ui, "Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            });
        });
    close
}

/// A grid of template cards (preview key, name, subtitle, choice); true when a double-click created a publication.
fn gallery(
    app: &mut NewpubApp,
    ui: &mut egui::Ui,
    st: &mut PickerState,
    salt: &str,
    items: &[(String, String, String, Choice)],
) -> bool {
    let p = widgets::pal(ui);
    let mut close = false;
    egui::Grid::new(salt).spacing(Vec2::new(14.0, 14.0)).show(ui, |ui| {
        for (k, (key, name, sub, choice)) in items.iter().enumerate() {
            let on = st.choice == *choice;
            let (resp, rect) = card(ui, Vec2::new(122.0, 196.0), name, on);
            let thumb_r = egui::Rect::from_min_size(rect.left_top() + Vec2::new(12.0, 12.0), Vec2::new(98.0, 127.0));
            let shadow =
                egui::epaint::Shadow { offset: [0, 3], blur: 10, spread: 0, color: Color32::from_black_alpha(40) };
            ui.painter().add(shadow.as_shape(thumb_r, 2.0));
            match preview(app, ui.ctx(), key) {
                Some(t) => {
                    let size = t.size_vec2();
                    let fitted = size * (thumb_r.width() / size.x).min(thumb_r.height() / size.y);
                    let img = egui::Rect::from_center_size(thumb_r.center(), fitted);
                    ui.painter().image(
                        t.id(),
                        img,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                None => {
                    ui.painter().rect_filled(thumb_r, 2.0, Color32::WHITE);
                }
            }
            ui.painter().text(
                rect.left_top() + Vec2::new(12.0, 150.0),
                Align2::LEFT_TOP,
                name,
                theme::medium(12.5),
                p.text,
            );
            ui.painter().text(
                rect.left_top() + Vec2::new(12.0, 170.0),
                Align2::LEFT_TOP,
                sub,
                egui::FontId::proportional(11.5),
                p.text_muted,
            );
            if resp.clicked() {
                st.choice = choice.clone();
            }
            if resp.double_clicked() {
                st.choice = choice.clone();
                close = create(app, st);
            }
            if k % 4 == 3 {
                ui.end_row();
            }
        }
    });
    close
}

fn section_title(ui: &mut egui::Ui, text: &str) {
    let p = widgets::pal(ui);
    ui.label(egui::RichText::new(text.to_uppercase()).font(theme::semibold(11.0)).color(p.text_muted));
    ui.add_space(2.0);
}

fn create(app: &mut NewpubApp, st: &PickerState) -> bool {
    let action = match &st.choice {
        Choice::Template(id) => SessionAction::NewFromBuiltin { id: id.clone() },
        Choice::Mine(path) => SessionAction::NewFromTemplate { path: path.clone() },
        Choice::PubType(id) => SessionAction::NewFromPublicationType { id: id.clone() },
        Choice::Blank => match (parse_len(&st.width), parse_len(&st.height)) {
            (Some(width), Some(height)) => SessionAction::NewDocument {
                width,
                height,
                margins: Some(Insets::uniform(36.0)),
                facing: false,
                pages: 1,
                bleed: None,
            },
            _ => {
                app.status = "Enter a page size such as 8.5in or 210mm".into();
                return false;
            }
        },
    };
    if app.act(action).is_none() {
        return false;
    }
    app.page = 0;
    app.selection.clear();
    app.end_text_edit();
    app.view.reset_transient();
    app.reset_gesture();
    app.fit_page();
    true
}
