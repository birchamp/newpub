# newpub-rs: Publisher parity checklist

This is the single source of truth for feature parity with Microsoft Publisher (Microsoft 365).
The goal is behavioural parity. We reproduce what Publisher does, never its code, assets, or templates.

**Rules**
- An item is checked (`[x]`) only when every journey listed for it passes in CI on macOS, Windows, and Linux.
  The first items were checked from CI run 37839317947 (commit addfc94), whose job logs show 93/97 journeys passing on all three OSes;
  the failing set (J-AX-003, J-BB-003, J-BB-004, UI-SH-007) is the same on every OS. The remaining P0–P2 items were checked
  from CI run 37842573100 (commit bb529dc): 100/100 journeys pass on macOS, Windows, and Linux. The P3 items were
  checked from CI run 37854377656 (commit 6c64ad8): 116/116 journeys pass on all three OSes.
  PF-01 and the widened SH-08 were checked from CI run 37859050166 (commit 41e16cb): 119/119 journeys pass on all three OSes.
  UI-09..UI-15 (the redesigned UI) were checked from CI run 37978026939 (commit 6755f2f): 131/131 journeys pass on all three OSes.
  SH-08, AX-03 and PF-01 with their widened journey lists were re-checked from CI run 37864129775 (commit 1d082f4): 122/122 journeys pass on all three OSes.
- Journeys are written by the lead **before** an item is dispatched. The implementing agent never edits them.
- Priorities: **P0** means newpub is unusable without it. **P1** means everyday work needs it.
  **P2** covers professional and occasional workflows. **P3** is rare, legacy, or research.
- Target users (assumed until told otherwise; see ARCHITECTURE.md §0): small organisations, schools, and
  churches that produce newsletters, bulletins, flyers, and folded booklets and print them in-house or at a print shop.
  Their workflows decide what goes into P0 and P1.
- Journey IDs (`J-…`) refer to files in `journeys/scripts/<ID>.yaml`. A `UI-…` journey drives the real egui app through AccessKit.
- The dashboard parses the tables below. Keep the column layout unchanged: `| ID | P | Feature | Journeys | Done |`.

## PG: Page setup and master pages

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| PG-01 | P0 | New publication from a page size (Letter, A4, A5, custom W×H) and orientation | J-PG-001 | [x] |
| PG-02 | P0 | Margins (top/bottom/inside/outside) per publication | J-PG-001 | [x] |
| PG-03 | P0 | Insert, delete, duplicate, and reorder pages | J-PG-002 | [x] |
| PG-04 | P0 | Master pages: create, apply to page, objects on master appear on pages | J-PG-003 | [x] |
| PG-05 | P1 | Two-page (facing) masters and spread view; inside/outside margins mirror | J-PG-004 | [x] |
| PG-06 | P1 | Page numbers, page count, date, and section fields on masters (auto-updating) | J-PG-005, J-PG-009 | [x] |
| PG-07 | P1 | Ignore master on a page; multiple masters per publication | J-PG-003 | [x] |
| PG-08 | P1 | Page background colour / fill | J-PG-006 | [x] |
| PG-09 | P2 | Sections with restart numbering and number format (1, i, I, a, A) | J-PG-005 | [x] |
| PG-10 | P2 | Change page size of an existing publication (objects stay put, reported off-page) | J-PG-007 | [x] |
| PG-11 | P3 | Publication types (envelopes, labels, business cards: multiple pages per sheet) | J-PG-008 | [x] |

## TF: Text frames, linked text, and overflow

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| TF-01 | P0 | Draw text frame, type/insert text, text wraps to frame width | J-TF-001 | [x] |
| TF-02 | P0 | Overflow detection (text that does not fit is flagged) | J-TF-001 | [x] |
| TF-03 | P0 | Link frames (also across pages); text flows frame to frame | J-TF-002 | [x] |
| TF-04 | P0 | Unlink/relink frames; text reflows | J-TF-003 | [x] |
| TF-05 | P0 | Paste long text and auto-flow: add pages + linked frames until text fits | J-TF-004 | [x] |
| TF-06 | P1 | Frame columns with gutter; text flows column to column | J-TF-005 | [x] |
| TF-07 | P1 | Frame internal margins (insets) and vertical alignment (top/middle/bottom) | J-TF-006 | [x] |
| TF-08 | P1 | Text autofit: shrink text on overflow; best fit; grow frame to fit text | J-TF-007 | [x] |
| TF-09 | P1 | "Continued on page / from page" notices | J-TF-008 | [x] |
| TF-10 | P2 | Rotated text frames; vertical text direction | J-TF-009 | [x] |
| TF-11 | P2 | Import text file (.txt / .docx paragraph text) into a story | J-TF-010 | [x] |

