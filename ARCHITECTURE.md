# newpub-rs architecture

A fresh session should be able to resume work from this file, PARITY.md, PROGRESS.md, and `journeys/` alone.

## 0. Context and assumptions

- **Predecessor (corrected 2026-10-08).** The existing newpub is the Electron + React + TypeScript app on
  `birchamp/newspub` branches `master` (spec + plan + scaffold) and `claude/ui-testing-responsiveness-smep9u` (fuller implementation).
  Its model and UX decisions are surveyed in §12. The original Phase 0 note below described only the `main` branch:
  `main` held only a README when the rewrite began ("Simple desktop publishing
  app using Pretext as the layout engine"). There was no document model or UX to carry over. The single
  inherited decision is the **Pretext approach to text layout**: measure text segments once, then lay them
  out with cheap arithmetic, never through a DOM. Our layout engine keeps that approach (§5).
- **Repository.** newpub-rs lives in this repository (`birchamp/newspub`) as a Cargo workspace. The brief's
  `<<repo URL>>` was left blank, and this is the only repository in scope.
- **Target users (assumed).** Small organisations, schools, and churches that make newsletters, bulletins,
  flyers, and folded booklets. P0/P1 ordering in PARITY.md follows from this. Change it here if the
  assumption is wrong.
- **Dashboard publishing.** The dashboard is GitHub Pages, built by CI. It is also mirrored to a claude.ai
  Artifact while sessions run. The brief's `<<GitHub Pages / local path>>` was left blank.

## 1. Crates

| Crate | Path | Role | May depend on |
|-------|------|------|---------------|
| `newpub-core` | crates/core | Document model, units, colours, styles, `Command` enum, `apply`, undo `History`. No IO, no fonts, no UI. | serde |
| `newpub-layout` | crates/layout | Font store, shaping, line breaking, hyphenation, frame flow, columns, wrap. Produces a `DocLayout`. | core |
| `newpub-render` | crates/render | Paints a page (or imposed sheet) from `Document` + `DocLayout` to a raster. Also provides the shared `PageGeometry` and the display-list walk used by PDF. | core, layout |
| `newpub-io-pdf` | crates/io-pdf | PDF export (krilla): boxes, bleed, marks, imposition, CMYK/spot, PDF/X, tagging. | core, layout, render (display list only) |
| `newpub-io-native` | crates/io-native | `.npub` save/open (zip: `document.json` + `media/*`). | core |
| `newpub-io-pub` | crates/io-pub | MS Publisher `.pub` import (research track). | core |
| `newpub-engine` | crates/engine | `Session`: owns the document, history, fonts, and layout cache. Executes `Action`s (commands, undo/redo, save/open, export, autoflow, merge) and answers `Query`s. The app and the journey runner **both** drive the program only through this. | everything except app |
| `newpub-app` | crates/app | egui/eframe desktop app. A thin view over `Session`: every UI gesture becomes an `Action`. Exposes `NewpubApp` as a lib for UI journeys. | engine, core |
| `newpub-agent` | crates/agent | Agent access: MCP server (stdio) and command line over `Session` actions and queries; the app hosts the same server with `--agent`. Embeds the generated docs/agent-reference.md. | core, engine |
| `newpub-journeys` | crates/journeys | Headless runner (`newpub-journeys` binary) and UI-journey harness. Scripts live in `/journeys/scripts`. | engine, app |
| dashboard | tools/dashboard | Python 3 stdlib script that writes `dashboard/index.html`. | — |

`newpub-engine` is an addition to the crate list in the brief. It exists so the app and the headless runner share
one dispatch path (rationale: "every user action is a command" can only be checked if the UI cannot bypass it).

## 2. Crate choices (one line each)

- **UI: egui/eframe 0.36.** Immediate mode keeps the UI a thin view over `Session`. It has built-in AccessKit on all three
  OSes, and `egui_kittest` drives the real UI headlessly through the AccessKit tree, which makes UI journeys possible
  in CI without a display server.
- **Raster rendering: tiny-skia 0.12.** CPU-only and deterministic across OSes, so golden images stay stable. Vello needs a GPU (and
  vello_cpu is still young).
- **Shaping: rustybuzz 0.20 + ttf-parser 0.25.** Same versions krilla uses (one copy of each in the tree); full OpenType
  feature control for kerning, ligatures, and small caps.
- **Text layout: our own engine (`newpub-layout`).** parley and cosmic-text lay out a paragraph at one fixed width. Publisher
  needs a different width per line (wrap around objects, drop caps) and lines that continue across columns and frames.
- **Line breaking: unicode-linebreak** (UAX #14). **Hyphenation: hyphenation** (Knuth–Liang, en-US embedded).
- **Fonts: fontdb 0.24** for system fonts in the app. Journeys load **only** the bundled fonts in `assets/fonts`
  (Carlito ≈ Calibri metrics, Liberation Serif/Sans ≈ Times/Arial metrics, DejaVu Sans as fallback; all OFL or
  Bitstream-Vera licensed) so output is identical on every OS.
- **PDF output: krilla 0.8.** Font subsetting, PDF/A, PDF/UA tagging, and validation for export profiles. Actively maintained.
- **PDF inspection in journeys: lopdf** (structure: pages, boxes, fonts) + **hayro** (rasterise exported PDFs for pixel checks).
- **Images: image 0.25** (PNG/JPEG decode for raster rendering; krilla embeds the original bytes).
- **Native format: zip + serde_json.** Human-diffable JSON, media kept as separate entries.
- **.pub import: cfb 0.15** (OLE compound files); see §9.
- **Spell check: spellbook 0.4** (pure-Rust Hunspell-compatible, MPL-2.0 used as an unmodified dependency) with the bundled SCOWL en_US dictionary (`assets/dict`, permissive SCOWL licence).
- **Toolchain pinned** to Rust 1.97.0 (`rust-toolchain.toml`), so clippy lints are identical locally and in CI.

## 3. Document model (`newpub-core`)

All geometry is in **points** (`f64`, 1/72 in), page coordinates, origin top-left, y down.
All ids are `Id(u64)` from one counter per document (`Document::next_id`), so any object, story, page, master, style, or
asset can be named by one integer.

```
Document
├── setup: PageSetup { width, height, margins{top,bottom,inside,outside}, facing, bleed }
├── pages: Vec<Page { id, master: Option<Id>, ignore_master, background: Option<Color>, objects: Vec<Id> /* back→front */ }>
├── masters: Vec<Master { id, name, objects: Vec<Id> }>
├── objects: BTreeMap<Id, Object { id, name, rect, rotation, kind, wrap, alt_text, decorative, locked, layer }>
│     kind = Text(TextFrame{story, columns, gutter, insets, valign, autofit}) | Shape(Shape{..}) | Image(ImageFrame{asset, crop, fit}) | Table(..) | Group(..)
├── stories: BTreeMap<Id, Story { id, text: String, chars: Vec<CharSpan>, paras: Vec<ParaAttrs>, frames: Vec<Id> }>
├── styles: Styles { para: BTreeMap<Id, ParaStyle>, chars: BTreeMap<Id, CharStyle> }
├── assets: BTreeMap<Id, Asset { name, mime, bytes: Arc<[u8]> /* not in JSON; media/ in .npub */ }>
└── next_id
```

**Stories are attributed strings.** `text` uses `'\n'` as the paragraph separator. `chars` is a contiguous list of
`CharSpan { len, attrs }` (lengths in chars) that covers the text exactly. `paras` has one `ParaAttrs` per paragraph
(always `text.matches('\n').count() + 1`). All text positions in the API are **char indices** into `text`. The model
normalises itself after each edit (merges equal adjacent spans, drops empty ones).

**Formatting resolution** (lowest to highest priority): built-in defaults → paragraph style chain (`based_on`) → that
style's char attrs → character style chain → run overrides. Attrs are structs of `Option<T>`; `None` means inherit.

**Linked frames.** A story owns the ordered chain `frames`. Linking B after A requires B's story to be empty; B's story
is deleted and B joins A's chain. Unlinking A splits the chain after A; the frames after A keep reading A's story
(Publisher behaviour: the text stays in the story and goes to overflow).

## 4. Command layer

- `core::Command` is a serde enum (`#[serde(tag = "cmd")]`, snake_case). It holds every **document mutation**. `Document::apply(&Command) -> Result<Applied, CoreError>`
  where `Applied { created: Vec<Id> }`.
- `engine::Action` = `Command(Command)` plus session-level actions: `undo`, `redo`, `begin_group`/`end_group`,
  `save`, `open`, `export_pdf`, `export_png`, `autoflow`, `new_document`, …
- `engine::Query` holds read-only questions answered as `serde_json::Value`: `page_count`, `story_text`, `frame_text`,
  `overflow`, `object`, `char_attrs_at`, `frame_lines`, … Journeys assert only through queries and exported files.
- Every command is undoable. **Undo model (lead-owned):** snapshot history. Before each top-level action the session
  stores the previous `Document` (cheap: assets are `Arc`, everything else is small). Typing commands with
  `coalesce: true` that continue the previous insert at its end position merge into one step. `begin_group`/`end_group`
  make compound actions such as autoflow and replace-all into one step. Snapshots were chosen over inverse commands
  because every new command is then undoable automatically, with no per-command inverse code to get wrong. History is capped at 200 steps.

**Interface rule:** changes to `Command`, `Action`, `Query`, or the model types in `core` are made by the lead only and
recorded in §11.

## 5. Layout engine (`newpub-layout`, lead-owned)

The Pretext approach applied to frames:
1. **Itemise:** split each paragraph into style runs, then into font-coverage runs (fallback fonts).
2. **Shape** each run once with rustybuzz (features from attrs: `kern`, `liga`, `dlig`, `smcp`, …). Cache by (face, size, features, text).
3. **Segment:** UAX #14 break opportunities give segments; each segment records advance width, trailing-space width,
   and glyph slice. Hyphenation points inside long words become extra optional break points with hyphen width.
4. **Flow:** a `Region` iterator yields line slots: for each frame in the story's chain, each column, the next line top `y`.
   The available x-intervals at `[y, y+lineheight]` come from the column minus wrap exclusions of objects above it in z-order.
   The greedy first-fit line breaker (Publisher's behaviour) fills slots with segments. A line that does not fit vertically moves on to
   the next column, then the next frame. Whatever is left is **overflow** (`StoryLayout.overflow_at: Option<char index>`).
5. **Finish lines:** alignment and justification (space distribution), tabs, drop caps (an exclusion box for N lines),
   then per-frame vertical alignment.

**Incremental layout (PF-01).** `layout_document_with(doc, fonts, &mut LayoutMemo)` keeps each story's previous flow:
per-paragraph input keys (paragraph attributes, text, runs relative to the paragraph, paragraph-mark attributes), the
flow state at each paragraph start (the keep-rule `Snap`), per-frame geometry keys, and the raw lines before vertical
alignment and continued notices. A document-wide key covers styles, schemes, the baseline grid and the font store.
On the next layout a story resumes at its first changed paragraph. It backs up while keep-with-next ties a paragraph to
the one before it, or while that start state lies in a changed frame. It stops as soon as an unchanged paragraph starts
in exactly the saved state (with no keep-with-next across the join): the saved lines after it are appended, shifted to
the new char and paragraph offsets. Stories with fields (page numbers and the like) are always laid out in full.
`NEWPUB_VERIFY_INCREMENTAL=1` checks every incremental result against a full layout through `DocLayout::fingerprint`
(the journey suite passes in that mode). The engine keeps one memo per session; autoflow uses its own memo and adds
pages in batches estimated from the fullest frame.

Output: `DocLayout { frames: HashMap<Id, FrameLayout { lines: Vec<Line> }>, stories: HashMap<Id, StoryLayout> }` with glyph
positions in **frame-local** coordinates (the renderer applies the frame transform, so rotation works).

## 6. Rendering and export

`newpub-render` builds a per-page **display list** (`Vec<DisplayItem>`: path fill/stroke, image, glyph run, clip, transform)
from model + layout. Two back ends consume it: tiny-skia (PNG, screen) and krilla (PDF). This keeps screen and PDF
output consistent. Imposition (booklet, n-up) maps pages onto sheets as transforms over display lists.

## 6a. App UI (`newpub-app`)

- **Design system:** `theme.rs` holds the light and dark `Palette` (Venice Blue `#085078` to seafoam `#85D8CE`),
  the Inter and Phosphor font families and the egui style. `widgets.rs` has the shared buttons, cards and fields; every
  widget sets its AccessKit label, which is what UI journeys click.
- **Shell:** `shell.rs` draws the gradient header, the tabbed ribbon (Home, Insert, Page Design, Mailings, Review,
  View; every tab the same height) and the status bar. Each non-Home tab lives in `tabs/` with its own window state.
- **Panels:** `pages.rs` (thumbnails), `inspector.rs` (Format panel), `view.rs` (canvas, rulers, zoom), `files.rs`
  (Save, Open, Export As, Insert Picture; `dialog_window` centres every dialog, Escape cancels), `picker.rs`
  (start screen).
- **Text editing:** `text_edit.rs` keeps a `Caret {frame, pos, anchor}` in story offsets and hit-tests against the
  layout's glyph positions. Formatting from shortcuts and the Format panel targets the selection
  (`text_target_range`) or the caret's paragraphs (`para_target_range`); inserts go to `insertion_point`.
- **What the caret edits:** an "editable" is a text frame, a shape (its story), or a table cell (the cell's story id).
  Commands target the story; the selection holds the object that shows it (`holder`). `table_ui.rs` adds the current
  cell, the Shift-click cell block (`ViewState.cell_extent`), Tab between cells and the Format panel's Table card.
- **Canvas tools beyond selection:** link mode for "Link to Next Box" (`ViewState.link_from`, also started from a
  frame's overflow badge); ruler guides (`guides_ui.rs`: drag out of a ruler, move, drag back to delete; objects win
  over guides when both are under the pointer). Clicking the page gives it keyboard focus.
- **Windows:** Styles (`styles_ui.rs`, styles by example via `effective_para_attrs`/`effective_char_attrs`),
  Properties and Design Checker (`checker.rs`; the checker combines layout overflow, empty stories and the
  `OffPageObjects`, `MissingFonts` and `MissingLinks` queries). "My templates" are `.npubt` files in
  `user_templates`, made by the Save dialog's Save as Template and listed on the start screen.
- **Preferences:** the recent-files list and the theme choice live in the OS config dir (`recent.rs`); journeys keep
  both in memory.

## 6b. Agent access (`newpub-agent`)

An AI agent gets the same interface the journeys use (§4): every action and query, as JSON. `newpub-agent` is an
MCP server over stdio (JSON-RPC 2.0, one message per line) with ten tools: `newpub_reference` (the generated
docs/agent-reference.md, searchable), `newpub_status`, `newpub_action`, `newpub_actions` (an undo group),
`newpub_query`, `newpub_render_page` (PNG as image content), `newpub_new`, `newpub_open`, `newpub_save` and
`newpub_export_pdf`. Tool failures are results with `isError`, so the agent can read and retry. The dispatch is
generic over a `Host` (a `Session`, or the app): `newpub --agent` runs the same server inside the desktop app,
where a reader thread queues requests and the UI answers them each frame, keeps its page and selection valid and
shows "Agent: …" in the status bar (crates/app/src/agent.rs). The command line (`exec`, `query`, `render`,
`status`, `reference`) wraps the same tools. The runner step `agent: {tool, args, path, check, as}` calls a tool
in-process in both headless and UI journeys. docs/agent.md is the user guide.

## 7. Journeys (the only tests)

- Scripts: `journeys/scripts/<ID>.yaml`. Fixtures: `journeys/fixtures/`. Approved goldens: `journeys/goldens/<ID>/<name>.png`.
- Runner: `cargo run -p newpub-journeys --release -- [--filter ID] [--out DIR]`. It writes `DIR/<ID>/…` (exports and
  page screenshots) and `DIR/results.json`.
- Step kinds: an **action** (`- add_text_frame: {...}`), optionally `as: name` to bind the created id, used later as
  `"$name"`; **`expect`** with a `query` and a matcher (`equals`, `contains`, `gt`, `lt`, `approx`, `len`);
  **`expect_pdf`** (pages, text, fonts, boxes, pixel probes via hayro); **`expect_png`** (golden compare with
  tolerance); **`expect_roundtrip`** (save → open → document equality).
- Lengths in scripts may be numbers (pt) or strings with units: `"8.5in"`, `"210mm"`, `"2cm"`, `"12pt"`, `"3pi"`.
- **Golden approval:** a missing golden writes `DIR/<ID>/pending/<name>.png` and fails with `needs-approval`. Only the lead
  copies pending images into `journeys/goldens/`.
- UI journeys (`UI-*.yaml`, `ui: true`) run the real `NewpubApp` in `egui_kittest`, find widgets by AccessKit
  label/role, click and type, then assert through the same queries.
- Agents implementing features **never** add or edit anything under `journeys/` or `crates/journeys/`.

## 8. CI

`.github/workflows/ci.yml` runs on ubuntu-latest, macos-latest, and windows-latest: `fmt --check`, `clippy --all-targets -D warnings`,
build, and the full journey suite. Each OS uploads `journey-results-<os>` (results.json, screenshots, PDFs). A final job merges
the three result sets with per-OS job status, runs `tools/dashboard`, uploads it as an artifact, and deploys it to GitHub Pages.

## 9. .pub import (`newpub-io-pub`)

The format is undocumented. libmspub (MPL-2.0, LibreOffice) is read to learn it, never copied or translated: MPL is
file-level copyleft. The format notes are in crates/io-pub/FORMAT-NOTES.md.

- Two readers produce one format-neutral intermediate form (`ir.rs`):
  - `v2002.rs` reads Publisher 2002+: the `Contents` chunk directory plus the Escher drawing records.
  - `legacy.rs` reads Publisher 98/2000: the fixed-layout `Contents` records.
- Both read the shared Quill text stream (`quill.rs`).
- `build.rs` turns the intermediate form into a newpub `Document`: pages, text boxes and their chains, formatting,
  pictures (decoded by the engine, WMF/EMF through `newpub-io-metafile`), shapes, groups and tables.
- What is not imported goes into the import report's warnings.
- Opening a `.pub` anywhere in the app runs `SessionAction::Open`, which imports it as a new, unsaved publication.
- Journeys J-PI-001..007 use Apache POI's Publisher samples. Expected positions come from LibreOffice's rendering of
  the same files.

## 10. Conventions

- Rust 2024, stable. `cargo clippy --all-targets -- -D warnings` must be clean.
- No `unwrap()` on user-controlled data in core, layout, or io. Return `CoreError` / `anyhow` at the edges.
- Feature work must not add placeholders. Return `Err(Unsupported)` until a feature really works.
- No Microsoft code, assets, or templates. Built-in templates and building blocks are original designs.

## 11. Interface change log

| Date | Change | Reason |
|------|--------|--------|
| 2026-10-08 | Initial `Command`/`Action`/`Query` sets defined (see crates/core/src/command.rs, crates/engine/src/action.rs) | Phase 0 |
| 2026-10-08 | Batch 2 lead additions: fields/sections, tables (`ObjectKind::Table`), baseline grid, special chars, shape text, guides/links/layers/words module hooks, per-task engine modules | Batch 2 dispatch and lead layout work |
| 2026-10-08 | Mail merge: `Document.merge: Option<MergeData>`, `InsertMergeField`, `SetPictureField`, `core::merge` (`Document::merged`, `merge_publication`); engine merge actions and `DataSource` query; Session keeps a preview `view` document used by layout, rendering and PDF export | MM-01..MM-04 |
| 2026-10-08 | `Story::insert` with inherited attrs never extends a field run | Typing after a field duplicated it |
| 2026-10-08 | `Command::DuplicateObjects{ids, dx, dy}` (deep copy via `duplicate_object`) | App Cmd+D rebuilt objects command by command and could not copy groups/tables |
| 2026-10-08 | `core::fragment` (`Fragment`, `Document::extract_fragment`, `paste_fragment`); engine `SaveBuildingBlock`/`InsertBuildingBlock`, `BuildingBlocks`/`BuildingBlockLibrary` queries, `Session.library_dir` | BB-03 building blocks (and a future clipboard) |
| 2026-10-08 | `Color::Scheme{slot, a}` (short form `{scheme: accent1}`), `Color::with_alpha`; `core::schemes`; `Document.color_scheme`/`font_scheme`; `ApplyColorScheme`/`ApplyFontScheme`; fonts "+major"/"+minor"; `resolve_char` resolves scheme fonts/colours; engine view doc resolves object scheme colours; queries ColorSchemes/ColorScheme/FontSchemes/ResolvedFill | BB-04 |
| 2026-10-08 | Object query always reports `layer` (null when unset) | LY-02 journey; file format unchanged |
| 2026-10-08 | App: `NewpubApp.startup_picker`, `print_spool`; journey runner steps `focus`, `scroll`, journey option `startup_template_picker` | UI-08, PR-07, AX-04, UI-06 journeys |
| 2026-10-08 | P3 interfaces (Batch 4): `CharAttrs.effects: TextEffects` (→ `ResolvedChar.effects`, `GlyphRun.effects`); `Object.hidden`, `Object.overprint` (+ patch); `ObjectKind::WordArt` (`core::wordart`), `ShapeKind::Bezier` + `BezierNode` (`core::freeform`, render `bezier_path`); commands PasteTableText, AddCaption, AddWordArt, SetWordArt, AddFreeform, Move/Insert/DeletePathNode, SetPathNodeKind, SetBusinessInfo, InsertBusinessField; `Field::Business`; `Document.sheet: SheetLayout`, `Document.business_info`; `MergeData.catalog`; `Imposition::DocumentSheet` (impose `plan` takes the doc sheet); `PdfOptions.separations` (`io-pdf::separations`); engine actions SetCatalogArea, ClearCatalogArea, Save/ApplyBusinessInfoSet, ReplaceAdvanced, NewFromPublicationType, PackAndGo, ExportEpub, ExportXps, AttachDataSource `sheet`; queries BusinessInfo(Sets), WordArtStyles, PathNodes, FindAdvanced, PublicationType(s), SeparationPlates; `imgformats::decode_picture`; runner `expect_files`, `expect_zip` | P3 items |
| 2026-10-09 | Layout performance: `layout::text::slice` (per-pass char→byte tables replacing `Story::slice` in layout), global shaping cache in `layout::shape::shape`, glyph-coverage cache in `Face::has_glyph`; runner step `timed`; every PDF export is tagged, RTL runs get /ActualText; pdfcheck `actual_text_contains` | PF-01 (8 s → ~45 ms per edit on a 20-page story), RTL copy/paste |
| 2026-10-09 | Incremental layout: `layout::LayoutMemo`, `layout_document_with`, `layout_one_with`, `DocLayout::fingerprint`, `FontStore::face_count`; engine `Session.layout_memo`, query `LayoutFingerprint`; autoflow adds pages in batches. App: Windows prints natively through GDI (`print_win`, test hook `NEWPUB_PRINT_OUTPUT`) | PF-01 (J-PF-002: ~150 ms → ~12 ms per keystroke on a 60-page story), PR-07 on Windows |
| 2026-10-09 | Engine object clipboard: `SessionAction::CopyObjects`, `CutObjects`, `PasteObjects{page,x,y}` over `core::fragment` (session clipboard; paste cascades 12 pt). App: design system (`theme.rs`, `widgets.rs`, `icons.rs`), shell (`shell.rs`), per-area modules (`inspector.rs`, `files.rs`, `tabs/`), caret text editing (`text_edit.rs`), clipboard and context menu (`clipboard.rs`); runner `click_at` with button/double/shift and a `paste` step | UI redesign, UI-14, UI-15 |
| 2026-10-09 | `Field::Date(DateFormat)` (`NEWPUB_TODAY` pins the date in journeys); app `view` query reports `text_selection`; runner allows `expect_files`/`expect_zip` in UI journeys | Insert tab date field, Find Next selecting text, UI-EX-001 |
| 2026-10-09 | Query `PageObjectKinds{page}` (kinds of every object on a page, group members included) | .pub import journeys (J-PI-005, J-PI-006) |
| 2026-10-09 | `Session::render_page_preview(page, dpi, cmd)` (renders a copy with one command applied); colour scheme "Navy"; built-in templates use scheme colours and fonts ("+major"/"+minor") with the Navy and Editorial schemes | Gallery live preview (UI-PD-002); schemes restyle templates |
| 2026-10-09 | Layout `fonts::substitutes`: missing Windows fonts resolve to bundled stand-ins (Liberation for Times New Roman/Arial, Carlito for Calibri, else by kind) before the generic fallback; `char_box` also reports `page_x`/`page_baseline`; `.pub` import rewritten (v2002 + Publisher 98/2000 readers over one IR) | Real .pub import (PI-01..PI-07) |
| 2026-10-10 | `newpub-agent` crate: `Host` trait (impl for `Session` and `NewpubApp`), `tools()`, `call_tool`, `handle` (JSON-RPC), `serve_stdio`, `stdio_link`/`Link`, `status`, embedded `REFERENCE`; app `--agent`, `NewpubApp::attach_agent`, `sync_after_change`; runner step `agent`; app `view` query reports `status`; CI checks `tools/agent/reference.py --check` and runs the crate's stdio test | AG-01, AG-02 |
| 2026-10-31 | App only (no engine changes): canvas multi-selection (`NewpubApp.marquee`; Shift-click toggles, a Select-tool drag over empty page is a marquee); runner `drag` takes `shift: true`; the runner draws a frame after `expect_roundtrip` | UI-28 |
| 2026-10-09 | App only (no engine changes): `ViewState.link_from`, `cell_extent`, `guide_drag`, `canvas_area`; `Dialog::InsertText`, `Dialog::SaveTemplate`; app `view` query reports `table_cell` and `table_selection`; page renders capped at the GPU's maximum texture side. Runner: widgets are scrolled into view with the mouse wheel before `click`/`fill`, Shift is held for a whole shift-click, a control is preferred over a caption with the same name, `step_dt` 1/30 s | UI-19..UI-23 |

## 12. Predecessor survey (NewsPub, Electron + React + TS) and what we adopt

Source: `birchamp/newspub` branches `master` and `claude/ui-testing-responsiveness-smep9u`, surveyed read-only on 2026-10-08.

**Target users (from its spec).** Non-technical users making multi-page newsletters on desktop. Their workflow: start from a
template, type into linked frames that flow across pages, place images with text wrap, export PDF. This confirms the P0/P1
ordering in PARITY.md.

**Same as ours already:** points with y down; text lives in stories ("threads") that frames display in order; z-order is the
order of the page's object list; zip file with `document.json` plus separate asset bytes and an atomic save; one layout feeding
both screen and PDF.

**Adopted (no conflict with Publisher):**
- File extension **`.newspub`** for native files (our format, version 1). We also **open the predecessor's `.newspub`**
  (`metadata.json` + `document.json` with `threads`/`runs` + `assets/`) by converting it on load. See FI-05.
- Keyboard: Ctrl/Cmd+Z, Shift+Z / Y redo; B / I; **D duplicate (+12, +12 pt)**; **] front, [ back**; = / - zoom to the next
  or previous stop (25, 50, 75, 100, 150, 200, 300, 400 %); **0 fit**; N / O / S / Shift+S / **E export PDF**.
  Arrows nudge 1 pt, Shift+arrows 10 pt, and held nudges within 600 ms are one undo step. Delete / Backspace deletes;
  **Tab cycles through the frames of the selected story**; Esc deselects or leaves text editing; Enter starts editing. See UI-07.
- Snapping tolerance is 6 screen px. Targets are page edges, margins, page centre, and sibling objects' edges and centres. When
  moving, the closest match per axis wins; when resizing, only the dragged edges snap. No snapping while nudging. Same as GD-03 plus
  the page-centre target.
- Undo grouping: a typing burst is one step (we coalesce on contiguous insertion; the predecessor used 900 ms), and a drag
  gesture is one step.
- Spread view: page 1 alone on the right, then 2+3, 4+5 (matches Publisher's facing-pages display and our `spreads` query).
- The template picker opens at startup (UI-08). Its "Classic 4-Page Newsletter" layout (headline frame, two-column body
  threaded across pages 1–3, image band on page 3, contact block on page 4) becomes our built-in **newsletter** template (BB-02).
- Save As Template options: keep article text (off), keep placed images (off), keep background images (on).
- Empty frames show a dashed placeholder ("Text" / "Image"). Overflow shows a red "+" marker at the frame's bottom-right.

**Not adopted (Publisher parity wins):**
- Default text 14 pt sans with line height 1.4 → we use Publisher-like 11 pt Carlito with font metrics.
- 8 pt frame padding → 0.08 in, Publisher's default.
- "Cont. pg N" → "(Continued on page N)" / "(Continued from page N)".
- No named styles → we have paragraph and character styles (TY-04).
- Rect-wrap-one-side → wrap on both sides (IM-03).
- Fixed 36 pt margins → per-publication margins (PG-02).

Known predecessor bugs we do not reproduce: image wrap leaked across pages; PDF used only standard-14 fonts; WebP was skipped in PDF.
