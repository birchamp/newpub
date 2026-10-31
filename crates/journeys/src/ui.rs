//! UI journeys: drive the real egui app through its AccessKit tree with egui_kittest.
//!
//! UI steps: `click: {label, button}`, `fill: {label, text}`, `type: {text}`, `key: {key, command, shift}`,
//! `drag: {from: [x, y], to: [x, y], shift}` (or `from_ruler`/`to_ruler: top | left` for one end) and `click_at: [x, y]` (page points on the canvas),
//! `expect_ui: {label, exists}`, `focus: {label}` (keyboard focus), `scroll: {dx, dy}` (mouse wheel over
//! the canvas, screen points), `hover: {label}`, `close_window: {}` (the window's close button), `drop_file: {path, at}` (a file
//! dragged from the desktop and dropped, optionally over a page point), `restart_app: {}` (the app goes away
//! without a clean exit and starts again), `run: {}`.
//! Journey options: `startup_template_picker: true` starts the app as the desktop binary does;
//! print jobs go to `<out>/print-spool/job-<n>.pdf`.
//! Observation steps are shared with headless journeys: `expect`, `let`, `dump`, `expect_pdf`,
//! `expect_png`, `expect_roundtrip` (they query the app's session).

use crate::runner::{self, Ctx, substitute};
use anyhow::{Result, anyhow, bail};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use newpub_app::NewpubApp;
use newpub_engine::Session;
use serde_json::Value;

/// A file dropped on the window from the desktop.
#[derive(Debug)]
struct DroppedPath(std::path::PathBuf);

impl egui::DroppedFile for DroppedPath {
    fn path(&self) -> &std::path::Path {
        &self.0
    }

    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.0).map_err(|e| e.to_string())
    }
}

fn key_from(name: &str) -> Option<egui::Key> {
    egui::Key::from_name(name)
}

fn settle(h: &mut Harness<'_, NewpubApp>) {
    // The app does not animate; a few steps let windows open and layouts settle.
    h.run_steps(3);
}

fn pt(v: &Value) -> Result<(f64, f64)> {
    let a = v.as_array().ok_or_else(|| anyhow!("expected [x, y]"))?;
    let n = |i: usize| {
        a.get(i).and_then(|x| x.as_f64().or_else(|| x.as_str().and_then(newpub_engine::core::units::parse_length)))
    };
    Ok((n(0).ok_or_else(|| anyhow!("bad x"))?, n(1).ok_or_else(|| anyhow!("bad y"))?))
}

fn find<'a>(h: &'a Harness<'_, NewpubApp>, label: &'a str) -> Result<egui_kittest::Node<'a>> {
    // Prefer interactive nodes when several share a label (e.g. a text field and its caption).
    let all: Vec<_> = h.query_all_by_label(label).collect();
    if all.is_empty() {
        bail!("no UI element labelled {label:?}");
    }
    // A row's caption and its control share a name; the control is what a person uses.
    use egui::accesskit::Role;
    let pick = all
        .iter()
        .position(|n| n.accesskit_node().role() == Role::TextInput)
        .or_else(|| all.iter().position(|n| n.accesskit_node().role() == Role::ComboBox))
        .unwrap_or(0);
    Ok(all.into_iter().nth(pick).expect("index in range"))
}

/// Scrolls the widget labelled `label` into view the way a person would: with the mouse wheel over the scrolled
/// area. Widgets already on screen are left alone.
fn ensure_visible(h: &mut Harness<'_, NewpubApp>, label: &str) -> Result<()> {
    let screen = h.ctx.content_rect();
    for _ in 0..40 {
        let r = find(h, label)?.rect();
        // The status bar covers the bottom 30 points of the window.
        let (below, above) = (r.max.y > screen.max.y - 40.0, r.min.y < screen.min.y);
        if !below && !above {
            return Ok(());
        }
        let at = egui::pos2(r.center().x, if below { screen.max.y - 80.0 } else { screen.min.y + 80.0 });
        h.hover_at(at);
        h.run_steps(1);
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, if below { -60.0 } else { 60.0 }),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        // Wheel scrolling animates; wait for it to finish so the widget is where the next lookup finds it.
        h.run_steps(12);
    }
    Ok(())
}

