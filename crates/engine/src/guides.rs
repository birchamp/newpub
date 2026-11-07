//! Guides, snapping, units, numeric geometry (GD-01..GD-05).

use crate::action::Units;
use crate::{EngineError, Outcome, Query, Session, SessionAction};
use newpub_core::units::parse_length;
use newpub_core::{Command, CoreError, Id, ObjectPatch, Orientation, Rect};
use serde_json::{Value, json};

/// Snap distance in points.
const SNAP_TOLERANCE: f64 = 6.0;
const DEDUP_EPS: f64 = 1e-6;

/// Session state owned by this module.
#[derive(Default)]
pub struct State {
    /// Snapping is on unless this is set.
    pub snapping_off: bool,
    pub units: Units,
}

impl Session {
    pub(crate) fn guides_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        match a {
            SessionAction::SetSnapping { enabled } => {
                self.guides.snapping_off = !*enabled;
                Ok(Outcome::default())
            }
            SessionAction::SetUnits { units } => {
                self.guides.units = *units;
                Ok(Outcome::default())
            }
            SessionAction::SetGeometry { id, x, y, w, h, rotation } => {
                let o = self.doc.object(*id)?;
                let mut rect = o.rect;
                let mut patch = ObjectPatch::default();
                let parse = |name: &str, s: &str| {
                    parse_length_text(s).ok_or_else(|| EngineError::Other(format!("cannot read {name} from {s:?}")))
                };
                if let Some(s) = x {
                    rect.x = parse("x", s)?;
                }
                if let Some(s) = y {
                    rect.y = parse("y", s)?;
                }
                if let Some(s) = w {
                    rect.w = parse("width", s)?;
                }
                if let Some(s) = h {
                    rect.h = parse("height", s)?;
                }
                if x.is_some() || y.is_some() || w.is_some() || h.is_some() {
                    patch.rect = Some(rect);
                }
                if let Some(s) = rotation {
                    patch.rotation = Some(
                        parse_angle(s).ok_or_else(|| EngineError::Other(format!("cannot read rotation from {s:?}")))?,
                    );
                }
                let cmd = Command::SetObject { id: *id, patch };
                Ok(Outcome { created: self.apply_cmd(&cmd)?.created })
            }
            other => Err(EngineError::Other(format!("{other:?} is not a guides action"))),
        }
    }

    pub(crate) fn guides_query(&mut self, q: &Query) -> Result<Value, EngineError> {
        match q {
            Query::Guides { page } => {
                let (v, h) = self.guide_lines(*page)?;
                Ok(json!({"vertical": v, "horizontal": h}))
            }
            Query::Snap { page, rect, ignore, tolerance } => {
                self.snap(*page, *rect, ignore, tolerance.unwrap_or(SNAP_TOLERANCE))
            }
            Query::ObjectGeometry { id } => {
                let o = self.doc.object(*id)?;
                let u = self.guides.units;
                Ok(json!({
                    "x": format_length(o.rect.x, u),
                    "y": format_length(o.rect.y, u),
                    "w": format_length(o.rect.w, u),
                    "h": format_length(o.rect.h, u),
                    "rotation": format!("{}°", trim_num(o.rotation)),
                }))
            }
            other => Err(EngineError::Other(format!("{other:?} is not a guides query"))),
        }
    }

    /// Sorted, de-duplicated (vertical, horizontal) guide positions of a page.
    fn guide_lines(&self, page: usize) -> Result<(Vec<f64>, Vec<f64>), EngineError> {
        let doc = &self.doc;
        let p = doc.pages.get(page).ok_or(CoreError::NoSuchPage(page))?;
        let (top, bottom, left, right) = doc.page_margins(page);
        let (pw, ph) = (doc.setup.width.0, doc.setup.height.0);
        let (x0, x1, y0, y1) = (left, pw - right, top, ph - bottom);
        let mut v = vec![x0, x1];
        let mut h = vec![y0, y1];
        if let Some(g) = &doc.guides.grid {
            grid_edges(x0, x1, g.columns, g.gutter.0, &mut v);
            grid_edges(y0, y1, g.rows, g.gutter.0, &mut h);
        }
        let master = doc.master_for_page(page).map(|m| m.id);
        for g in &doc.guides.ruler {
            let on_page = g.page == Some(p.id) || (g.master.is_some() && g.master == master);
            if on_page {
                match g.orientation {
                    Orientation::Vertical => v.push(g.pos.0),
                    Orientation::Horizontal => h.push(g.pos.0),
                }
            }
        }
        Ok((dedup(v), dedup(h)))
    }

    fn snap(&self, page: usize, rect: Rect, ignore: &[Id], tolerance: f64) -> Result<Value, EngineError> {
        let p = self.doc.pages.get(page).ok_or(CoreError::NoSuchPage(page))?;
        if self.guides.snapping_off {
            return Ok(json!({"rect": rect, "lines": []}));
        }
        let (mut vt, mut ht) = self.guide_lines(page)?;
        let (pw, ph) = (self.doc.setup.width.0, self.doc.setup.height.0);
        vt.extend([0.0, pw / 2.0, pw]);
        ht.extend([0.0, ph / 2.0, ph]);
        for id in &p.objects {
            if ignore.contains(id) {
                continue;
            }
            // Nothing snaps to an object that is not shown.
            if let Ok(o) = self.doc.object(*id)
                && !o.hidden
            {
                let r = o.rect;
                vt.extend([r.x, r.x + r.w / 2.0, r.x + r.w]);
                ht.extend([r.y, r.y + r.h / 2.0, r.y + r.h]);
            }
        }
        let sx = best_snap(&[rect.x, rect.x + rect.w / 2.0, rect.x + rect.w], &vt, tolerance);
        let sy = best_snap(&[rect.y, rect.y + rect.h / 2.0, rect.y + rect.h], &ht, tolerance);
        let mut out = rect;
        let mut lines = Vec::new();
        if let Some((delta, target)) = sx {
            out.x += delta;
            lines.push(json!({"orientation": "vertical", "pos": target}));
        }
        if let Some((delta, target)) = sy {
            out.y += delta;
            lines.push(json!({"orientation": "horizontal", "pos": target}));
        }
        Ok(json!({"rect": out, "lines": lines}))
    }
}