## TY: Typography

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| TY-01 | P0 | Font family, size, bold, italic, underline, colour per character run | J-TY-001 | [x] |
| TY-02 | P0 | Paragraph alignment: left, centre, right, justify | J-TY-002 | [x] |
| TY-03 | P0 | Line spacing (multiple and exact pt), space before/after paragraph | J-TY-003 | [x] |
| TY-04 | P0 | Named paragraph and character styles; apply, modify (text updates), based-on | J-TY-004 | [x] |
| TY-05 | P1 | Indents: left, right, first-line, hanging | J-TY-005 | [x] |
| TY-06 | P1 | Tabs: left/centre/right/decimal with leaders | J-TY-006 | [x] |
| TY-07 | P1 | Bulleted and numbered lists | J-TY-007 | [x] |
| TY-08 | P1 | Kerning (font kerning on/off, manual pair adjustment) | J-TY-008 | [x] |
| TY-09 | P1 | Tracking (character spacing) and character scaling | J-TY-008 | [x] |
| TY-10 | P1 | Drop caps (lines, size, font) | J-TY-009 | [x] |
| TY-11 | P1 | Hyphenation (auto, hyphenation zone, manual/optional hyphens) | J-TY-010 | [x] |
| TY-12 | P1 | Superscript, subscript, small caps, all caps, strikethrough | J-TY-011 | [x] |
| TY-13 | P2 | OpenType ligatures (standard, discretionary), stylistic sets, number styles | J-TY-012 | [x] |
| TY-14 | P2 | Baseline guides / align text to baseline grid | J-TY-013 | [x] |
| TY-15 | P2 | Keep with next, keep lines together, widow/orphan control | J-TY-014 | [x] |
| TY-16 | P2 | Special characters: non-breaking space, em/en dash, optional hyphen, line break | J-TY-015 | [x] |
| TY-17 | P2 | Right-to-left and complex-script shaping (Arabic, Hebrew, Devanagari) | J-TY-016 | [x] |
| TY-18 | P3 | Text effects (shadow, outline, glow, reflection, emboss on text) | J-TY-017 | [x] |
| TY-19 | P3 | WordArt-style text objects | J-TY-018 | [x] |

## SH: Shapes and lines

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| SH-01 | P0 | Rectangle, ellipse, line with fill colour, stroke colour and weight | J-SH-001 | [x] |
| SH-02 | P1 | Move, resize, rotate, flip objects | J-SH-002 | [x] |
| SH-03 | P1 | Dash styles, arrowheads, line caps/joins | J-SH-003 | [x] |
| SH-04 | P1 | Group / ungroup; align and distribute objects | J-SH-004 | [x] |
| SH-05 | P2 | AutoShapes library (rounded rectangle, star, arrow, callout, polygon, triangle) | J-SH-005 | [x] |
| SH-06 | P2 | Text inside shapes | J-SH-006 | [x] |
| SH-07 | P2 | Gradient, pattern, and transparency fills; shape shadow | J-SH-007 | [x] |
| SH-08 | P3 | Freeform / Bézier drawing and point editing (canvas tool and point handles) | J-SH-008, J-SH-009, UI-SH-008 | [x] |

## TB: Tables

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| TB-01 | P1 | Insert table rows×cols; type in cells; text wraps in cell | J-TB-001 | [x] |
| TB-02 | P1 | Insert/delete rows and columns; resize rows/columns | J-TB-002 | [x] |
| TB-03 | P1 | Cell borders, fill; merge and split cells | J-TB-003 | [x] |
| TB-04 | P2 | Table formats (preset styles), header row | J-TB-004 | [x] |
| TB-05 | P3 | Paste tabular data (TSV) into a table | J-TB-005 | [x] |