fn ui_step(h: &mut Harness<'_, NewpubApp>, ctx: &mut Ctx, step: &Value) -> Result<()> {
    let m = step.as_object().ok_or_else(|| anyhow!("step must be a map"))?;
    let (name, raw) = m.iter().find(|(k, _)| *k != "as").ok_or_else(|| anyhow!("empty step"))?;
    let args = substitute(raw, ctx)?;
    match name.as_str() {
        "click" => {
            // click: {label} or {label, button: secondary} (a right-click, e.g. for a context menu).
            let label = args.get("label").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("click needs label"))?;
            if args.get("button").and_then(|v| v.as_str()) == Some("secondary") {
                let at = find(h, label)?.rect().center();
                h.hover_at(at);
                h.run_steps(1);
                for pressed in [true, false] {
                    h.event(egui::Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Secondary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    });
                    h.run_steps(1);
                }
            } else {
                ensure_visible(h, label)?;
                find(h, label)?.click();
            }
            settle(h);
        }
        "fill" => {
            let label = args.get("label").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("fill needs label"))?;
            let text = args.get("text").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("fill needs text"))?;
            ensure_visible(h, label)?;
            find(h, label)?.focus();
            settle(h);
            h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
            h.run_steps(1);
            find(h, label)?.type_text(text);
            settle(h);
        }
        "hover" => {
            // hover: {label} — the pointer rests over a widget (e.g. a gallery item).
            let label = args.get("label").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("hover needs label"))?;
            let at = find(h, label)?.rect().center();
            h.hover_at(at);
            settle(h);
        }
        "close_window" => {
            h.input_mut().viewports.entry(egui::ViewportId::ROOT).or_default().events.push(egui::ViewportEvent::Close);
            settle(h);
        }
        "drop_file" => {
            let p = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("drop_file needs path"))?;
            let path = std::path::absolute(ctx.out.join(p))?;
            if let Some(at) = args.get("at") {
                let (x, y) = pt(at)?;
                let pos = h.state().page_to_screen(x, y);
                h.hover_at(pos);
                h.run_steps(1);
            }
            h.input_mut().dropped_files.push(std::sync::Arc::new(DroppedPath(path)));
            settle(h);
        }
        "paste" => {
            // The OS clipboard delivering text (Cmd+V with text on the system clipboard).
            let text = args.get("text").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("paste needs text"))?;
            h.event(egui::Event::Paste(text.to_string()));
            settle(h);
        }
        "type" => {
            let text = args.get("text").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("type needs text"))?;
            h.event(egui::Event::Text(text.to_string()));
            settle(h);
        }
        "key" => {
            let k = args.get("key").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("key needs key"))?;
            let key = key_from(k).ok_or_else(|| anyhow!("unknown key {k}"))?;
            let mut mods = egui::Modifiers::NONE;
            if args.get("command").and_then(|v| v.as_bool()).unwrap_or(false) {
                mods |= egui::Modifiers::COMMAND;
            }
            if args.get("shift").and_then(|v| v.as_bool()).unwrap_or(false) {
                mods |= egui::Modifiers::SHIFT;
            }
            h.key_press_modifiers(mods, key);
            settle(h);
        }
        "drag" => {
            // Page points; `from_ruler`/`to_ruler: top | left` start or end on a ruler, level with the other end.
            // `shift: true` holds Shift for the whole drag, as on a real keyboard.
            let shift = args.get("shift").and_then(|v| v.as_bool()).unwrap_or(false);
            let ruler = |h: &Harness<'_, NewpubApp>, which: &str| -> Result<egui::Rect> {
                let label = match which {
                    "top" => "Horizontal ruler",
                    "left" => "Vertical ruler",
                    other => bail!("unknown ruler {other:?} (top or left)"),
                };
                Ok(find(h, label)?.rect())
            };
            let on_ruler = |r: egui::Rect, which: &str, p: egui::Pos2| {
                if which == "top" { egui::pos2(p.x, r.center().y) } else { egui::pos2(r.center().x, p.y) }
            };
            let from_ruler = args.get("from_ruler").and_then(|v| v.as_str()).map(str::to_string);
            let to_ruler = args.get("to_ruler").and_then(|v| v.as_str()).map(str::to_string);
            let (a, b) = match (&from_ruler, &to_ruler) {
                (Some(w), None) => {
                    let (x1, y1) = pt(args.get("to").ok_or_else(|| anyhow!("drag needs to"))?)?;
                    let b = h.state().page_to_screen(x1, y1);
                    (on_ruler(ruler(h, w)?, w, b), b)
                }
                (None, Some(w)) => {
                    let (x0, y0) = pt(args.get("from").ok_or_else(|| anyhow!("drag needs from"))?)?;
                    let a = h.state().page_to_screen(x0, y0);
                    (a, on_ruler(ruler(h, w)?, w, a))
                }
                _ => {
                    let (x0, y0) = pt(args.get("from").ok_or_else(|| anyhow!("drag needs from"))?)?;
                    let (x1, y1) = pt(args.get("to").ok_or_else(|| anyhow!("drag needs to"))?)?;
                    (h.state().page_to_screen(x0, y0), h.state().page_to_screen(x1, y1))
                }
            };
            h.hover_at(a);
            h.run_steps(1);
            if shift {
                h.event(egui::Event::ModifiersChanged(egui::Modifiers::SHIFT));
                h.run_steps(1);
                h.event(egui::Event::PointerButton {
                    pos: a,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::SHIFT,
                });
            } else {
                h.drag_at(a);
            }
            h.run_steps(1);
            for i in 1..=4 {
                h.hover_at(a + (b - a) * (i as f32 / 4.0));
                h.run_steps(1);
            }
            if shift {
                h.event(egui::Event::PointerButton {
                    pos: b,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::SHIFT,
                });
                h.run_steps(1);
                h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
                h.remove_cursor();
            } else {
                h.drop_at(b);
            }
            settle(h);
        }
        "click_at" => {
            // click_at: [x, y] or {at: [x, y], button: secondary, double: true, shift: true} on the current page.
            let at = args.get("at").cloned().unwrap_or_else(|| args.clone());
            let (x, y) = pt(&at)?;
            let p = h.state().page_to_screen(x, y);
            let flag = |k: &str| args.get(k).and_then(|v| v.as_bool()).unwrap_or(false);
            let secondary = args.get("button").and_then(|v| v.as_str()) == Some("secondary");
            let modifiers = if flag("shift") { egui::Modifiers::SHIFT } else { egui::Modifiers::NONE };
            h.hover_at(p);
            h.run_steps(1);
            if flag("double") {
                // A pause first, as a person makes: an earlier click within egui's triple-click window (0.6 s)
                // would turn this double-click into a triple-click.
                h.run_steps(24);
            }
            if secondary || flag("shift") || flag("double") {
                // Shift is held down for the whole click, as on a real keyboard.
                h.event(egui::Event::ModifiersChanged(modifiers));
                h.run_steps(1);
                let button = if secondary { egui::PointerButton::Secondary } else { egui::PointerButton::Primary };
                let clicks = if flag("double") { 2 } else { 1 };
                for _ in 0..clicks {
                    for pressed in [true, false] {
                        h.event(egui::Event::PointerButton { pos: p, button, pressed, modifiers });
                        h.run_steps(1);
                    }
                }
                h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
            } else {
                h.drag_at(p);
                h.run_steps(1);
                h.drop_at(p);
            }
            settle(h);
        }
        "expect_ui" => {
            let label = args.get("label").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("expect_ui needs label"))?;
            let want = args.get("exists").and_then(|v| v.as_bool()).unwrap_or(true);
            let got = h.query_by_label(label).is_some();
            if got != want {
                bail!("UI element {label:?} exists = {got}, expected {want}");
            }
        }
        "focus" => {
            let label = args.get("label").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("focus needs label"))?;
            find(h, label)?.focus();
            settle(h);
        }
        "scroll" => {
            // Mouse-wheel scroll over the canvas centre, in screen points (positive = content moves up/left).
            let dx = args.get("dx").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let dy = args.get("dy").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let c = h.state().page_to_screen(0.0, 0.0);
            h.hover_at(c + egui::Vec2::new(40.0, 40.0));
            h.run_steps(1);
            h.event(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::Vec2::new(-dx, -dy),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            });
            settle(h);
        }
        "run" => settle(h),
        "agent" => {
            // The app itself is the MCP host, as with `newpub --agent`.
            let v = runner::agent_step(h.state_mut(), &args)?;
            if let Some(Value::String(var)) = m.get("as").or_else(|| args.get("as")) {
                ctx.vars.insert(var.clone(), v);
            }
            settle(h);
        }
        "expect" if args.get("query").and_then(|q| q.get("q")).and_then(|q| q.as_str()) == Some("view") => {
            // App view state is answered by the app, not the engine.
            let spec = args.as_object().ok_or_else(|| anyhow!("expect needs a map"))?;
            let mut actual = h.state().view_state();
            if let Some(p) = spec.get("path").and_then(|p| p.as_str()) {
                actual = runner::pointer(&actual, p)?.clone();
            }
            runner::check_matchers(&actual, spec)?;
            if let Some(Value::String(var)) = spec.get("as") {
                ctx.vars.insert(var.clone(), actual);
            }
        }
        "expect" | "let" | "dump" | "expect_pdf" | "expect_png" | "expect_image" | "expect_html"
        | "expect_roundtrip" | "expect_files" | "expect_zip" => {
            let s: &mut Session = &mut h.state_mut().session;
            runner::run_step(s, ctx, step)?;
            if name == "expect_roundtrip" {
                // The round trip saves through the session, which gives the publication a name: the header's
                // title grows and its buttons move, so draw a frame before the next click reads their positions.
                settle(h);
            }
        }
        other => bail!("{other:?} is not a UI journey step (UI journeys drive the app only through its UI)"),
    }
    Ok(())
}

