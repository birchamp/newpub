//! Session-level actions and read-only queries.
//! Interface rule (ARCHITECTURE.md §4): lead-owned; record changes in §11.

use newpub_core::{CharAttrs, Command, Id, Insets, Length, Rect};
use newpub_io_pdf::PdfOptions;
use serde::{Deserialize, Serialize};

/// Everything a user (or a journey) can do. Document mutations are [`Command`]s; the rest
/// are session actions. Both share the `cmd` tag namespace.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum Action {
    Session(SessionAction),
    Doc(Command),
}

impl<'de> Deserialize<'de> for Action {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let v = serde_json::Value::deserialize(d)?;
        let name = v
            .get("cmd")
            .and_then(|c| c.as_str())
            .map(str::to_string)
            .ok_or_else(|| D::Error::custom("action needs a \"cmd\""))?;
        if SessionAction::NAMES.contains(&name.as_str()) {
            serde_json::from_value(v).map(Action::Session).map_err(|e| D::Error::custom(format!("{name}: {e}")))
        } else {
            serde_json::from_value(v).map(Action::Doc).map_err(|e| D::Error::custom(format!("{name}: {e}")))
        }
    }
}

impl From<SessionAction> for Action {
    fn from(s: SessionAction) -> Self {
        Action::Session(s)
    }
}