/// Edges of `n` equal columns separated by `gutter` between `a` and `b`.
fn grid_edges(a: f64, b: f64, n: u32, gutter: f64, out: &mut Vec<f64>) {
    if n == 0 || b <= a {
        return;
    }
    let n_f = f64::from(n);
    let size = (b - a - gutter * (n_f - 1.0)) / n_f;
    if size <= 0.0 {
        return;
    }
    for i in 0..n {
        let start = a + f64::from(i) * (size + gutter);
        out.push(start);
        out.push(start + size);
    }
}

fn dedup(mut v: Vec<f64>) -> Vec<f64> {
    v.retain(|x| x.is_finite());
    v.sort_by(f64::total_cmp);
    v.dedup_by(|a, b| (*a - *b).abs() < DEDUP_EPS);
    v
}

/// The closest (shift, target) over all candidate edges within `tolerance`.
fn best_snap(edges: &[f64], targets: &[f64], tolerance: f64) -> Option<(f64, f64)> {
    let mut best: Option<(f64, f64)> = None;
    for e in edges {
        for t in targets {
            let d = t - e;
            if d.abs() <= tolerance && best.is_none_or(|(bd, _)| d.abs() < bd.abs()) {
                best = Some((d, *t));
            }
        }
    }
    best
}

/// Parses "2in", "50 mm", "12pt", "6p", "11p9.73" into points.
fn parse_length_text(s: &str) -> Option<f64> {
    let s = s.trim();
    let v = if let Some((a, b)) = s
        .split_once('p')
        .filter(|(a, b)| !a.trim().is_empty() && !b.starts_with(['t', 'x', 'i']) && a.trim().parse::<f64>().is_ok())
    {
        // Picas and points: "6p" or "11p9.73".
        let picas = a.trim().parse::<f64>().ok()?;
        let pts = if b.trim().is_empty() { 0.0 } else { b.trim().parse::<f64>().ok()? };
        picas * 12.0 + if picas < 0.0 { -pts } else { pts }
    } else {
        parse_length(s)?
    };
    v.is_finite().then_some(v)
}

fn parse_angle(s: &str) -> Option<f64> {
    let s = s.trim();
    let s = s.strip_suffix('°').or_else(|| s.strip_suffix("deg")).unwrap_or(s);
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Number with up to two decimals and trailing zeros trimmed.
fn trim_num(v: f64) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}

fn format_length(pt: f64, units: Units) -> String {
    match units {
        Units::In => format!("{:.2} in", pt / 72.0),
        Units::Mm => format!("{} mm", trim_num(pt * 25.4 / 72.0)),
        Units::Cm => format!("{} cm", trim_num(pt * 2.54 / 72.0)),
        Units::Pt => format!("{} pt", trim_num(pt)),
        Units::Pi => {
            let sign = if pt < 0.0 { "-" } else { "" };
            let a = pt.abs();
            let mut picas = (a / 12.0).floor();
            let mut rem = (a - picas * 12.0 * 1.0).max(0.0);
            rem = (rem * 100.0).round() / 100.0;
            if rem >= 12.0 {
                picas += 1.0;
                rem = 0.0;
            }
            format!("{sign}{}p{}", picas as u64, trim_num(rem))
        }
    }
}
