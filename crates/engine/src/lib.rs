//! newpub-engine: the session that the app and the journey runner both drive.
//! Every user-visible operation goes through [`Session::run`]; every observation through
//! [`Session::query`] or exported files.

pub mod action;
mod autosave;
mod bizinfo;
mod blocks;
mod exportx;
mod findfmt;
mod fixups;
mod guides;
mod html;
mod imgformats;
mod importer;
mod layersq;
mod merge;
mod packgo;
mod pdfq;
mod pictures;
mod products;
mod pubimport;
mod spell;
mod templates;

pub use action::{Action, Query, SessionAction};
pub use newpub_core as core;
pub use newpub_io_pdf::{Imposition, PdfOptions};
pub use newpub_layout as layout;
pub use newpub_render as render;

use newpub_core::*;
use newpub_layout::{DocLayout, FontStore};
use newpub_render::Rasterizer;
use serde::Serialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Native(#[from] newpub_io_native::NativeError),
    #[error(transparent)]
    Pdf(#[from] newpub_io_pdf::PdfError),
    #[error("image: {0}")]
    Image(String),
    #[error("nothing to {0}")]
    Nothing(&'static str),
    #[error("{0}")]
    Other(String),
}

/// Reads a PNG or JPEG file into a new asset of `d`.
pub(crate) fn load_picture(d: &mut Document, p: &Path) -> Result<Id, EngineError> {
    let bytes = std::fs::read(p)?;
    let pic = imgformats::decode_picture(&bytes)?;
    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    Ok(d.add_asset(&name, pic.mime, Arc::from(pic.bytes), pic.px_w, pic.px_h))
}

/// Result of running an action.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Outcome {
    /// Ids created, primary first.
    pub created: Vec<Id>,
}

pub struct Session {
    doc: Document,
    history: History<Document>,
    fonts: Arc<FontStore>,
    layout: Option<Arc<DocLayout>>,
    /// Previous story flows, reused by the next layout (incremental layout, PF-01).
    layout_memo: newpub_layout::LayoutMemo,
    raster: Rasterizer,
    /// Snapshot taken at `begin_group`, plus nesting depth.
    group: Option<(Document, usize)>,
    /// Where typing coalesces: (story, next insertion index).
    typing: Option<(Id, usize)>,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    /// Directory relative paths are resolved against.
    pub base_dir: PathBuf,
    /// Incremented on every document change (for view caches).
    revision: u64,
    pub(crate) guides: guides::State,
    #[allow(dead_code)] // used once SP-01 lands
    pub(crate) spell: spell::State,
    #[allow(dead_code)] // used once PI-01 lands
    pub(crate) pub_import: pubimport::State,
    #[allow(dead_code)] // used once FI-03 lands
    pub(crate) autosave: autosave::State,
    /// Mail-merge preview record (index into the filtered, sorted records).
    pub(crate) merge_preview: Option<usize>,
    /// The document as displayed when it differs from `doc` (merge preview); computed with the layout.
    pub(crate) view: Option<Arc<Document>>,
    /// Folder of the user building-block library; None = `<base_dir>/library`.
    pub library_dir: Option<PathBuf>,
    /// Objects copied with CopyObjects/CutObjects: the fragment, its page, and how often it was pasted.
    pub(crate) clipboard: Option<(newpub_core::fragment::Fragment, usize, u32)>,
}

impl Session {
    pub fn new(fonts: Arc<FontStore>) -> Session {
        Session {
            doc: Document::default(),
            history: History::default(),
            fonts,
            layout: None,
            layout_memo: Default::default(),
            raster: Rasterizer::new(),
            group: None,
            typing: None,
            path: None,
            dirty: false,
            base_dir: PathBuf::from("."),
            revision: 0,
            guides: Default::default(),
            spell: Default::default(),
            pub_import: Default::default(),
            autosave: Default::default(),
            merge_preview: None,
            view: None,
            library_dir: None,
            clipboard: None,
        }
    }

    /// A session that only uses bundled fonts (deterministic; used by journeys).
    pub fn bundled() -> Session {
        Session::new(Arc::new(FontStore::bundled()))
    }

    /// Whether canvas snapping ("Snap to Guides") is on. The app reads it each frame, since an agent can set it.
    pub fn snapping(&self) -> bool {
        !self.guides.snapping_off
    }

    pub fn doc(&self) -> &Document {
        &self.doc
    }

    pub fn fonts(&self) -> &FontStore {
        &self.fonts
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    fn resolve(&self, p: &str) -> PathBuf {
        let p = Path::new(p);
        if p.is_absolute() { p.to_path_buf() } else { self.base_dir.join(p) }
    }

    /// Current layout (cached until the document changes).
    pub fn layout(&mut self) -> Arc<DocLayout> {
        if self.layout.is_none() {
            let mut view = self.preview_doc();
            // Scheme colours on objects become concrete colours for display and export (text resolves itself).
            if view.as_ref().unwrap_or(&self.doc).uses_object_scheme_colors() {
                let mut d = view.unwrap_or_else(|| self.doc.clone());
                d.resolve_object_colors();
                view = Some(d);
            }
            self.view = view.map(Arc::new);
            if self.merge_preview.is_some() {
                // Preview pictures get fresh asset ids each time.
                self.raster.clear_images();
            }
            let doc = self.view.as_deref().unwrap_or(&self.doc);
            self.layout = Some(Arc::new(newpub_layout::layout_document_with(doc, &self.fonts, &mut self.layout_memo)));
        }
        self.layout.clone().expect("layout computed")
    }

    fn changed(&mut self) {
        self.layout = None;
        self.dirty = true;
        self.revision += 1;
        self.autosave_tick();
    }

    /// Changes whenever the document changes.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Replaces the document wholesale as one undoable step.
    fn commit(&mut self, mut new: Document, coalesce: Option<String>) {
        fixups::run(&mut new, &self.fonts);
        let old = std::mem::replace(&mut self.doc, new);
        if self.group.is_none() {
            self.history.record(old, coalesce);
        }
        self.changed();
    }

    fn apply_cmd(&mut self, cmd: &Command) -> Result<Applied, EngineError> {
        let mut d = self.doc.clone();
        let a = d.apply(cmd)?;
        self.commit(d, None);
        Ok(a)
    }

    /// Runs one action.
    pub fn run(&mut self, action: &Action) -> Result<Outcome, EngineError> {
        if !matches!(action, Action::Session(SessionAction::TypeText { .. })) {
            self.typing = None;
            self.history.seal();
        }
        match action {
            Action::Doc(cmd) => Ok(Outcome { created: self.apply_cmd(cmd)?.created }),
            Action::Session(s) => self.run_session(s),
        }
    }

    fn run_session(&mut self, s: &SessionAction) -> Result<Outcome, EngineError> {
        use SessionAction::*;
        match s {
            NewDocument { width, height, margins, facing, pages, bleed } => {
                let setup = PageSetup {
                    width: *width,
                    height: *height,
                    margins: margins.unwrap_or(Insets::uniform(36.0)),
                    facing: *facing,
                    bleed: bleed.unwrap_or(Length(0.0)),
                };
                if width.0 <= 0.0 || height.0 <= 0.0 {
                    return Err(CoreError::Invalid("page size must be positive".into()).into());
                }
                self.doc = Document::new(setup, *pages);
                self.history = History::default();
                self.group = None;
                self.path = None;
                self.raster.clear_images();
                self.changed();
                self.dirty = false;
                Ok(Outcome { created: self.doc.pages.iter().map(|p| p.id).collect() })
            }
            Undo => {
                if self.group.is_some() {
                    return Err(EngineError::Other("cannot undo inside an undo group".into()));
                }
                let cur = self.doc.clone();
                let prev = self.history.undo(cur).ok_or(EngineError::Nothing("undo"))?;
                self.doc = prev;
                self.raster.clear_images();
                self.changed();
                Ok(Outcome::default())
            }
            Redo => {
                let cur = self.doc.clone();
                let next = self.history.redo(cur).ok_or(EngineError::Nothing("redo"))?;
                self.doc = next;
                self.raster.clear_images();
                self.changed();
                Ok(Outcome::default())
            }
            BeginGroup => {
                match &mut self.group {
                    Some((_, depth)) => *depth += 1,
                    None => self.group = Some((self.doc.clone(), 1)),
                }
                Ok(Outcome::default())
            }
            EndGroup => {
                let Some((snap, depth)) = self.group.take() else {
                    return Err(EngineError::Other("end_group without begin_group".into()));
                };
                if depth > 1 {
                    self.group = Some((snap, depth - 1));
                } else if snap != self.doc {
                    self.history.record(snap, None);
                }
                Ok(Outcome::default())
            }
            TypeText { target, at, text } => {
                let sid = self.doc.story_of(*target)?;
                let len = self.doc.story(sid)?.len();
                let at = at.unwrap_or(len);
                let key = match self.typing {
                    Some((s, next)) if s == sid && next == at && !text.contains(PARA_SEP) => {
                        Some(format!("type:{}", sid.0))
                    }
                    _ => None,
                };
                let mut d = self.doc.clone();
                d.apply(&Command::InsertText { target: sid, at: Some(at), text: text.clone(), attrs: None })?;
                let coalesce = key.or_else(|| Some(format!("type:{}", sid.0)));
                if self.typing.map(|(s, n)| s != sid || n != at).unwrap_or(true) {
                    self.history.seal();
                }
                self.commit(d, coalesce);
                self.typing = Some((sid, at + text.chars().count()));
                if text.contains(PARA_SEP) {
                    // A new paragraph ends the coalesced typing run (Publisher-like word/para granularity).
                    self.history.seal();
                }
                Ok(Outcome::default())
            }
            InsertPicture { path, page, x, y, width, height, into, link } => {
                self.insert_picture(path, *page, *x, *y, *width, *height, *into, *link)
            }
            ImportText { target, path, at } => self.import_text(*target, path, *at),
            Autoflow { frame } => self.autoflow(*frame),
            Save { path } => {
                let p = self.resolve(path);
                newpub_io_native::save(&self.doc, &p)?;
                self.path = Some(p);
                self.dirty = false;
                self.autosave_clear();
                Ok(Outcome::default())
            }
            Open { path } => {
                let p = self.resolve(path);
                // A Microsoft Publisher file opens as a new, unsaved publication named after the file.
                if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pub")) {
                    let out = self.pub_action(&ImportPub { path: path.clone() })?;
                    if self.doc.meta.title.trim().is_empty()
                        && let Some(stem) = p.file_stem()
                    {
                        self.doc.meta.title = stem.to_string_lossy().into_owned();
                    }
                    return Ok(out);
                }
                let d = newpub_io_native::open(&p)?;
                self.doc = d;
                self.history = History::default();
                self.group = None;
                self.path = Some(p);
                self.raster.clear_images();
                self.changed();
                self.dirty = false;
                Ok(Outcome::default())
            }
            ExportPdf { path, options } => {
                let bytes = self.pdf_bytes(options)?;
                std::fs::write(self.resolve(path), bytes)?;
                Ok(Outcome::default())
            }
            ReplaceAll { find, replace, match_case, whole_word } => {
                let mut d = self.doc.clone();
                for (sid, story) in &self.doc.stories {
                    for (start, end) in find_matches(&story.text, find, *match_case, *whole_word).into_iter().rev() {
                        d.apply(&Command::ReplaceText { target: *sid, start, end, text: replace.clone() })?;
                    }
                }
                self.commit(d, None);
                Ok(Outcome::default())
            }
            ExportImage { path, page, dpi, format, quality } => {
                let bytes = self.page_image(*page, *dpi, *format, *quality)?;
                std::fs::write(self.resolve(path), bytes)?;
                Ok(Outcome::default())
            }
            SetSnapping { .. } | SetUnits { .. } | SetGeometry { .. } => self.guides_action(s),
            SaveTemplate { .. } | NewFromTemplate { .. } | NewFromBuiltin { .. } => self.templates_action(s),
            IgnoreWord { .. } => self.spell_action(s),
            RelinkPicture { .. } | EmbedPicture { .. } => self.pictures_action(s),
            ExportHtml { .. } => self.html_action(s),
            ImportPub { .. } => self.pub_action(s),
            SetAutosave { .. } | RecoverAutosave { .. } => self.autosave_action(s),
            AttachDataSource { .. }
            | SetMergePreview { .. }
            | SetMergeFilter { .. }
            | SetMergeSort { .. }
            | SetMergeOptions { .. }
            | MergeToPdf { .. }
            | MergeToPublication {} => self.merge_action(s),
            SaveBuildingBlock { .. } | InsertBuildingBlock { .. } => self.blocks_action(s),
            CopyObjects { .. } | CutObjects { .. } | PasteObjects { .. } => self.clipboard_action(s),
            SetCatalogArea { .. } | ClearCatalogArea {} => self.merge_action(s),
            SaveBusinessInfoSet {} | ApplyBusinessInfoSet { .. } => self.bizinfo_action(s),
            ReplaceAdvanced { .. } => self.findfmt_action(s),
            NewFromPublicationType { .. } => self.products_action(s),
            PackAndGo { .. } => self.packgo_action(s),
            ExportEpub { .. } | ExportXps { .. } => self.exportx_action(s),
            ExportPng { path, page, dpi } => {
                let png = self.page_png(*page, *dpi)?;
                std::fs::write(self.resolve(path), png)?;
                Ok(Outcome::default())
            }
        }
    }

    pub fn pdf_bytes(&mut self, options: &PdfOptions) -> Result<Vec<u8>, EngineError> {
        let layout = self.layout();
        let doc = self.view.as_deref().unwrap_or(&self.doc);
        Ok(newpub_io_pdf::export_pdf(doc, &layout, &self.fonts, options)?)
    }

    /// Renders a page to a pixmap at `dpi`.
    pub fn render_page(&mut self, page: usize, dpi: f64) -> Result<newpub_render::Pixmap, EngineError> {
        if page >= self.doc.pages.len() {
            return Err(CoreError::NoSuchPage(page).into());
        }
        let layout = self.layout();
        let doc = self.view.as_deref().unwrap_or(&self.doc);
        let disp = newpub_render::page_display_for(doc, &layout, &self.fonts, page, true);
        newpub_render::render_page(&mut self.raster, doc, &self.fonts, &disp, dpi)
            .ok_or_else(|| EngineError::Other("render failed".into()))
    }

    /// Renders a page as it would look after `cmd` (a gallery preview), without changing the publication, its
    /// history or its layout cache.
    pub fn render_page_preview(
        &mut self,
        page: usize,
        dpi: f64,
        cmd: &Command,
    ) -> Result<newpub_render::Pixmap, EngineError> {
        if page >= self.doc.pages.len() {
            return Err(CoreError::NoSuchPage(page).into());
        }
        let mut d = self.doc.clone();
        d.apply(cmd)?;
        if d.uses_object_scheme_colors() {
            d.resolve_object_colors();
        }
        let layout = newpub_layout::layout_document(&d, &self.fonts);
        let disp = newpub_render::page_display_for(&d, &layout, &self.fonts, page, true);
        newpub_render::render_page(&mut self.raster, &d, &self.fonts, &disp, dpi)
            .ok_or_else(|| EngineError::Other("render failed".into()))
    }

    pub fn page_png(&mut self, page: usize, dpi: f64) -> Result<Vec<u8>, EngineError> {
        let pm = self.render_page(page, dpi)?;
        pm.encode_png().map_err(|e| EngineError::Image(e.to_string()))
    }

    /// Encodes a page as PNG or JPEG at `dpi`. The rendered pixmap has an opaque white background,
    /// so dropping alpha after demultiplying yields the flattened RGB image for JPEG.
    pub fn page_image(
        &mut self,
        page: usize,
        dpi: f64,
        format: action::ImageFormat,
        quality: u8,
    ) -> Result<Vec<u8>, EngineError> {
        let pm = self.render_page(page, dpi)?;
        match format {
            action::ImageFormat::Png => pm.encode_png().map_err(|e| EngineError::Image(e.to_string())),
            action::ImageFormat::Jpeg => {
                let mut rgb = Vec::with_capacity(pm.pixels().len() * 3);
                for px in pm.pixels() {
                    let c = px.demultiply();
                    rgb.extend_from_slice(&[c.red(), c.green(), c.blue()]);
                }
                let mut out = Vec::new();
                let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality.clamp(1, 100));
                image::ImageEncoder::write_image(
                    encoder,
                    &rgb,
                    pm.width(),
                    pm.height(),
                    image::ExtendedColorType::Rgb8,
                )
                .map_err(|e| EngineError::Image(e.to_string()))?;
                Ok(out)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_picture(
        &mut self,
        path: &str,
        page: Option<usize>,
        x: Option<Length>,
        y: Option<Length>,
        width: Option<Length>,
        height: Option<Length>,
        into: Option<Id>,
        link: bool,
    ) -> Result<Outcome, EngineError> {
        let p = self.resolve(path);
        let mut d = self.doc.clone();
        let aid = load_picture(&mut d, &p)?;
        let (pw, ph) = d.assets.get(&aid).map(|a| (a.px_w, a.px_h)).unwrap_or((1, 1));
        if link && let Some(a) = d.assets.get_mut(&aid) {
            a.link = Some(p.to_string_lossy().to_string());
        }
        if let Some(frame) = into {
            d.apply(&Command::SetImage {
                id: frame,
                patch: ImagePatch { asset: Some(aid), fit: Some(Fit::Fill), ..Default::default() },
            })?;
            self.commit(d, None);
            return Ok(Outcome { created: vec![frame, aid] });
        }
        let page = page.unwrap_or(0);
        if page >= d.pages.len() {
            return Err(CoreError::NoSuchPage(page).into());
        }
        let aspect = pw.max(1) as f64 / ph.max(1) as f64;
        let (w, h) = match (width, height) {
            (Some(w), Some(h)) => (w.0, h.0),
            (Some(w), None) => (w.0, w.0 / aspect),
            (None, Some(h)) => (h.0 * aspect, h.0),
            (None, None) => {
                let (mut w, mut h) = (pw as f64 * 0.75, ph as f64 * 0.75);
                let (t, b, l, r) = d.page_margins(page);
                let (aw, ah) = (d.setup.width.0 - l - r, d.setup.height.0 - t - b);
                let k = (aw / w).min(ah / h).min(1.0);
                w *= k;
                h *= k;
                (w, h)
            }
        };
        let (t, _, l, _) = d.page_margins(page);
        let rx = x.map(|v| v.0).unwrap_or(l + ((d.setup.width.0 - l - d.page_margins(page).3) - w) / 2.0);
        let ry = y.map(|v| v.0).unwrap_or(t);
        let a = d.apply(&Command::AddImage {
            page: Some(page),
            master: None,
            rect: Rect::new(rx, ry, w, h),
            asset: Some(aid),
        })?;
        self.commit(d, None);
        let mut created = a.created;
        created.push(aid);
        Ok(Outcome { created })
    }

    fn autoflow(&mut self, frame: Id) -> Result<Outcome, EngineError> {
        let sid = self.doc.story_of(frame)?;
        let template = self.doc.object(frame)?.clone();
        let ObjectKind::Text(tf) = &template.kind else { return Err(CoreError::NotText(frame).into()) };
        let tf = tf.clone();
        let mut d = self.doc.clone();
        let mut created = vec![];
        let mut memo = newpub_layout::LayoutMemo::default();
        for _ in 0..2000 {
            let Some((sl, frames)) = newpub_layout::layout_one_with(&d, &self.fonts, sid, &mut memo) else { break };
            let Some(over) = sl.overflow_at else { break };
            // Add the pages the rest of the text needs at the fullest frame's rate, less a margin, so the last
            // passes add one page at a time and no page is left empty.
            let rest = d.story(sid)?.len().saturating_sub(over);
            let per_frame = frames.iter().map(|f| f.char_range.len()).max().unwrap_or(0).max(1);
            let count = (rest * 9 / 10 / per_frame).clamp(1, 200);
            for _ in 0..count {
                let last = *d.story(sid)?.frames.last().ok_or(CoreError::NotText(frame))?;
                let last_page = d.page_of(last).unwrap_or(d.pages.len() - 1);
                let at = last_page + 1;
                d.apply(&Command::InsertPages { at: Some(at), count: 1, master: None })?;
                let r = d.apply(&Command::AddTextFrame {
                    page: Some(at),
                    master: None,
                    rect: template.rect,
                    columns: Some(tf.columns),
                    gutter: Some(tf.gutter),
                })?;
                let nf = r.created[0];
                d.apply(&Command::SetTextFrame {
                    id: nf,
                    patch: TextFramePatch { insets: Some(tf.insets), valign: Some(tf.valign), ..Default::default() },
                })?;
                d.apply(&Command::LinkFrames { from: last, to: nf })?;
                created.push(d.pages[at].id);
                created.push(nf);
            }
        }
        self.commit(d, None);
        Ok(Outcome { created })
    }

    /// Sorted, de-duplicated font families used by any text, resolved per paragraph through styles.
    fn fonts_in_use(&self) -> std::collections::BTreeSet<String> {
        let mut out = std::collections::BTreeSet::new();
        for st in self.doc.stories.values() {
            let ranges = st.para_ranges();
            for (run, attrs) in st.runs() {
                if run.is_empty() {
                    continue;
                }
                for (pi, pr) in ranges.iter().enumerate() {
                    let overlaps = run.start.max(pr.start) < run.end.min(pr.end);
                    let Some(para) = st.paras.get(pi).filter(|_| overlaps) else { continue };
                    out.insert(self.doc.resolve_char(para, attrs).font);
                }
            }
        }
        out
    }

    /// Answers a query.
    pub fn query(&mut self, q: &Query) -> Result<Value, EngineError> {
        use Query::*;
        let to = |v: &dyn erased::Ser| v.to_json();
        Ok(match q {
            Document => serde_json::to_value(&self.doc).map_err(|e| EngineError::Other(e.to_string()))?,
            PageCount => json!(self.doc.pages.len()),
            LayoutFingerprint => json!(self.layout().fingerprint()),
            Page { page } => {
                let p = self.doc.pages.get(*page).ok_or(CoreError::NoSuchPage(*page))?;
                to(p)
            }
            Object { id } => {
                let mut v = to(self.doc.object(*id)?);
                // Optional fields the file format omits are reported as null.
                if let Some(m) = v.as_object_mut() {
                    m.entry("layer").or_insert(Value::Null);
                    m.entry("hidden").or_insert(Value::Bool(false));
                    m.entry("overprint").or_insert(Value::Bool(false));
                }
                v
            }
            ObjectCount { page } => json!(self.doc.pages.get(*page).ok_or(CoreError::NoSuchPage(*page))?.objects.len()),
            PageObjects { page } => to(&self.doc.pages.get(*page).ok_or(CoreError::NoSuchPage(*page))?.objects),
            PageObjectKinds { page } => {
                fn walk(doc: &newpub_core::Document, ids: &[Id], out: &mut Vec<String>) {
                    for id in ids {
                        let Some(o) = doc.objects.get(id) else { continue };
                        let kind = serde_json::to_value(&o.kind).ok();
                        out.push(kind.and_then(|k| k["type"].as_str().map(String::from)).unwrap_or_default());
                        if let ObjectKind::Group { children } = &o.kind {
                            walk(doc, children, out);
                        }
                    }
                }
                let page = self.doc.pages.get(*page).ok_or(CoreError::NoSuchPage(*page))?;
                let mut out = vec![];
                walk(&self.doc, &page.objects, &mut out);
                json!(out)
            }
            StoryText { target } => {
                let sid = self.doc.story_of(*target)?;
                json!(self.doc.story(sid)?.text)
            }
            StoryLength { target } => {
                let sid = self.doc.story_of(*target)?;
                json!(self.doc.story(sid)?.len())
            }
            StoryFrames { target } => {
                let sid = self.doc.story_of(*target)?;
                to(&self.doc.story(sid)?.frames)
            }
            FrameText { frame } => {
                self.doc.object(*frame)?;
                let l = self.layout();
                json!(l.frames.get(frame).map(|f| f.text()).unwrap_or_default())
            }
            FrameLines { frame } => {
                self.doc.object(*frame)?;
                let l = self.layout();
                let lines: Vec<Value> = l
                    .frames
                    .get(frame)
                    .map(|f| {
                        f.lines
                            .iter()
                            .map(|ln| {
                                let one = newpub_layout::FrameLayout { lines: vec![ln.clone()], ..Default::default() };
                                let first_x = ln
                                    .runs
                                    .iter()
                                    .flat_map(|r| r.glyphs.first())
                                    .map(|g| g.x)
                                    .fold(f64::INFINITY, f64::min);
                                // Ink excludes whitespace glyphs (e.g. the trailing space of a justified line).
                                let last_x = ln
                                    .runs
                                    .iter()
                                    .flat_map(|r| r.glyphs.iter().map(move |g| (r, g)))
                                    .filter(|(r, g)| {
                                        !r.text
                                            .get(g.text_range.clone())
                                            .map(|t| !t.is_empty() && t.trim().is_empty())
                                            .unwrap_or(false)
                                    })
                                    .map(|(_, g)| g.x + g.advance)
                                    .fold(f64::NEG_INFINITY, f64::max);
                                json!({
                                    "text": one.text(),
                                    "x": ln.x, "width": ln.width, "top": ln.top, "height": ln.height,
                                    "baseline": ln.baseline, "column": ln.column, "hyphenated": ln.hyphenated,
                                    "start": ln.char_range.start, "end": ln.char_range.end,
                                    "ink_left": if first_x.is_finite() { json!(first_x) } else { Value::Null },
                                    "ink_right": if last_x.is_finite() { json!(last_x) } else { Value::Null },
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                Value::Array(lines)
            }
            Overflow { target } => {
                let sid = self.doc.story_of(*target)?;
                let l = self.layout();
                json!(l.stories.get(&sid).and_then(|s| s.overflow_at).is_some())
            }
            OverflowAt { target } => {
                let sid = self.doc.story_of(*target)?;
                let l = self.layout();
                json!(l.stories.get(&sid).and_then(|s| s.overflow_at))
            }
            CharPage { target, at } | CharFrame { target, at } => {
                let sid = self.doc.story_of(*target)?;
                let l = self.layout();
                let frame = self.doc.story(sid)?.frames.iter().copied().find(|f| {
                    l.frames
                        .get(f)
                        .map(|fl| {
                            fl.lines.iter().any(|ln| {
                                ln.char_range.contains(at) || (ln.char_range.end == *at && ln.char_range.start < *at)
                            })
                        })
                        .unwrap_or(false)
                });
                match (q, frame) {
                    (CharFrame { .. }, Some(f)) => json!(f),
                    (CharPage { .. }, Some(f)) => json!(self.doc.page_of(f)),
                    // Text in a table cell is shown on the page of its table.
                    (CharPage { .. }, None) => {
                        let shown = l.stories.get(&sid).is_some_and(|s| s.overflow_at.is_none_or(|o| o > *at));
                        let table = self.doc.objects.values().find(|o| {
                            matches!(&o.kind, ObjectKind::Table(t) if t.cells.iter().any(|c| c.story == sid && !c.covered))
                        });
                        match table {
                            Some(t) if shown && *at < self.doc.story(sid)?.len() => json!(self.doc.page_of(t.id)),
                            _ => Value::Null,
                        }
                    }
                    _ => Value::Null,
                }
            }
            CharAttrs { target, at } => {
                let sid = self.doc.story_of(*target)?;
                let st = self.doc.story(sid)?;
                let pi = st.para_index_at(*at);
                to(&self.doc.resolve_char(&st.paras[pi], &st.span_attrs_at(*at)))
            }
            ParaAttrs { target, at } => {
                let sid = self.doc.story_of(*target)?;
                let st = self.doc.story(sid)?;
                let pi = st.para_index_at(*at);
                let mut v = to(&self.doc.resolve_para(&st.paras[pi]));
                let style = st.paras[pi].style.and_then(|s| self.doc.styles.para.get(&s)).map(|s| s.name.clone());
                v["style"] = json!(style);
                v
            }
            ObjectPage { id } => {
                self.doc.object(*id)?;
                json!(self.doc.page_of(*id))
            }
            Styles => json!({
                "para": self.doc.styles.para.values().map(|s| s.name.clone()).collect::<Vec<_>>(),
                "chars": self.doc.styles.chars.values().map(|s| s.name.clone()).collect::<Vec<_>>(),
            }),
            Masters => Value::Array(
                self.doc.masters.iter().map(|m| json!({"id": m.id, "name": m.name, "objects": m.objects})).collect(),
            ),
            History => json!({"undo": self.history.undo_len(), "redo": self.history.redo_len()}),
            CharBox { target, at } => {
                let sid = self.doc.story_of(*target)?;
                let l = self.layout();
                for f in self.doc.story(sid)?.frames.clone() {
                    let Some(fl) = l.frames.get(&f) else { continue };
                    for (li, ln) in fl.lines.iter().enumerate() {
                        for r in &ln.runs {
                            if let Some(g) = r
                                .glyphs
                                .iter()
                                .find(|g| g.char_index == *at && !g.generated && !g.text_range.is_empty())
                            {
                                // x and baseline are relative to the frame; page_x and page_baseline to the page.
                                let o = self.doc.object(f)?.rect;
                                return Ok(json!({
                                    "frame": f, "page": self.doc.page_of(f), "x": g.x, "baseline": g.y,
                                    "page_x": o.x + g.x, "page_baseline": o.y + g.y, "width": g.advance, "line": li,
                                }));
                            }
                        }
                    }
                }
                Value::Null
            }
            Decorations { frame } => {
                self.doc.object(*frame)?;
                let l = self.layout();
                let v: Vec<Value> = l
                    .frames
                    .get(frame)
                    .map(|f| {
                        f.decorations
                            .iter()
                            .map(|d| {
                                let kind = match d.kind {
                                    newpub_layout::DecorationKind::Underline => "underline",
                                    newpub_layout::DecorationKind::Strike => "strike",
                                };
                                json!({"kind": kind, "x0": d.x0, "x1": d.x1, "y": d.y, "thickness": d.thickness})
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                Value::Array(v)
            }
            PageMargins { page } => {
                if *page >= self.doc.pages.len() {
                    return Err(CoreError::NoSuchPage(*page).into());
                }
                let (top, bottom, left, right) = self.doc.page_margins(*page);
                json!({"top": top, "bottom": bottom, "left": left, "right": right})
            }
            Spreads => {
                let n = self.doc.pages.len();
                let mut groups: Vec<Vec<usize>> = Vec::new();
                if self.doc.setup.facing {
                    // Page 0 is a right-hand page on its own; following pages pair up as left/right.
                    let mut i = 0;
                    while i < n {
                        if i == 0 || i + 1 >= n {
                            groups.push(vec![i]);
                            i += 1;
                        } else {
                            groups.push(vec![i, i + 1]);
                            i += 2;
                        }
                    }
                } else {
                    groups.extend((0..n).map(|i| vec![i]));
                }
                serde_json::to_value(groups).map_err(|e| EngineError::Other(e.to_string()))?
            }
            FontsUsed => json!(self.fonts_in_use().into_iter().collect::<Vec<_>>()),
            MissingFonts => {
                json!(self.fonts_in_use().into_iter().filter(|f| !self.fonts.has_family(f)).collect::<Vec<_>>())
            }
            Assets => Value::Array(
                self.doc
                    .assets
                    .values()
                    .map(|a| {
                        json!({"id": a.id, "name": a.name, "mime": a.mime, "px_w": a.px_w, "px_h": a.px_h, "link": a.link})
                    })
                    .collect(),
            ),
            Find { text, match_case, whole_word } => {
                let mut out = Vec::new();
                for (sid, story) in &self.doc.stories {
                    for (start, end) in find_matches(&story.text, text, *match_case, *whole_word) {
                        out.push(json!({"story": sid, "start": start, "end": end}));
                    }
                }
                Value::Array(out)
            }
            AccessibilityCheck => self.accessibility_check(),
            Session => json!({
                "path": self.path.as_ref().map(|p| p.to_string_lossy().to_string()),
                "dirty": self.dirty,
                "units": self.guides.units,
                "snapping": self.snapping(),
            }),
            AllStoryText => json!(self.doc.stories.values().map(|s| s.text.clone()).collect::<Vec<_>>().join("\n")),
            Table { id } => {
                let ObjectKind::Table(t) = &self.doc.object(*id)?.kind else {
                    return Err(CoreError::WrongKind(*id, "table").into());
                };
                json!({
                    "rows": t.rows(), "cols": t.cols(),
                    "col_widths": t.col_widths, "row_heights": t.row_heights,
                    "header_rows": t.header_rows, "format": t.format,
                })
            }
            TableCell { table, row, col } => {
                let ObjectKind::Table(t) = &self.doc.object(*table)?.kind else {
                    return Err(CoreError::WrongKind(*table, "table").into());
                };
                let (r, c) = t.owner(*row, *col).ok_or_else(|| EngineError::Other(format!("no cell {row},{col}")))?;
                let cell = t.cell(r, c).ok_or_else(|| EngineError::Other(format!("no cell {row},{col}")))?;
                json!({"story": cell.story, "rowspan": cell.rowspan, "colspan": cell.colspan,
                       "covered": (r, c) != (*row, *col), "fill": cell.fill})
            }
            TableFormats => json!(newpub_core::table::FORMATS.iter().map(|f| f.0).collect::<Vec<_>>()),
            DataSource => self.merge_query(q)?,
            ColorSchemes => to(&newpub_core::schemes::color_schemes()),
            ColorScheme => to(&self.doc.color_scheme()),
            FontSchemes => to(&newpub_core::schemes::font_schemes()),
            ResolvedFill { id } => {
                let fill = match &self.doc.object(*id)?.kind {
                    ObjectKind::Shape(s) => s.fill.clone(),
                    ObjectKind::Text(t) => t.fill.clone(),
                    _ => return Err(CoreError::WrongKind(*id, "shape or text box").into()),
                };
                to(&fill.map(|c| self.doc.scheme_color(&c)))
            }
            BuildingBlocks | BuildingBlockLibrary => self.blocks_query(q)?,
            BusinessInfo | BusinessInfoSets => self.bizinfo_query(q)?,
            WordArtStyles => json!(newpub_core::wordart::styles().into_iter().map(|s| s.name).collect::<Vec<_>>()),
            PathNodes { id } => to(&self.doc.path_nodes(*id)?),
            FindAdvanced { .. } => self.findfmt_query(q)?,
            PublicationTypes | PublicationType { .. } => self.products_query(q)?,
            SeparationPlates => json!(newpub_io_pdf::separations::plates(&self.doc)),
            PageLabel { page } => {
                if *page >= self.doc.pages.len() {
                    return Err(CoreError::NoSuchPage(*page).into());
                }
                json!(self.doc.page_label(*page))
            }
            GlyphCount { frame } => {
                self.doc.object(*frame)?;
                let l = self.layout();
                let n: usize = l
                    .frames
                    .get(frame)
                    .map(|f| f.lines.iter().flat_map(|ln| ln.runs.iter()).flat_map(|r| r.glyphs.iter()).filter(|g| !g.generated).count())
                    .unwrap_or(0);
                json!(n)
            }
            MissingGlyphs { frame } => {
                self.doc.object(*frame)?;
                let l = self.layout();
                // Glyph 0 is the missing-glyph (.notdef) glyph; spaces and controls never count.
                let n: usize = l
                    .frames
                    .get(frame)
                    .map(|f| {
                        f.lines
                            .iter()
                            .flat_map(|ln| ln.runs.iter())
                            .flat_map(|r| r.glyphs.iter().map(move |g| (r, g)))
                            .filter(|(r, g)| {
                                g.id == 0 && r.text.get(g.text_range.clone()).is_some_and(|t| t.chars().any(|c| !c.is_whitespace() && !c.is_control()))
                            })
                            .count()
                    })
                    .unwrap_or(0);
                json!(n)
            }
            PageBaselines { frame } => {
                let o = self.doc.object(*frame)?.clone();
                let l = self.layout();
                let v: Vec<f64> = l
                    .frames
                    .get(frame)
                    .map(|f| f.lines.iter().filter(|ln| ln.para != usize::MAX).map(|ln| o.rect.y + ln.baseline).collect())
                    .unwrap_or_default();
                json!(v)
            }
            ShapeKinds => json!([
                "rect", "round_rect", "ellipse", "line", "triangle", "star", "polygon", "arrow", "callout", "path"
            ]),
            Guides { .. } | Snap { .. } | ObjectGeometry { .. } => self.guides_query(q)?,
            BuiltinTemplates => self.templates_query(q)?,
            Spelling | SpellingLanguages => self.spell_query(q)?,
            Layers => self.layers_query(q)?,
            MissingLinks | NpubEntries { .. } | OffPageObjects => self.pictures_query(q)?,
            ColorsUsed | NUpLayout { .. } | Hyperlinks => self.pdf_query(q)?,
            PubReport { .. } | ImportReport => self.pub_query(q)?,
            Autosaves { .. } => self.autosave_query(q)?,
            TextBounds { frame } => {
                let o = self.doc.object(*frame)?.clone();
                let l = self.layout();
                let Some(fl) = l.frames.get(frame) else { return Ok(Value::Null) };
                let mut b: Option<Rect> = None;
                for ln in &fl.lines {
                    for r in &ln.runs {
                        for g in &r.glyphs {
                            let gr = Rect::new(o.rect.x + g.x, o.rect.y + ln.top, g.advance.max(0.0), ln.height);
                            b = Some(b.map_or(gr, |x| x.union(&gr)));
                        }
                    }
                }
                b.map(|r| json!({"x": r.x, "y": r.y, "w": r.w, "h": r.h})).unwrap_or(Value::Null)
            }
        })
    }
}

mod erased {
    pub trait Ser {
        fn to_json(&self) -> serde_json::Value;
    }
    impl<T: serde::Serialize> Ser for T {
        fn to_json(&self) -> serde_json::Value {
            serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
        }
    }
}

/// Non-overlapping matches of `needle` in `hay` as char index ranges. Case-insensitive matching uses simple
/// per-char lowercase folding so that char indices stay aligned.
fn find_matches(hay: &str, needle: &str, match_case: bool, whole_word: bool) -> Vec<(usize, usize)> {
    let fold = |c: char| {
        if match_case {
            c
        } else {
            let mut l = c.to_lowercase();
            match (l.next(), l.next()) {
                (Some(x), None) => x,
                _ => c,
            }
        }
    };
    let h: Vec<char> = hay.chars().collect();
    let hf: Vec<char> = h.iter().map(|&c| fold(c)).collect();
    let n: Vec<char> = needle.chars().map(fold).collect();
    let mut out = Vec::new();
    if n.is_empty() || n.len() > h.len() {
        return out;
    }
    let mut i = 0;
    while i + n.len() <= h.len() {
        let end = i + n.len();
        let ok = hf[i..end] == n[..]
            && (!whole_word
                || ((i == 0 || !h[i - 1].is_alphanumeric()) && (end == h.len() || !h[end].is_alphanumeric())));
        if ok {
            out.push((i, end));
            i = end;
        } else {
            i += 1;
        }
    }
    out
}

/// WCAG relative luminance of an sRGB colour.
fn relative_luminance(c: &Color) -> f64 {
    let [r, g, b, _] = c.to_rgba8();
    let lin = |v: u8| {
        let v = f64::from(v) / 255.0;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

fn contrast_ratio(a: &Color, b: &Color) -> f64 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

impl Session {
    /// Page objects in z-order, descending into groups.
    fn flat_page_objects(&self, ids: &[Id], out: &mut Vec<Id>) {
        for id in ids {
            if let Some(o) = self.doc.objects.get(id) {
                out.push(*id);
                if let ObjectKind::Group { children } = &o.kind {
                    self.flat_page_objects(children, out);
                }
            }
        }
    }

    fn accessibility_check(&mut self) -> Value {
        let layout = self.layout();
        let mut out = vec![];
        let issue = |rule: &str, object: Id, page: usize, message: &str| json!({"rule": rule, "object": object, "page": page, "message": message});
        for pi in 0..self.doc.pages.len() {
            let page = &self.doc.pages[pi];
            let bg = page
                .background
                .clone()
                .or_else(|| self.doc.master_for_page(pi).and_then(|m| m.background.clone()))
                .unwrap_or(Color::WHITE);
            let mut ids = vec![];
            self.flat_page_objects(&page.objects, &mut ids);
            // Overflow is reported on the last frame of a story, once.
            for id in ids {
                let Some(o) = self.doc.objects.get(&id) else { continue };
                let (mut missing, mut low, mut small, mut over) = (None, false, false, false);
                match &o.kind {
                    ObjectKind::Image(_) | ObjectKind::Shape(_) | ObjectKind::WordArt(_) => {
                        let has_alt = o.alt_text.as_deref().is_some_and(|t| !t.trim().is_empty());
                        if !o.decorative && !has_alt {
                            let m = match o.kind {
                                ObjectKind::Image(_) => "Picture has no alt text",
                                ObjectKind::WordArt(_) => "WordArt has no alt text",
                                _ => "Shape has no alt text",
                            };
                            missing = Some(m);
                        }
                    }
                    ObjectKind::Text(t) => {
                        if let Some(st) = self.doc.stories.get(&t.story) {
                            let text: Vec<char> = st.text.chars().collect();
                            for (range, _) in st.runs() {
                                if !text[range.clone()].iter().any(|c| !c.is_whitespace() && *c != PARA_SEP) {
                                    continue;
                                }
                                let at = range.start;
                                let pi_ = st.para_index_at(at);
                                let rc = self.doc.resolve_char(&st.paras[pi_], &st.span_attrs_at(at));
                                low |= contrast_ratio(&rc.color, &bg) < 4.5;
                                small |= rc.size < 8.0;
                            }
                            over = st.frames.last() == Some(&id)
                                && layout.stories.get(&t.story).is_some_and(|s| s.overflow_at.is_some());
                        }
                    }
                    ObjectKind::Group { .. } | ObjectKind::Table(_) => {}
                }
                if let Some(m) = missing {
                    out.push(issue("missing_alt_text", id, pi, m));
                }
                if low {
                    out.push(issue("low_contrast", id, pi, "Text colour has low contrast with the page"));
                }
                if small {
                    out.push(issue("small_text", id, pi, "Text is smaller than 8 pt"));
                }
                if over {
                    out.push(issue("overflow", id, pi, "Text does not fit in its text box"));
                }
            }
        }
        Value::Array(out)
    }
}