## IM: Images

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| IM-01 | P0 | Insert PNG/JPEG picture; placed at native aspect | J-IM-001 | [x] |
| IM-02 | P0 | Resize keeps aspect; crop (all sides), fit/fill in frame | J-IM-002 | [x] |
| IM-03 | P1 | Text wrap around object: square, tight, top-and-bottom, none, through; wrap distance | J-IM-003 | [x] |
| IM-04 | P1 | Picture placeholder frames; swap picture keeps frame | J-IM-004 | [x] |
| IM-05 | P1 | Alt text on pictures | J-AX-001 | [x] |
| IM-06 | P2 | Recolour, brightness, contrast, greyscale | J-IM-005 | [x] |
| IM-07 | P2 | Picture borders, shapes (crop to shape), shadow, soft edges | J-IM-006 | [x] |
| IM-08 | P2 | Linked (external) vs embedded pictures; relink | J-IM-007 | [x] |
| IM-09 | P3 | SVG, TIFF, GIF and BMP import | J-IM-008 | [x] |
| IM-10 | P3 | Captions (caption gallery) grouped to picture | J-IM-009 | [x] |
| IM-11 | P3 | EMF/WMF (Windows metafile) import as vector pictures (common drawing records) | J-IM-010 | [x] |

## GD: Layout guides and snapping

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| GD-01 | P1 | Margin and column/row grid guides | J-GD-001 | [x] |
| GD-02 | P1 | Ruler guides (add, move, delete) | J-GD-001 | [x] |
| GD-03 | P1 | Snap to guides, objects, and margins when moving/resizing | J-GD-002 | [x] |
| GD-04 | P2 | Rulers, zoom, measurement units (in, cm, mm, pt, pi) | J-GD-003, UI-GD-001 | [x] |
| GD-05 | P2 | Object position/size panel (exact numeric placement) | J-GD-003 | [x] |

## LY: Layers and z-order

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| LY-01 | P0 | Z-order: bring to front/forward, send to back/backward | J-LY-001 | [x] |
| LY-02 | P2 | Layers: create, rename, reorder, hide, lock; move object to layer | J-LY-002 | [x] |
| LY-03 | P2 | Lock object position | J-LY-003 | [x] |
| LY-04 | P3 | Selection pane (list, rename, hide objects) | UI-LY-001 | [x] |

## BB: Building blocks and templates

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| BB-01 | P1 | Save publication as template; new publication from template | J-BB-001 | [x] |
| BB-02 | P1 | Built-in starter templates (original designs: newsletter, flyer, bulletin, booklet) | J-BB-002 | [x] |
| BB-03 | P2 | Building blocks: save selection, insert block; built-in originals (headings, sidebars, pull quotes) | J-BB-003 | [x] |
| BB-04 | P2 | Colour schemes and font schemes applied publication-wide | J-BB-004 | [x] |
| BB-05 | P3 | Business information sets (fields auto-filled into publication) | J-BB-005 | [x] |

## MM: Mail merge and catalog merge

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| MM-01 | P2 | Mail merge from CSV: insert field codes; preview record N | J-MM-001 | [x] |
| MM-02 | P2 | Merge to new publication / to PDF (one copy per record) | J-MM-001 | [x] |
| MM-03 | P2 | Filter/sort recipient list; skip blank fields | J-MM-002 | [x] |
| MM-04 | P2 | Picture fields in merge | J-MM-003 | [x] |
| MM-05 | P3 | Catalog merge (repeating area, multiple records per page) | J-MM-004 | [x] |
| MM-06 | P3 | Excel (.xlsx) data source | J-MM-005 | [x] |

## PR: Print production

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| PR-01 | P0 | Booklet imposition (fold, page order for saddle stitch, blank padding to ×4) | J-PR-001 | [x] |
| PR-02 | P1 | Bleed (extend objects past trim) and BleedBox/TrimBox | J-PR-002 | [x] |
| PR-03 | P1 | Crop marks, registration marks, page information | J-PR-002 | [x] |
| PR-04 | P1 | Multiple pages per sheet (n-up) and duplex settings | J-PR-003 | [x] |
| PR-05 | P2 | CMYK colours in the document model and DeviceCMYK output | J-PR-004 | [x] |
| PR-06 | P2 | Spot colours (Separation colourspace) | J-PR-004 | [x] |
| PR-07 | P2 | Print to system printer (OS print dialog) | UI-PR-001 | [x] |
| PR-08 | P3 | Colour separations and overprint control | J-PR-005 | [x] |
| PR-09 | P3 | Pack and Go (collect fonts/images into a folder) | J-PR-006 | [x] |