impl From<Command> for Action {
    fn from(c: Command) -> Self {
        Action::Doc(c)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case", deny_unknown_fields)]
#[allow(clippy::large_enum_variant)]
pub enum SessionAction {
    /// Start a new publication (clears history).
    NewDocument {
        width: Length,
        height: Length,
        #[serde(default)]
        margins: Option<Insets>,
        #[serde(default)]
        facing: bool,
        #[serde(default = "one")]
        pages: usize,
        #[serde(default)]
        bleed: Option<Length>,
    },
    Undo,
    Redo,
    /// Start an undo group: everything until `end_group` undoes as one step.
    BeginGroup,
    EndGroup,
    /// Type text as a user would: consecutive typing at the insertion point coalesces into one undo step.
    TypeText {
        target: Id,
        #[serde(default)]
        at: Option<usize>,
        text: String,
    },
    /// Insert a picture file. Without a size it is placed at its natural size (96 dpi), scaled to fit
    /// the page margins. With only `width` (or `height`) the other side keeps the aspect ratio.
    InsertPicture {
        path: String,
        #[serde(default)]
        page: Option<usize>,
        #[serde(default)]
        x: Option<Length>,
        #[serde(default)]
        y: Option<Length>,
        #[serde(default)]
        width: Option<Length>,
        #[serde(default)]
        height: Option<Length>,
        /// Fill an existing picture frame (placeholder) instead of creating one.
        #[serde(default)]
        into: Option<Id>,
        /// Store a link to the file instead of embedding it.
        #[serde(default)]
        link: bool,
    },
    /// Insert a text file's contents into a story.
    ImportText {
        target: Id,
        path: String,
        #[serde(default)]
        at: Option<usize>,
    },
    /// Flow overflow text: add pages with linked frames (same position as `frame`) until the story fits.
    Autoflow {
        frame: Id,
    },
    Save {
        path: String,
    },
    Open {
        path: String,
    },
    ExportPdf {
        path: String,
        #[serde(default)]
        options: PdfOptions,
    },
    ExportPng {
        path: String,
        page: usize,
        #[serde(default = "dpi")]
        dpi: f64,
    },
    /// Export one page as an image file (PNG or JPEG) at `dpi`, white background, trim size.
    ExportImage {
        path: String,
        page: usize,
        #[serde(default = "dpi")]
        dpi: f64,
        #[serde(default)]
        format: ImageFormat,
        /// JPEG quality 1–100.
        #[serde(default = "quality")]
        quality: u8,
    },
    // ---- GUIDES task (engine/src/guides.rs) ----
    /// Turn snapping on or off (default on).
    SetSnapping {
        enabled: bool,
    },
    /// Display unit for geometry shown to the user.
    SetUnits {
        units: Units,
    },
    /// Set geometry from text in any unit ("2in", "50 mm", "6p", "12pt"); omitted fields keep their value.
    SetGeometry {
        id: Id,
        #[serde(default)]
        x: Option<String>,
        #[serde(default)]
        y: Option<String>,
        #[serde(default)]
        w: Option<String>,
        #[serde(default)]
        h: Option<String>,
        #[serde(default)]
        rotation: Option<String>,
    },
    // ---- TEMPLATES task (engine/src/templates.rs) ----
    /// Save the publication as a template file (a .newspub flagged as a template, with a name).
    SaveTemplate {
        path: String,
        name: String,
        #[serde(default)]
        keep_text: bool,
        #[serde(default)]
        keep_images: bool,
    },
    /// New untitled publication copied from a template file.
    NewFromTemplate {
        path: String,
    },
    /// New untitled publication from a built-in template (see query builtin_templates).
    NewFromBuiltin {
        id: String,
    },
    // ---- SPELL task (engine/src/spell.rs) ----
    /// Ignore a word for the rest of the session (not stored in the document).
    IgnoreWord {
        word: String,
    },
    // ---- PICTURES task (engine/src/pictures.rs) ----
    /// Point a (missing) linked picture asset at another file.
    RelinkPicture {
        asset: Id,
        path: String,
    },
    /// Store a linked picture's bytes in the publication.
    EmbedPicture {
        asset: Id,
    },
    // ---- HTML task (engine/src/html.rs) ----
    /// Export the publication as HTML into directory `path` (index.html, page-2.html, …, assets).
    ExportHtml {
        path: String,
    },
    // ---- PUB task (engine/src/pubimport.rs) ----
    /// Import a Microsoft Publisher .pub file as a new publication.
    ImportPub {
        path: String,
    },
    // ---- MISC task (engine/src/autosave.rs) ----
    /// Autosave into `dir` every `every_actions` document changes.
    SetAutosave {
        dir: String,
        every_actions: u32,
    },
    /// Replace the session document with the newest autosave in `dir` (marked dirty).
    RecoverAutosave {
        dir: String,
    },
    /// Replace every match in every story; one undo step. Outcome is empty; query `find` to verify.
    ReplaceAll {
        find: String,
        replace: String,
        #[serde(default)]
        match_case: bool,
        #[serde(default)]
        whole_word: bool,
    },
    // ---- Mail merge (lead, engine/src/merge.rs) ----
    /// Attach a CSV data source (first row = field names). Undoable.
    AttachDataSource {
        path: String,
        /// Worksheet of an .xlsx workbook (default: the first sheet) (MM-06).
        #[serde(default)]
        sheet: Option<String>,
    },
    /// Show record `record` (index into the filtered, sorted list) in place of the fields; None shows field names.
    SetMergePreview {
        #[serde(default)]
        record: Option<usize>,
    },
    /// Filter the recipient list; with no field the filter is cleared.
    SetMergeFilter {
        #[serde(default)]
        field: Option<String>,
        #[serde(default)]
        op: Option<newpub_core::FilterOp>,
        #[serde(default)]
        value: Option<String>,
    },
    /// Sort the recipient list; with no field the sort is cleared.
    SetMergeSort {
        #[serde(default)]
        field: Option<String>,
        #[serde(default)]
        descending: bool,
    },
    SetMergeOptions {
        #[serde(default)]
        skip_blank_lines: Option<bool>,
    },
    /// Export one copy of the publication per record to a single PDF.
    MergeToPdf {
        path: String,
        #[serde(default)]
        options: Option<PdfOptions>,
    },
    /// Replace the publication with one copy of its pages per record, fields replaced by text. Undoable.
    MergeToPublication {},
    // ---- BLOCKS task (engine/src/blocks.rs) ----
    /// Save the objects as a user building block in the library (replaces a block of the same name).
    SaveBuildingBlock {
        ids: Vec<Id>,
        name: String,
        #[serde(default)]
        category: String,
    },
    /// Copy objects (with their stories, pictures and styles) to the session clipboard. Not undoable (the
    /// document does not change).
    CopyObjects {
        ids: Vec<Id>,
    },
    /// Copy objects to the session clipboard, then delete them. Undoable.
    CutObjects {
        ids: Vec<Id>,
    },
    /// Paste the session clipboard on `page` (default: the page copied from) with its top-left at (x, y);
    /// without a position the objects keep theirs, offset by 12 pt for each repeated paste. Undoable;
    /// created = the new top-level object ids.
    PasteObjects {
        #[serde(default)]
        page: Option<usize>,
        #[serde(default)]
        x: Option<Length>,
        #[serde(default)]
        y: Option<Length>,
    },
    /// Insert a building block (user or built-in) with its top-left at (x, y) on `page`. Undoable;
    /// created = the new top-level object ids.
    InsertBuildingBlock {
        name: String,
        page: usize,
        x: Length,
        y: Length,
    },
    // ---- lead: catalog merge (engine/src/merge.rs) ----
    /// Catalog merge (MM-05): the first item's cell on `page`; objects lying inside it repeat `across` × `down`
    /// per page with `gap` between cells, one record per cell. Undoable.
    SetCatalogArea {
        page: usize,
        rect: Rect,
        across: u32,
        down: u32,
        #[serde(default)]
        gap: Length,
    },
    ClearCatalogArea {},
    // ---- lead: business information sets (engine/src/bizinfo.rs) ----
    /// Save the publication's business information set to the user library (by its name).
    SaveBusinessInfoSet {},
    /// Apply a saved set to the publication (undoable).
    ApplyBusinessInfoSet {
        name: String,
    },
    // ---- FINDFMT task (engine/src/findfmt.rs) ----
    /// Replace all matches of text and/or formatting (FR-03). `find`/`replace` understand the codes ^p (paragraph),
    /// ^l (line break U+2028), ^t (tab), ^s (non-breaking space), ^- (optional hyphen U+00AD), ^^ (caret).
    /// `replace` None keeps the matched text; `replace_format` is overlaid on the matched (or replacement) text.
    /// Nothing to find (no `find` and no `find_format`) → error. One undo step.
    ReplaceAdvanced {
        #[serde(default)]
        find: Option<String>,
        #[serde(default)]
        find_format: Option<CharAttrs>,
        #[serde(default)]
        replace: Option<String>,
        #[serde(default)]
        replace_format: Option<CharAttrs>,
        #[serde(default)]
        match_case: bool,
        #[serde(default)]
        whole_word: bool,
    },
    // ---- PRODUCTS task (engine/src/products.rs) ----
    /// New publication of a built-in type (PG-11): page size, margins and (for cards/labels) `Document.sheet`.
    NewFromPublicationType {
        id: String,
    },
    // ---- PACKGO task (engine/src/packgo.rs) ----
    /// Pack and Go (PR-09): copy the publication, its linked pictures and the fonts it uses into `dir`.
    PackAndGo {
        dir: String,
        #[serde(default = "yes")]
        fonts: bool,
        #[serde(default)]
        pdf: bool,
    },
    // ---- EXPORT task (engine/src/exportx.rs) ----
    /// Fixed-layout EPUB 3 (EX-06).
    ExportEpub {
        path: String,
    },
    /// XPS (EX-06).
    ExportXps {
        path: String,
    },
}

fn yes() -> bool {
    true
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Units {
    #[default]
    In,
    Cm,
    Mm,
    Pt,
    Pi,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageFormat {
    #[default]
    Png,
    Jpeg,
}

fn quality() -> u8 {
    90
}

fn one() -> usize {
    1
}
fn dpi() -> f64 {
    96.0
}

impl SessionAction {
    pub const NAMES: &'static [&'static str] = &[
        "new_document",
        "undo",
        "redo",
        "begin_group",
        "end_group",
        "type_text",
        "insert_picture",
        "import_text",
        "autoflow",
        "save",
        "open",
        "export_pdf",
        "export_png",
        "export_image",
        "replace_all",
        "set_snapping",
        "set_units",
        "set_geometry",
        "save_template",
        "new_from_template",
        "new_from_builtin",
        "ignore_word",
        "relink_picture",
        "embed_picture",
        "export_html",
        "import_pub",
        "set_autosave",
        "recover_autosave",
        "attach_data_source",
        "set_merge_preview",
        "set_merge_filter",
        "set_merge_sort",
        "set_merge_options",
        "merge_to_pdf",
        "merge_to_publication",
        "save_building_block",
        "insert_building_block",
        "copy_objects",
        "cut_objects",
        "paste_objects",
        "set_catalog_area",
        "clear_catalog_area",
        "save_business_info_set",
        "apply_business_info_set",
        "replace_advanced",
        "new_from_publication_type",
        "pack_and_go",
        "export_epub",
        "export_xps",
    ];
}

/// Read-only questions about the session. Answers are JSON.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "q", rename_all = "snake_case", deny_unknown_fields)]
pub enum Query {
    /// The whole document model as JSON.
    Document,
    PageCount,
    /// Hash of the current layout (every glyph position): equal values mean the same layout (PF-01).
    LayoutFingerprint,
    /// `{id, master, ignore_master, background, objects: [ids]}`.
    Page {
        page: usize,
    },
    /// Object JSON (rect, kind, …).
    Object {
        id: Id,
    },
    /// Number of objects on a page (top level).
    ObjectCount {
        page: usize,
    },
    /// Ids of top-level objects on a page, back to front.
    PageObjects {
        page: usize,
    },
    /// Kinds ("text", "shape", "image", "group", "table", "word_art") of every object on a page, back to front,
    /// with group members listed after their group.
    PageObjectKinds {
        page: usize,
    },
    /// Full story text (paragraphs separated by "\n").
    StoryText {
        target: Id,
    },
    StoryLength {
        target: Id,
    },
    /// Ordered frame ids of the story.
    StoryFrames {
        target: Id,
    },
    /// Text displayed in a frame (lines joined with "\n").
    FrameText {
        frame: Id,
    },
    /// Lines in a frame: `[{text, x, y, width, baseline, column, hyphenated}]`.
    FrameLines {
        frame: Id,
    },
    /// Whether the story shown by this frame (or story id) has text that does not fit.
    Overflow {
        target: Id,
    },
    /// First char index that does not fit, or null.
    OverflowAt {
        target: Id,
    },
    /// 0-based page index showing char `at` of the story, or null if it overflows.
    CharPage {
        target: Id,
        at: usize,
    },
    /// Frame id showing char `at`, or null.
    CharFrame {
        target: Id,
        at: usize,
    },
    /// Fully resolved character formatting at char `at`.
    CharAttrs {
        target: Id,
        at: usize,
    },
    /// Fully resolved paragraph formatting of the paragraph containing `at`.
    ParaAttrs {
        target: Id,
        at: usize,
    },
    /// Page index of an object (null when on a master).
    ObjectPage {
        id: Id,
    },
    /// Named styles: `{para: [names], chars: [names]}`.
    Styles,
    /// Masters: `[{id, name, objects}]`.
    Masters,
    /// `{undo, redo}` depths.
    History,
    /// Position of the glyph showing char `at`: `{frame, page, x, baseline, width, line}` in frame-local
    /// points (x = pen position), or null if the char is not displayed (overflow, or a hidden char).
    CharBox {
        target: Id,
        at: usize,
    },
    /// Underline/strike decorations in a frame: `[{kind, x0, x1, y, thickness}]` frame-local.
    Decorations {
        frame: Id,
    },
    /// Margins of a page after facing-page mirroring: `{top, bottom, left, right}` in points.
    PageMargins {
        page: usize,
    },
    /// Page indices grouped as displayed: with facing pages `[[0], [1, 2], [3, 4], …]`, otherwise one per page.
    Spreads,
    /// Sorted, de-duplicated font family names used by any text (resolved through styles).
    FontsUsed,
    /// Font families used by text that are not available in the font store (sorted).
    MissingFonts,
    /// Assets: `[{id, name, mime, px_w, px_h, link}]` in id order.
    Assets,
    /// Find text in all stories: `[{story, start, end}]` ordered by story id then position.
    Find {
        text: String,
        #[serde(default)]
        match_case: bool,
        #[serde(default)]
        whole_word: bool,
    },
    /// Accessibility checker: `[{rule, object, page, message}]`. Rules: `missing_alt_text`
    /// (picture or shape without alt text that is not decorative), `low_contrast` (text colour vs
    /// page background contrast ratio < 4.5), `small_text` (text under 8pt), `overflow` (story with
    /// hidden overflow text).
    AccessibilityCheck,
    // ---- lead ----
    /// `{path, dirty, units}` of the session.
    Session,
    /// All story text concatenated in story-id order, stories separated by "\n".
    AllStoryText,
    /// Number of glyphs laid out in a frame (ligatures make it smaller than the char count).
    GlyphCount {
        frame: Id,
    },
    /// Glyphs in a frame that are the font's missing-glyph (.notdef) glyph.
    MissingGlyphs {
        frame: Id,
    },
    /// Page label (respecting sections), e.g. "ii" or "3".
    PageLabel {
        page: usize,
    },
    /// `{fields, records (after filter), path}` or null when no data source is attached.
    DataSource,
    /// Built-in colour schemes `[{name, main, accent1..accent5, hyperlink, followed_hyperlink}]`.
    ColorSchemes,
    /// The publication's colour scheme (same shape).
    ColorScheme,
    /// Built-in font schemes `[{name, major, minor}]`.
    FontSchemes,
    /// Concrete fill colour of a shape or text frame (scheme colours resolved), or null.
    ResolvedFill {
        id: Id,
    },
    /// `[{name, category, user}]`: built-in blocks then the user library.
    BuildingBlocks,
    /// `{path, user_count}` of the user building-block library.
    BuildingBlockLibrary,
    /// The publication's business information set, or null (BB-05).
    BusinessInfo,
    /// Names of the business information sets in the user library.
    BusinessInfoSets,
    /// Names of the WordArt style presets (TY-19).
    WordArtStyles,
    /// Points of a Bézier/path shape in page coordinates (SH-08): `[{x, y, ctrl_in, ctrl_out, smooth}]`.
    PathNodes {
        id: Id,
    },
    /// Matches of text and/or formatting (FR-03), same shape as Find: `[{story, start, end}]`.
    FindAdvanced {
        #[serde(default)]
        text: Option<String>,
        #[serde(default)]
        format: Option<Box<CharAttrs>>,
        #[serde(default)]
        match_case: bool,
        #[serde(default)]
        whole_word: bool,
    },
    /// Built-in publication types (PG-11): `[{id, name, width, height, per_sheet, sheet}]` (sheet null for single items).
    PublicationTypes,
    PublicationType {
        id: String,
    },
    /// Separation plate names (PR-08).
    SeparationPlates,
    /// Baselines of a frame's lines in page coordinates.
    PageBaselines {
        frame: Id,
    },
    /// Names of the shape kinds the model supports.
    ShapeKinds,
    /// Table geometry: `{rows, cols, col_widths, row_heights, header_rows, format}`.
    Table {
        id: Id,
    },
    /// A cell: `{story, rowspan, colspan, covered, fill}` (a covered cell reports its owner's story).
    TableCell {
        table: Id,
        row: usize,
        col: usize,
    },
    /// Names of the preset table formats.
    TableFormats,
    // ---- GUIDES task ----
    /// Guides on a page (margin grid, page and master ruler guides): `{vertical: [x…], horizontal: [y…]}` sorted, de-duplicated.
    Guides {
        page: usize,
    },
    /// Snap a proposed rect on a page: `{rect, lines: [{orientation, pos}]}`. `tolerance` is the snap distance in
    /// points (default 6; the app passes 6 screen pixels at the current zoom).
    Snap {
        page: usize,
        rect: Rect,
        #[serde(default)]
        ignore: Vec<Id>,
        #[serde(default)]
        tolerance: Option<f64>,
    },
    /// Object geometry formatted in the display unit: `{x, y, w, h, rotation}` strings.
    ObjectGeometry {
        id: Id,
    },
    // ---- TEMPLATES task ----
    /// Built-in templates: `[{id, name, pages}]`.
    BuiltinTemplates,
    // ---- SPELL task ----
    /// Misspellings in story order: `[{story, start, end, word, suggestions}]`.
    Spelling,
    /// `{installed: [langs], missing: [langs used in text without a dictionary]}`.
    SpellingLanguages,
    // ---- LAYERS task ----
    /// Layers bottom to top: `[{id, name, visible, locked}]`.
    Layers,
    // ---- PICTURES task ----
    /// Linked picture assets whose file is missing: `[{asset, path}]`.
    MissingLinks,
    /// Entry names inside a saved .newspub file.
    NpubEntries {
        path: String,
    },
    /// Objects entirely outside their page: `[{id, page}]`.
    OffPageObjects,
    // ---- PDF task ----
    /// Colours used by objects and text: `[Color]` de-duplicated.
    ColorsUsed,
    /// n-up grid for the current page size: `{columns, rows, per_sheet}`.
    NUpLayout {
        sheet_width: Length,
        sheet_height: Length,
        #[serde(default)]
        gap: Length,
    },
    /// Hyperlinks: `[{story, start, end, url | page}]`.
    Hyperlinks,
    // ---- PUB task ----
    /// Inspect a .pub file without importing: `{is_publisher, version, streams}`.
    PubReport {
        path: String,
    },
    /// Report of the last import: `{frames_placed, fonts, warnings}`.
    ImportReport,
    // ---- MISC task ----
    /// Autosave files in a directory.
    Autosaves {
        dir: String,
    },
    /// Bounding box of all glyphs in a frame, page coordinates: `{x, y, w, h}` or null.
    TextBounds {
        frame: Id,
    },
}

/// Convenience for building rect values in code.
pub fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect::new(x, y, w, h)
}