/// A fresh app as the journey options describe it. `recovery: true` keeps AutoRecover copies in
/// `<out>/recovery`, written after every change.
fn start_app(script: &Value, ctx: &Ctx) -> Harness<'static, NewpubApp> {
    let mut session = Session::bundled();
    session.base_dir = ctx.out.clone();
    let mut app = NewpubApp::new(session);
    app.startup_picker = script.get("startup_template_picker").and_then(|v| v.as_bool()).unwrap_or(false);
    // Journeys spool print jobs to files; the CI conformance job sets NEWPUB_REAL_PRINT=1 to print through the OS
    // (a CUPS virtual PDF printer) instead.
    if std::env::var_os("NEWPUB_REAL_PRINT").is_none() {
        app.print_spool = Some(ctx.out.join("print-spool"));
    }
    // `user_templates: true` keeps "My templates" in `<out>/templates`.
    if script.get("user_templates").and_then(|v| v.as_bool()).unwrap_or(false) {
        app = app.with_user_templates(ctx.out.join("templates"));
    }
    if script.get("recovery").and_then(|v| v.as_bool()).unwrap_or(false) {
        app = app.with_recovery(ctx.out.join("recovery"), 1);
    }
    // Frames 1/30 s apart, like a running app: double-clicks (two frames apart) fall inside egui's 0.3 s window.
    let mut h = Harness::builder()
        .with_size(egui::Vec2::new(1280.0, 860.0))
        .with_step_dt(1.0 / 30.0)
        .with_max_steps(64)
        .build_ui_state(|ui, app: &mut NewpubApp| app.ui(ui), app);
    settle(&mut h);
    h
}

/// Runs a UI journey. Errors carry the 1-based failing step.
pub fn run_ui_journey(script: &Value, steps: &[Value], ctx: &mut Ctx) -> Result<(), (usize, anyhow::Error)> {
    let mut h = start_app(script, ctx);
    for (i, step) in steps.iter().enumerate() {
        if step.get("restart_app").is_some() {
            // The app goes away without a clean exit (a crash) and starts again.
            drop(h);
            h = start_app(script, ctx);
            continue;
        }
        ui_step(&mut h, ctx, step).map_err(|e| (i + 1, e))?;
    }
    // Screenshots of the final document for the gallery.
    let shots = ctx.out.join("screens");
    let _ = std::fs::create_dir_all(&shots);
    let s = &mut h.state_mut().session;
    for p in 0..s.doc().pages.len().min(4) {
        if let Ok(png) = s.page_png(p, 48.0) {
            let _ = std::fs::write(shots.join(format!("page-{}.png", p + 1)), png);
        }
    }
    Ok(())
}