## EX: Export

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| EX-01 | P0 | Export PDF: vector text with embedded subset fonts, images, shapes | J-EX-001, J-EX-007 | [x] |
| EX-02 | P1 | Export page as PNG/JPEG at chosen DPI | J-EX-002 | [x] |
| EX-03 | P2 | Export PDF/X-4 (OutputIntent, boxes, no transparency issues). Risk: krilla 0.8 has no PDF/X validator, so this needs our own post-processing | J-EX-003 | [x] |
| EX-04 | P2 | Export HTML (one page per page, positioned) | J-EX-004 | [x] |
| EX-05 | P2 | PDF hyperlinks and bookmarks | J-EX-005 | [x] |
| EX-06 | P3 | Export to XPS / EPUB | J-EX-006 | [x] |

## FI: Native file format

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| FI-01 | P0 | Save and open `.npub`; round-trip preserves everything | J-FI-001 | [x] |
| FI-02 | P1 | Embedded images and fonts list survive round-trip | J-FI-002 | [x] |
| FI-03 | P2 | Autosave and crash recovery | J-FI-003 | [x] |
| FI-04 | P2 | File format versioning and forward migration | J-FI-004 | [x] |
| FI-05 | P1 | Open predecessor NewsPub (Electron) `.newspub` files, converting threads, runs, and assets | J-FI-005 | [x] |

## PI: .pub import (see ARCHITECTURE.md §9; samples in journeys/fixtures/pub)

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| PI-01 | P2 | Open .pub container (OLE CFB); report streams; reject non-Publisher and damaged files gracefully | J-PI-001 | [x] |
| PI-02 | P1 | Import each story into its own text box on its own page | J-PI-002 | [x] |
| PI-03 | P1 | Import page size and orientation, page count, and text box positions | J-PI-003 | [x] |
| PI-04 | P1 | Import character and paragraph formatting (fonts, sizes, bold/italic, spacing) | J-PI-004 | [x] |
| PI-05 | P1 | Import pictures and shapes | J-PI-005 | [x] |
| PI-06 | P2 | Import tables | J-PI-006 | [x] |
| PI-07 | P1 | Import files from Publisher 98, 2000, 2003 and 2010+ | J-PI-007 | [x] |
| PI-08 | P0 | Open a .pub file from the app (Open dialog, file picker, double-click) | UI-PI-001 | [x] |

## UR: Undo/redo

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| UR-01 | P0 | Undo/redo of every document command, multi-level | J-UR-001 | [x] |
| UR-02 | P1 | Typing coalesced into one undo step; undo history survives save (not across sessions) | J-UR-002 | [x] |

## SP: Spell check

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| SP-01 | P1 | Spell check stories against a dictionary (Hunspell-format, en-US bundled) | J-SP-001 | [x] |
| SP-02 | P1 | Suggestions, replace, ignore, add to user dictionary | J-SP-001 | [x] |
| SP-03 | P2 | Per-run language tagging; check other languages if dictionary installed | J-SP-002 | [x] |

## AX: Accessibility

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| AX-01 | P1 | Alt text on all non-text objects; mark decorative | J-AX-001 | [x] |
| AX-02 | P1 | Accessibility checker (missing alt text, low contrast, reading order, tiny text) | J-AX-002 | [x] |
| AX-03 | P2 | Tagged PDF (PDF/UA) with reading order and alt text | J-AX-003, J-AX-004, J-EX-007 | [x] |
| AX-04 | P2 | App UI fully operable by keyboard and screen reader (AccessKit tree) | UI-AX-001 | [x] |

## FR: Find and replace

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| FR-01 | P1 | Find across all stories; match case, whole word | J-FR-001 | [x] |
| FR-02 | P1 | Replace / replace all (one undo step) | J-FR-001 | [x] |
| FR-03 | P3 | Find/replace formatting and special characters | J-FR-002 | [x] |

## PF: Performance

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| PF-01 | P2 | Editing and exporting a long publication stays responsive (no quadratic slow-downs; edits re-flow only what they change) | J-PF-001, J-PF-002 | [x] |

## AG: Agent access

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| AG-01 | P1 | An AI agent can do everything a person can: an MCP server (`newpub-agent`, stdio) and a command line over every engine action and query, with a generated reference, page rendering, status and warnings, batches as one undo step, open/save/export | J-AG-001 | [x] |
| AG-02 | P1 | The running app hosts the same MCP server (`newpub --agent`): the agent's changes show in the window, the status bar says what it did, the person's selection and page stay valid and both can keep working | UI-AG-001 | [x] |

## UI: Application shell (real-UI journeys)

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| UI-01 | P0 | Window with page canvas, page navigator, ribbon/toolbars; launches on all three OSes | UI-SH-001 | [x] |
| UI-02 | P0 | Create publication, draw text frame, type text, export PDF through the UI | UI-SH-002 | [x] |
| UI-03 | P1 | Select, move, and resize objects with mouse and keyboard nudges | UI-SH-003 | [x] |
| UI-04 | P1 | Format panels (font, paragraph, object) wired to commands | UI-SH-004 | [x] |
| UI-05 | P1 | Open/save dialogs, recent files | UI-SH-005 | [x] |
| UI-06 | P2 | Zoom, scroll, two-page spread view, rulers | UI-GD-001 | [x] |
| UI-07 | P1 | Keyboard shortcuts carried over from NewsPub (duplicate, z-order, zoom stops, fit, nudge, Tab through story frames) | UI-SH-006 | [x] |
| UI-08 | P2 | Template picker at startup; new publication from a built-in or saved template | UI-SH-007 | [x] |
| UI-09 | P1 | Insert tab: tables, WordArt, shape gallery, fields, building blocks from the ribbon | UI-IN-001 | [x] |
| UI-10 | P1 | Page Design tab: page setup, colour and font schemes, background, master pages, grid guides | UI-PD-001 | [x] |
| UI-11 | P1 | Mailings and Review tabs: recipients, merge fields, preview, merge to PDF; spelling, find and replace, accessibility checker | UI-ML-001, UI-RV-001 | [x] |
| UI-12 | P0 | Inspector: fill, line, text colour, spacing, lists, frame margins and alignment, rotation, alt text | UI-IS-001 | [x] |
| UI-13 | P1 | One export dialog for PDF/X-4, PNG, HTML, EPUB, XPS and Pack and Go | UI-EX-001 | [x] |
| UI-14 | P0 | Text editing with a caret: click to place, arrows, selection, typing and deleting at the caret, formatting the selection | UI-TE-001 | [x] |
| UI-15 | P1 | Clipboard and context menu: cut, copy, paste objects and text; right-click menu | UI-CB-001 | [x] |
| UI-16 | P0 | Unsaved changes are never lost silently: Save Changes / Don't Save / Cancel before closing the window, New and Open | UI-SV-001 | [x] |
| UI-17 | P1 | Drag and drop from the desktop: pictures are placed where dropped; publications and .pub files open | UI-DD-001 | [x] |
| UI-18 | P0 | AutoRecover in the app: after a crash the next start offers unsaved work back (per-window locked copies; discarded work is not offered) | UI-AR-001 | [x] |
| UI-19 | P0 | Text flow in the UI: link text boxes, break links, flow onto new pages, import a text file, type in shapes, clear formatting | UI-TX-001, UI-TX-002 | [x] |
| UI-20 | P0 | Table editing in the UI: type in cells, Tab, merge/split, insert/delete rows and columns, sizes, table style, cell fill | UI-TB-101 | [x] |
| UI-21 | P1 | Pages and guides in the UI: reorder and duplicate pages from the page list; drag ruler guides out, move them, drag them back to remove | UI-PG-101 | [x] |
| UI-22 | P1 | Styles in the UI: paragraph and character styles from formatted text, apply, delete | UI-ST-001 | [x] |
| UI-23 | P1 | Publication properties, Design Checker (overflow, empty boxes, off-page objects), Save as Template and My Templates | UI-DC-001 | [x] |
| UI-24 | P2 | Gallery live preview: hovering a colour or font scheme shows it on the page before it is applied; built-in templates follow the schemes | UI-PD-002 | [x] |
| UI-25 | P1 | Text wrap in the UI: wrap text around any object placed over a text box (none, square, tight, top and bottom, through) with a distance, from the Format panel | UI-WR-001 | [x] |
| UI-26 | P1 | Text flow feedback in the UI: the boxes of a linked story are numbered on the canvas with Previous / Next buttons (also in the Format panel, with "Box n of m"); Link to Next Box shows a banner, highlights the boxes the story can continue in, keeps the mode on a wrong box and cancels with Esc | UI-TX-003 | [x] |
| UI-27 | P1 | Continued notices in the UI: "Include 'Continued on page…'" and "Include 'Continued from page…'" in the Format panel's Text box card turn Publisher's generated notices on per box; greyed out with the reason on unlinked, first, last and master-page boxes; several selected boxes change as one undo step | UI-TX-004 | [ ] |
