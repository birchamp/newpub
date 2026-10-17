# PROGRESS

Build log for newpub-rs. Each entry records what merged, what failed, and the decisions made.
The dashboard parses this file:
- Bullets under **Open blockers** are pinned at the top of the dashboard. Each starts with an ID in brackets.
- Each `###` heading under **Log** is one entry. Bullets containing `ESCALATION:` feed the escalation log.

## Open blockers (need human input)

- [B-003] Usability testing with the target users (newsletter and bulletin makers) needs real people: recruit 3–5 users, give them the starter templates and a short task list (make a 4-page newsletter, print a bulletin booklet, mail-merge a letter), and record where they get stuck. The UI polish backlog depends on what they find. The kit is ready: docs/usability-test-plan.md (screener, script, 7 task cards, observation sheet, SUS, consent).

(B-001 resolved 2026-10-08: the dashboard is published as a claude.ai page at the user's request, and GitHub Pages also deploys from CI.)

## Log

### 2026-10-08 17:40 — Phase 0: foundation (lead)
- Merged: Cargo workspace (core, layout, render, io-pdf, io-native, io-pub, engine, app, journeys); document model; `Command` layer with snapshot undo; own text layout engine (rustybuzz shaping, UAX#14 breaking, hyphenation, columns, linked-frame flow, wrap exclusions, justification, tabs); tiny-skia raster and krilla PDF back ends sharing one display list; `.npub` zip format; engine `Session` (actions + queries); headless journey runner with PDF (lopdf + hayro), PNG (pixel, ink, golden), and round-trip checks.
- Journeys written before dispatch: J-PG-001..003, J-TF-001..004, J-TY-001..004, J-SH-001, J-IM-001..002, J-LY-001, J-PR-001, J-EX-001, J-FI-001, J-UR-001. All 19 pass on Linux locally; CI on all three OSes is pending.
- Decisions: egui/eframe for UI (AccessKit + egui_kittest give headless UI journeys); our own layout engine instead of parley/cosmic-text, because Publisher needs per-line widths and frame chains; snapshot undo instead of inverse commands; journeys load only bundled OFL fonts so output is deterministic; `newpub-engine` crate added so the app and the runner share one dispatch path.
- Found by journeys while building: narrow wrap pieces force-broke words (fixed: only full-width lines may break inside a word); letterboxed images smeared edge pixels (fixed: clip to the visible image rect); PDF text extraction inserts spaces between runs (runner text matching is whitespace-tolerant).
- Risk noted: krilla 0.8 has no PDF/X validator, so EX-03 (PDF/X) needs an extension or post-processing.

### 2026-10-08 18:05 — Predecessor located (lead)
- User: the existing repo is birchamp/newspub. Beyond the initial-commit `main` it has two branches: `master` (design spec, implementation plan, Electron + React + Vite scaffold, model types) and `claude/ui-testing-responsiveness-smep9u` (a fuller TypeScript app: images, typography, snapping, arrange, thumbnails, shared undo). Phase 0 started from `main` alone and missed these.
- Decision: survey the predecessor's document model and UX now, adopt its decisions where they don't conflict with Publisher parity, and record each adoption in ARCHITECTURE.md §0.
- B-002 resolved: GitHub Pages was already enabled; the CI deploy-pages step succeeded.

### 2026-10-08 18:20 — Predecessor surveyed; decisions adopted (lead)
- An Explore agent surveyed the NewsPub TypeScript app (model, UX, shortcuts, snapping, templates, pretext layout). Results and adopt/not-adopt decisions are in ARCHITECTURE.md §12.
- New parity items: FI-05 (open predecessor `.newspub` files), UI-07 (carried-over shortcuts), UI-08 (template picker at startup). Native files use the `.newspub` extension.

### 2026-10-08 18:40 — Batch 1 merged (8 tasks, 14 items)
- Merged: LY-03 lock enforcement (Haiku), PG-05 page-margin/spread queries (Haiku), FI-02 fonts/assets queries (Haiku), EX-02 PNG/JPEG export (Haiku), SH-04 align/distribute (Haiku), SH-03 arrowheads, caps, joins (Haiku), FR-01/02 find, replace, replace all (Sonnet), AX-01/02 + IM-05 accessibility checker (Sonnet), TY-06/07/10 tab leaders, lists, drop caps in the layout engine (Sonnet).
- Every task passed Sonnet review on its first attempt; no escalations.
- Merge conflicts in engine/lib.rs and core/command.rs (several agents split the same placeholder match arms) were resolved by the lead. All 44 Batch 1-era journeys pass locally; 45/97 overall, and the other 52 belong to items not yet dispatched.
- Process issues found:
  - Workflow worktrees start at the repo's initial commit, so agents had to find the base commit themselves (EX-02 stacked on PG-05).
  - Placeholder "not implemented" arms shared by several tasks cause predictable merge conflicts.
- Fix for Batch 2 specs:
  - Name the exact base commit in each spec.
  - Give every task its own placeholder arm.
- Items are checked off in PARITY.md only once CI is green on all three OSes for this merge.

### 2026-10-08 19:50 — Lead layout work during Batch 2
- Done by the lead: fields and sections (PG-06, PG-09); continued notices (TF-09); autofit fixups (TF-08); tables (TB-01..TB-04); vertical text and rotated frames (TF-10); text inside shapes (SH-06); glyph queries (TY-13 ligatures, TY-16 special characters); keep-with-next / keep-together / widow-orphan control with paragraph rollback (TY-15); baseline grid (TY-14); bidi shaping and visual reordering (TY-17).
- Known gap (TY-17): the PDF text layer of right-to-left runs is in visual order, so copy/paste reverses Hebrew/Arabic. Fix: emit /ActualText spans (krilla marked content) — follow-up item.
- Journey corrections (lead's own arithmetic, not behaviour changes): TF-007/TF-008 story lengths now queried; TF-009 frame too short for 24 pt; TY-013 frame b moved so its first baseline can reach the grid; SH-006 shape text wraps; TY-016 char positions compared relatively.
- Decision: widow/orphan control is on by default (Publisher default); every earlier journey still passes.

### 2026-10-08 20:20 — Mail merge (lead)
- Done by the lead: MM-01..MM-04.
  - Engine: CSV data source (`attach_data_source`), merge fields shown as «name» or as preview values, recipient filter and sort, skip-blank-lines, picture fields resolved relative to the CSV's folder.
  - Output: `merge_to_pdf` writes one copy of the publication per record, with images deduplicated. `merge_to_publication` repeats pages per record and detaches the data source.
- Interface changes (ARCHITECTURE §11):
  - core: `merge` module (`substitute_story`, `Document::merged`, `Document::merge_publication`), `MergeData`.
  - engine: Session `view` doc (the preview substitution feeds layout, render and PDF).
- Bug fixed: text typed after a field no longer inherits the field marker (`Story::insert`).
- Local suite: 62/97.

### 2026-10-08 21:00 — Batch 2 merged (12 tasks)
- Approved by Sonnet review on the first attempt (10 tasks):
  - GUIDES (GD-01..05, Sonnet); TEMPLATES (BB-01/02, Sonnet); SPELL (SP-01..03, Sonnet);
  - PICTURES (IM-04/08, PG-10, Haiku); PDF (EX-03/05, PR-0x n-up, links/bookmarks); HTML (EX-04);
  - PUB (PI-01..04, .pub import); MISC (TF-11 text/.docx import, FI-03 autosave); UI (selection handles, nudging, panels, recent files, shortcuts); LEGACY (FI-05 .newspub).
- Not approved, then fixed by the lead:
  - LAYERS (LY-02). Escalation: Haiku failed twice and moved to Sonnet, which also failed twice. Every failure was the same blocker: the Object query omitted `layer` when it was None (serde skip), so the fix lay outside the task's files. Lead decision: the Object query reports `"layer": null`; the file format is unchanged.
  - EFFECTS (SH-05/07, IM-06/07). Blocked by the lead's ShapeKinds stub, which was already on the lead branch, and by a wrong J-IM-006 probe: [216,180] sits on the yellow box's corner. The probe moved to [288,144], image pixel (300,100), which is blue.
- ESCALATION: LAYERS (LY-02). Haiku failed twice, so it moved to Sonnet; Sonnet failed twice; the lead resolved it. Root cause was the out-of-scope Object query field.
- Lead merge work:
  - render `push_object` merged as page-aware plus the screen flag;
  - Table arms in io-html and pdfq;
  - `Story.typing_attrs` in the legacy importer;
  - new core `DuplicateObjects` command; the app's Cmd+D no longer rebuilds objects through individual commands, so groups, tables and shape text now duplicate;
  - a clippy allow in the templates builder.
- Journey corrections (lead's arithmetic): J-TF-010 deletes 112 chars, not 113; J-IM-006 probe as above.
- Disk: the session disk filled up (12 GB target plus agent worktrees) and caused a rustc ICE. Freed by dropping target/debug; agents now share one target dir that the lead clears after each batch.
- Local: 93/97. Remaining P0–P2: GD-04/UI-06 (rulers, zoom, spread view: UI-GD-001 to be written), PR-07 (print: UI-PR-001), AX-04 (keyboard/AccessKit: UI-AX-001), AX-03 (tagged PDF), BB-03/04 (building blocks, schemes), UI-08 (template picker).

### 2026-10-08 21:50 — Lead: BB-04 schemes, AX-03 tagged PDF, Batch 3 dispatched
- BB-04, done by the lead because it touches core colour and font resolution:
  - `Color::Scheme` slots; 7 original colour schemes and 6 font schemes on bundled fonts;
  - `resolve_char` resolves "+major"/"+minor" and scheme text colours; the engine's display document resolves object, page and table scheme colours, used for render, PDF, HTML and merge.
- AX-03 tagged PDF / PDF/UA-1:
  - Structure markers are added to the shared display list: P, or H1–H6 from "Heading N"/"Title" paragraph styles, with the heading text as title; Figure with alt text for pictures and described shapes; Table/TR/TD for cells.
  - The PDF exporter builds krilla's tag tree in each page's reading order. Undescribed, decorative, master-page and crop-mark content is drawn as artifacts. Link annotations are tagged with alt text.
  - krilla's UA-1 validator runs on export. When there are no bookmarks, the outline comes from the headings, or from the pages if there are no headings.
- Batch 3 dispatched (all Sonnet, base 4a2fcc8): BLOCKS (BB-03, on the new core `Fragment` API), PICKPRINT (UI-08, PR-07), VIEWAX (GD-04, UI-06, AX-04).
- Journey fix (lead): UI-AX-001 tool name is "TextBox" (the app's `Tool` debug name), not "Text".
- Local: 95/100.

### 2026-10-08 22:05 — First PARITY check-off (CI addfc94)
- CI run 37839317947 at commit addfc94: clippy and build are green on Linux, macOS and Windows. Journeys pass 93/97 on each OS, and the failing set is identical everywhere: J-AX-003, J-BB-003, J-BB-004, UI-SH-007, all unimplemented at that commit.
- 105 of 128 PARITY items are checked off: those whose listed journeys all passed on every OS.
- Unchecked:
  - Since fixed by the lead and awaiting CI: BB-04, AX-03.
  - In Batch 3: BB-03, UI-08, PR-07, GD-04, UI-06, AX-04.
  - P3 items without implementations: 15.

### 2026-10-08 22:30 — Batch 3 merged (3 tasks, 6 items)
- All three tasks were approved by Sonnet review on the first attempt:
  - BLOCKS (BB-03): a JSON user library with an assets folder, and 7 original built-in blocks;
  - PICKPRINT (UI-08, PR-07): the startup and New template picker, replacing the old New dialog; a print dialog with printer, copies and page range; OS hand-off through `lp` or PowerShell, or a spool folder in journeys;
  - VIEWAX (GD-04, UI-06, AX-04): rulers, a units menu, actual size and fit, wheel scrolling, two-page spread, a keyboard-focusable canvas with Enter to insert a frame, and screen-reader names for objects.
- Merge conflict in app/lib.rs (fields, `view_state`, and `ui()` hooks from both UI tasks) resolved by the lead. VIEWAX made `view_state` report the TextBox tool as "Text" to fit the original journey wording; the merge keeps the uniform debug name "TextBox", which matches the corrected UI-AX-001.
- Local: 100/100 journeys pass, clippy and fmt clean. All P0–P2 items are implemented; check-off waits for CI on all three OSes.
- Untested: the real OS print hand-off (CI has no printer).

### 2026-10-08 23:10 — Stop condition reached: all P0–P2 items pass on all three OSes
- CI run 37842573100 at commit bb529dc: clippy, build and 100/100 journeys pass on Linux, macOS and Windows.
- Checked off: BB-03, BB-04, AX-03, UI-08, PR-07, GD-04, UI-06, AX-04. PARITY now stands at 113/128; P0–P2 at 110/110.
- B-001 closed: the dashboard is the claude.ai page, with GitHub Pages as a CI mirror.
- Final report: REPORT.md (parity by area, known gaps, top risks, P3 recommendations).

### 2026-10-08 22:40 — P3 work started (user: "go ahead with the P3 items")
- Journeys were written first for all 15 P3 items: J-TB-005, J-FR-002, J-IM-008, J-IM-009, UI-LY-001, J-PG-008, J-TY-017, J-TY-018, J-SH-008, J-BB-005, J-MM-004, J-MM-005, J-PR-005, J-PR-006 and J-EX-006.
- New fixtures, all generated and original: halves.gif/.bmp/.tif, badge.svg, members.xlsx, products.csv.
- Runner checks added: `expect_files` and `expect_zip`.
- PARITY: IM-09 is narrowed to SVG, TIFF, GIF and BMP. EMF/WMF becomes a new IM-11 (P3, research: there is no maintained Rust parser; it needs a vector converter). Doing so keeps IM-09 from looking done while EMF/WMF is still missing.
- Interfaces, all lead decisions recorded in ARCHITECTURE §11:
  - text effects through `ResolvedChar` and `GlyphRun`;
  - `Object.hidden` and `Object.overprint`;
  - `ObjectKind::WordArt` and `ShapeKind::Bezier`, with the Bézier geometry rendered by the lead;
  - document sheet layout, business information, catalog areas;
  - `Imposition::DocumentSheet` and `PdfOptions.separations`;
  - engine actions and queries, and the picture-decoding hook.
- Done by the lead (journeys pass locally): MM-05 catalog merge, MM-06 Excel data source (calamine, MIT), BB-05 business information sets.
- Batch 4 dispatched on base f5fa56d (103/115 passing): TABLEPASTE (Haiku), FINDFMT, IMAGES, OBJECTS, PRODUCTS, TEXTART, FREEFORM, SEPARATIONS (Sonnet), PACKGO (Haiku), EXPORT (Sonnet).

### 2026-10-08 23:45 — Batch 4 (P3) merged; IM-11 done by the lead
- All 10 tasks were approved by Sonnet review on the first attempt:
  - TABLEPASTE (TB-05) and PACKGO (PR-09), on Haiku;
  - FINDFMT (FR-03), IMAGES (IM-09), OBJECTS (IM-10, LY-04), PRODUCTS (PG-11), TEXTART (TY-18, TY-19), FREEFORM (SH-08), SEPARATIONS (PR-08) and EXPORT (EX-06: EPUB and XPS), on Sonnet.
- IM-11 (lead): a new `newpub-io-metafile` crate converts EMF and WMF to SVG from the published record layouts, and `decode_picture` stores the result as an SVG picture.
  - It covers shapes, polygons, polylines, Béziers, paths, pens and brushes, text, mapping modes and world transforms.
  - Bitmaps, clipping and gradients inside metafiles are not converted.
  - The fixtures come from `tools/fixtures/make_metafiles.py`, and PIL parses both as valid metafiles.
- Lead fixes at merge:
  - the Object query reports `hidden` and `overprint` (LY-04);
  - `io-native::open` resolves relative picture links against the file's folder (Pack and Go);
  - the runner treats a missing path as null for `is_null`;
  - J-PG-008's `page_size` now uses the `{w, h}` form (my format error);
  - the display list receives the `FontStore`, so WordArt uses the publication's fonts and not just the bundled ones (TEXTART's LEAD-NEEDED);
  - conflicts resolved: render raster (SVG scaling plus TEXTART's refactor), io-pdf Cargo.toml and `Ctx.svgs`, and the workspace members (io-metafile and io-xps).
- Accepted agent notes:
  - IMAGES re-encodes GIF/BMP/TIFF as PNG with a "Source" text chunk recording the original format.
  - The XPS output has not been opened in a real XPS viewer, only checked as well-formed XML.
  - SEPARATIONS ignores overprint on group children.
- Local result: 116/116 journeys pass, and clippy and fmt are clean.

### 2026-10-08 23:55 — All parity items green on all three OSes
- CI run 37854377656 at commit 6c64ad8: clippy, build and 116/116 journeys pass on Linux, macOS and Windows.
- Checked off 16 P3 items: TB-05, FR-03, IM-09, IM-10, IM-11, LY-04, PG-11, TY-18, TY-19, SH-08, BB-05, MM-05, MM-06, PR-08, PR-09, EX-06.
- PARITY now stands at 129/129.
- REPORT.md is rewritten with the gaps, risks and next steps that remain.

### 2026-10-09 00:30 — Next steps (user: "go ahead with the recommended next steps")
- RTL copy and paste fixed: every PDF export is now tagged, not only PDF/UA. Right-to-left runs are drawn in visual order inside marked content whose /ActualText holds the logical text.
  - J-TY-016 asserts the Hebrew in logical order through the new pdfcheck `actual_text_contains`.
  - New J-EX-007 asserts that a plain export carries structure, alt text, language and title.
- Performance (new PARITY area PF, item PF-01; J-PF-001 with a new `timed` runner step):
  - Before: a 20-page, ~100 KB story took 8 s to lay out after each keystroke. `Story::slice` scans from the start, which made layout quadratic in story length.
  - Fixes: per-pass offset tables for slicing in layout; a shaping cache keyed by face, text and options, so unchanged paragraphs are not reshaped; a glyph-coverage cache.
  - After: about 45 ms per edit and 550 ms to autoflow 21 pages. The journey limits are about 4× the local times.
- SH-08 scope widened to include the on-canvas freeform tool and point handles (UI-SH-008), so SH-08 is unchecked again until that work lands. A Sonnet agent is on it.
- Usability testing with real target users needs people, so it is a blocker for a human (B-003).
- SH-08 UI (Sonnet, approved by the lead's review): Freeform tool (click points, Enter or click the first point to finish) and on-canvas point editing ("Edit Points": Point n handles, drag, smooth, corner, add, delete). The journey's JSON path was my own error (`/kind/kind/bezier/closed`).
- CI conformance job (Linux): veraPDF PDF/UA-1 on J-AX-003 and J-EX-007 output, epubcheck 5.1.0 on the EPUB, libgxps `xpstopdf` rendering of the XPS plus a text check, and a real `lp` print to a CUPS cups-pdf printer (conformance journey UI-PR-101 with NEWPUB_REAL_PRINT=1).
  - PDF/X-4 has no free validator, so it stays unvalidated externally (noted in REPORT.md).

### 2026-10-09 00:55 — Conformance green; next steps done
- CI run 37860525724 (commit 545d57d) is green: 120/120 journeys on all three OSes, plus the conformance job.
  - veraPDF PDF/UA-1 passes on J-AX-003 and the J-EX-007 UA export.
  - epubcheck 5.1.0 passes on the EPUB.
  - libgxps renders the XPS with its text intact.
  - A real `lp` print to the CUPS cups-pdf printer arrives with its text.
- My first conformance run also validated a plain, non-UA export as UA-1, which was a wrong expectation. veraPDF now runs only on files that claim PDF/UA and logs the failed rules.
- PARITY: 130/130 (PF-01 and SH-08 checked from CI 41e16cb).
- REPORT.md updated. Remaining work needs people (B-003 usability testing) or real Windows/macOS printers and viewers.

### 2026-10-09 02:10 — Remaining next steps (user: "go ahead with the remaining next steps")
- Polyline point editing (J-SH-009), TH with column scope in tagged tables, and PDF/UA exports of every built-in template validated by veraPDF in CI (J-AX-004). Committed in bc1876a.
- Incremental layout (PF-01, new J-PF-002; design in ARCHITECTURE §5):
  - Each story resumes at its first changed paragraph and stops once it rejoins its previous flow, reusing the saved lines shifted to their new offsets.
  - On a 60-page story a keystroke costs about 7–13 ms, against about 150 ms for a full layout. Autoflow of 60 pages takes about 230 ms, adding pages in batches.
  - J-PF-002 compares the edited layout's fingerprint with a fresh layout after save and open, and checks that an undone format change returns the same fingerprint.
  - The whole suite also passes with `NEWPUB_VERIFY_INCREMENTAL=1`, which checks every incremental layout against a full one.
  - PF-01 is unchecked until CI is green with J-PF-002.
- Windows printing (Sonnet agent, reviewed and merged): native GDI printing replaces PowerShell PrintTo, and EnumPrintersW lists the printers.
- New CI job `native-print`:
  - Windows: a real GDI print to "Microsoft Print to PDF" through the test hook `NEWPUB_PRINT_OUTPUT`, and the journey XPS opened by Windows' own XPS reader (page count).
  - macOS: a real `lp` print to a CUPS file-device queue, with the text checked by pdftotext.
- Usability test kit: docs/usability-test-plan.md. B-003 stays open because it needs people.

### 2026-10-09 01:45 — CI fully green, including real printing on all three OSes
- CI run 37864129775 (commit 1d082f4): 122/122 journeys on all three OSes. SH-08, AX-03 and PF-01 are checked again, so PARITY is 130/130.
- CI run 37870194049 (commit 6c25bc8) is fully green, with the native-print jobs on Windows and macOS.
- It took three fixes to get a macOS print queue in CI:
  - `cupsctl FileDevice` is refused;
  - raw queues are unsupported;
  - file-device URIs stay disabled.
- The queue that works uses the generic PostScript driver and a socket queue to a local `nc` listener.
- macOS's PostScript conversion re-encodes subset fonts, so the printed text is checked as a consistent one-to-one re-encoding of "Printed by newpub" (tools/conformance/text_or_reencoded.py).
- Open: the user asked about a Mac installer (.pkg). Proposed an unsigned universal .pkg CI job (signing needs an Apple Developer ID). Waiting for their answer.

### 2026-10-09 15:40 — Installers (user: "make easy installs for all 3 OSs")
- New workflow `.github/workflows/package.yml`, with scripts in `tools/package/`:
  - **Windows:** an Inno Setup installer that installs per user (no admin prompt), with a Start menu entry, an optional desktop icon, the `.npub` association and an uninstaller. Also a portable zip.
  - **macOS:** a universal (arm64 + x86_64), ad-hoc-signed `newpub.app` in a drag-to-Applications `.dmg`, and a `.pkg`.
  - **Linux:** an AppImage, a `.deb` (desktop entry, icon, `.npub` MIME type) and a `.tar.gz`.
- Each job installs what it built and checks that the app starts and is still running after 10 s (Linux under Xvfb).
- A `v*` tag publishes everything as a GitHub Release.
- The app has an original icon (tools/package/make_icon.py), shown as its window icon. Release builds on Windows no longer open a console window. A file passed on the command line (a double-clicked `.npub`) opens instead of the template picker.
- INSTALL.md explains installing, and the one-time "unidentified developer" prompts on Windows and macOS. Signing needs a Windows code-signing certificate and an Apple Developer ID.
- Known gap: on macOS, double-clicking a `.npub` file starts newpub but does not open that file yet, because macOS passes it as an Apple Event, not as an argument.
- First Package run: macOS was green. Linux and Windows failed the launch check:
  - **Linux** (a real packaging bug): the `.deb` lacked `libxkbcommon-x11-0`, so newpub panicked at start on systems without it. Reproduced locally and fixed in the dependency list.
  - **Windows:** newpub exited at once on the runner, which has no OpenGL 2. Startup now tries OpenGL first, then wgpu (Direct3D 12 with WARP on Windows), and shows an error box if both fail. The CI check now requires a window titled "newpub", so an error box cannot pass.
- Second Package run (fdfc7b5): all three jobs green; the Windows runner (no OpenGL 2) opened the newpub window through the wgpu fallback. CI run 37957271073 is fully green on the same code.
- **Released v0.1.0** (user: "yes, tag v0.1.0 when it's green"): https://github.com/birchamp/newspub/releases/tag/v0.1.0
  - Seven assets: the Windows setup and portable zip, the macOS .dmg and .pkg, and the Linux AppImage, .deb and .tar.gz.
  - This session's git proxy does not allow tag pushes, so the Package workflow gained a `release_tag` input. A manual run with it creates the tag on that run's commit (21b08f9: the workflow change only, with the same app code as fdfc7b5) and publishes the release.

### 2026-10-09 — UI redesign (user: "The UI design needs a massive improvement")
- New design system: Venice Blue to seafoam gradient (from the colorion palettes the user pointed to), Inter for text, Phosphor icons, and light and dark themes. Dark mode is remembered between launches.
- New shell: a gradient header (file actions, Print, Export As, Export PDF), a ribbon with six tabs, a page thumbnail strip and a status bar with zoom, units and spread controls.
- The canvas has a pasteboard, a page shadow, 8 round resize handles, hover outlines, dashed boundaries, empty-frame placeholders and overflow badges.
- Gaps closed by wiring up engine features the UI could not reach before (four Sonnet agents, reviewed and merged by the lead):
  - **Insert:** tables, shape gallery, WordArt, building blocks, fields, captions, hyperlinks and symbols.
  - **Page Design:** page setup, colour and font schemes, background, masters, guides and baseline grid.
  - **Mailings:** recipients, merge fields, preview, filter and sort, merge to PDF and to a publication.
  - **Review:** spelling with suggestions, find and replace (Find Next selects the match), and the accessibility checker.
  - **Export As:** one dialog for all ten export formats, with native Browse buttons (rfd).
  - **Format panel:** text, paragraph, text box, shape, picture, position and arrange cards. It acts on the text selection while editing.
- Real text editing (UI-14): a caret, click to place, arrows, word and line moves, Shift selection, drag selection, typing over a selection, Cmd+B/I/U on the selection.
- Clipboard (UI-15): cut, copy and paste of text (through the OS clipboard) and objects (through a new engine clipboard), plus a right-click menu.
- New start screen: template cards with rendered previews, blank sizes and publication types.
- Engine: a real Date field (`Field::Date`).
- Journeys: UI-IN-001, UI-PD-001, UI-ML-001, UI-RV-001, UI-IS-001, UI-EX-001, UI-TE-001, UI-CB-001 and J-PG-009 (131/131 on Linux). PARITY UI-09..UI-15 get checked once CI is green on all three OSes.
- Screenshots in docs/screenshots, shown in the README.
- Known gaps: Find Next is the only find that selects text in place; the Format panel shows the first character's attributes for a mixed selection.
- macOS: documents opened from Finder (double-click, Open With, dragging onto the Dock icon) now open in newpub. newpub builds winit's event loop itself on macOS and adds `application:openURLs:` to winit's delegate. The Package workflow proves it: `open -a newpub.app finder-test.npub`, then the file is in newpub's recent list (Package run 37979489501).
- Also: Cmd+F opens Find and Replace; dialogs are centred and Escape cancels them; the Print dialog matches the other dialogs; the .dmg check retries a busy volume.
- Version 0.2.0.
- **Released v0.2.0** (CI run 37980609786 and Package run 37980609854 green on f7b108e): https://github.com/birchamp/newspub/releases/tag/v0.2.0. It has seven assets: the Windows setup and portable zip, the macOS .dmg and .pkg, and the Linux AppImage, .deb and .tar.gz.

### 2026-10-09 — Audit, real .pub import, and the missing UI paths (lead, no agents)
- Audit: many checked features had engine journeys but no way in from the app (text box linking, autoflow, shape text, table editing, page reordering, guides, styles, templates, properties, Design Checker). The `.pub` importer only made one text box on one page. Those PARITY rows were reopened; UI-16..UI-23 and PI-05..PI-08 were added, each with a UI or import journey.
- User: "Is this app able to open existing .pub files from MS publisher?" Before this work, barely. Now: Publisher 98, 2000, 2002/2003 and 2007/2010+ files open with their pages, text boxes and linked chains, character and paragraph formatting, pictures (including WMF clip art), shapes, groups and tables. Checked against Apache POI's sample files, with positions measured from LibreOffice's rendering. We keep the Publisher 98 table and rectangle outline, which LibreOffice drops. Not imported: WordArt, embedded fonts, gradient and pattern fills, and Publisher 97.
- User: "stop delegating … just build it." From here on, everything was built by the lead.
- UI-16..UI-23 are built:
  - **Unsaved-changes prompt.**
  - **Drag and drop.**
  - **AutoRecover.**
  - **Text flow:** Insert Text File, Link to Next Box (or click the overflow badge), Break Link, Flow onto New Pages, typing in shapes, Clear Formatting.
  - **Table editing:** cells, Tab and Shift+Tab, Shift-click blocks, merge and split, rows and columns, sizes, table style, cell fill.
  - **Page list right-click menu and ruler guides.**
  - **Styles window.**
  - **Properties, Design Checker, Save as Template and My templates.**
- Bugs found by the journeys and fixed:
  - Tab moved keyboard focus into a panel field, because the canvas never took focus; clicking the page now focuses it.
  - Very high zoom panicked on a page texture larger than the GPU limit; renders are now capped.
  - A quick Shift-click registered as a double-click.
  - WMF pictures with a flipped window showed one corner.
  - Times New Roman rendered sans-serif when the font was missing.
- Runner improvements (harness only; no journeys were edited):
  - widgets are scrolled into view with the mouse wheel;
  - Shift is held for the whole of a shift-click;
  - a control is preferred over a caption with the same name;
  - frames step at 1/30 s, so double-clicks register.
- 145/145 journeys pass on Linux, macOS and Windows (CI run 38001645773, commit 6c857f4). PI-01..PI-08 and UI-16..UI-24 are checked; all 150 PARITY items are now checked.

### 2026-10-09 — v0.3.0 (user: "yes, cut v0.3.0")
- Version 0.3.0. It brings the real `.pub` import and the UI paths UI-16..UI-24 to the installers.
- **Released v0.3.0** (CI run 38003123705 and Package run 38003123649 green on 784886c; release published by Package run 38004300374, started by hand with `release_tag` because this session cannot push tags): https://github.com/birchamp/newspub/releases/tag/v0.3.0. It has seven assets: the Windows setup and portable zip, the macOS .dmg and .pkg, and the Linux AppImage, .deb and .tar.gz.

### 2026-10-10 — Text wrap in the UI (user: "How do you wrap text around another box that's overlaying a text box?")
- The layout engine already wrapped text around any object in front of a text box, but only pictures had wrapping on (by default), and nothing in the app could change it. So shapes and text boxes placed over a story could not push its text aside.
- The Format panel's Arrange card now has **Wrap text** (None, Square, Tight, Top and bottom, Through) and **Distance from text**. They apply to every selected object. Turning wrap on starts from a 0.1 in gap.
- Journey UI-WR-001 covers all four settings with measured line positions: Square starts lines 7.2 pt right of the box, 0.5 in starts them 36 pt right, and Top and bottom resumes the text below the box. PARITY UI-25 checked from CI run 38007907775 (748c8c2), green on Linux, macOS and Windows; 151/151 items.

### 2026-10-10 — v0.3.1 (user: "cut v0.3.1 with the text wrap")
- Version 0.3.1: v0.3.0 plus Wrap text in the Format panel (UI-25). Release notes in docs/release-notes/v0.3.1.md.
- **Released v0.3.1** (CI run 38010606933 and Package run 38010606913 green on 5fe2fef; published by Package run 38011418767 with `release_tag`): https://github.com/birchamp/newspub/releases/tag/v0.3.1. It has all seven assets, and the notes come from docs/release-notes/v0.3.1.md.

### 2026-10-10 — Weekly: text flow feedback in the UI (scheduled orchestrator run)
- Candidates came from the open blockers, REPORT.md's gaps and risks, the usability plan's task cards (T2: put an article into the first story and handle the overflow) and a hands-on pass through the text-flow journeys. The engine already chained stories across boxes, Tab cycled through them and "Link to Next Box" worked, but nothing on screen showed which boxes belong to one story, where the text continues, or that link mode was on; a click on a full box failed with an engine error and moved the selection to the rejected box.
- Chosen (UI-26): with one box of a linked story selected, the canvas numbers every box of the story on the page ("2 of 3"), outlines the others, draws connectors and puts Previous / Next chips on the selected box (Publisher's "Go to Previous / Next Text Box"). The Format panel's Text box card says "Box 2 of 3 in this story, continues on page 4" with Previous / Next Text Box buttons whose tooltips teach Tab and Shift+Tab. Link mode shows a banner with a Cancel button, highlights the empty boxes the story can continue in, stays on when a full box is clicked (with a hint) and ends with Esc from anywhere, including a panel field. The only document change is still the one `LinkFrames` command, so it stays one undo step. Known gap: in two-page spread view the tags, connectors and link targets are drawn on the current page only. New module `crates/app/src/flow_ui.rs`; the `view` query gains `link_mode`, `link_targets`, `status` and `story_nav`.
- Ranked alternatives, not taken this run: continued-on/from notice toggles in the Text box card (engine supports them, no UI); an AccessKit node and tooltip for the overflow badge, and "Text box (2 of 3)" screen-reader names; the Format panel showing mixed attributes for a mixed selection; text-flow entries in the right-click menu.
- Agents: an Explore agent mapped the link-mode, selection and status-bar code paths and the runner steps; a general-purpose agent wrote UI-TX-003 (link mode, cancel, wrong box, link, chain navigation on the canvas and in the panel, across pages after autoflow, undo and redo, save/open round-trip); a review agent checked the diff against ARCHITECTURE.md's conventions and found two real problems (banner clicks fell through to the canvas; the panel's Previous / Next did nothing with a multi-selection) plus dark-mode contrast and master-page edge cases, all fixed before the commit. A second, antagonistic review (user: "Do an antagonistic review") then wrote seven throwaway UI journeys and found three merge blockers, fixed in a follow-up commit: link mode survived New / Open / Delete / Undo and could link the wrong box in a fresh document (ids are reused), so `act` and every document-replacing path now drop the stale mode; the chips sat outside the box and stole clicks from neighbouring objects, so they moved inside the box, are registered only on the canvas and only on boxes at least 64×44 px on screen; and link mode is now modal (Escape ends it from anywhere, no key or drag reaches the document until it ends; a double-click no longer starts editing). Also: "Link to Next Box" is disabled on a box that already continues elsewhere (Publisher greys it out too; linking from a middle box silently inserted), Tab shares `go_to_box` (wrapping), screen readers hear "Text box 2 of 3: …", and link targets use a green readable on the white page in both themes. The lead planned, implemented, merged the findings and committed.
- Baseline before the change: fmt clean, clippy clean, tests pass, 146/146 journeys on Linux. After: fmt clean, clippy clean, tests pass, 147/147 journeys on Linux. PARITY UI-26 checked from CI run 38091503153 (commit 0e52b5b) on pull request #1: 147/147 journeys green on Linux, macOS and Windows, plus the conformance and native-print jobs. Also found this run: the repository is now `birchamp/newpub` (renamed from `newspub`); the REST API must use the new name because the proxy blocks GitHub's redirect.

### 2026-10-10 — Agent access (user: "the ability for an ai agent to use the app … fully accessible by an AI agent")
- The engine already was the interface: every user action is an `Action` and every question a `Query`, both JSON, and the journeys drive the program only through them. So agent access is a protocol in front of that, not a second feature set.
- New crate `newpub-agent` (AG-01): an MCP server over stdio with ten tools (`newpub_reference`, `newpub_status`, `newpub_action`, `newpub_actions` as one undo step, `newpub_query`, `newpub_render_page` returning the page as an image, `newpub_new`, `newpub_open`, `newpub_save`, `newpub_export_pdf`) and a command line (`exec`, `query`, `render`, `status`, `reference`). Tool failures are results the agent can read, with the reference search to run. docs/agent-reference.md (211 actions and queries with their fields, patch structs expanded one level) is generated from the Rust sources by tools/agent/reference.py, embedded in the binary, served as an MCP resource, and checked in CI.
- The app hosts the same server (AG-02): `newpub --agent` reads MCP from stdin on a thread, the UI answers each frame, keeps the page and selection valid, ends text editing the person was doing and shows "Agent: insert_text" in the status bar. Person and agent work on the same publication; the agent's batches are single undo steps for the person's Undo button.
- Journeys: J-AG-001 (reference, status with overflow warnings, a batch as one undo step, refused and partly-run batches, rendering, save, PDF/UA export, reopen, round-trip) and UI-AG-001 (the app as host); runner step `agent: {tool, args, path, check, as}`. The crate's `tests/stdio.rs` drives the real binary over a pipe (CI runs it on all three OSes). Decision: the MCP framing is hand-written (initialize, ping, tools/list, tools/call, resources/list, resources/read; notifications ignored) rather than a dependency, since it is a few dozen lines and the pinned toolchain must build everywhere.
- docs/agent.md: connecting Claude Code and Claude Desktop, the tools, a worked session, live mode, the command line.
- PARITY AG-01 and AG-02 checked from CI run 38097596925 (commit 37ff93a) on pull request #2: 149/149 journeys green on Linux, macOS and Windows (with UI-26 merged in), plus the conformance and native-print jobs and the Package workflow on all three OSes. The agent-test CI step now reports its output as an annotation on failure, because this session's proxy cannot fetch job logs.
- An antagonistic review (same treatment as UI-26) found five high-severity defects, fixed in a follow-up commit: an agent's `newpub_new`/`newpub_open` in the live app replaced the person's unsaved work with no prompt and let AutoRecover delete its copy (now refused while the publication is dirty, through a `Host::before_replace` hook; the app also resets page, selection, thumbnails and the picker after a replacement); `structuredContent` was a bare scalar or list for most queries, which Claude Code's SDK rejects (always an object now, `{"result": …}` when needed); a grouped batch could contain `undo`/`begin_group`/`new_document` and leave the session inside an undo group or report failure after running (refused up front; `undo_group: false` allows the replacing ones); the documented `--save` example swallowed the next action as its path (`--save=path` now requires the `=`); and the installers did not ship `newpub-agent` (they do now). Also: `newpub_status` nests group members and flags hidden objects and master pages; bad `page`/`dpi` arguments are refused instead of coerced; JSON-RPC batches, unknown tools (-32602) and unknown resources (-32002) follow the spec; the app notices a disconnected client ("Agent disconnected") and keeps the window; the generator no longer drops `Name {}` variants and adds a Types appendix with every enum's JSON values and the Color forms, with a self-check on the variant count; unit tests cover the reader thread, protocol errors and batch rules.

### 2026-10-17 — Weekly: continued notices in the Format panel (scheduled orchestrator run)
- Candidates came from the alternatives left by the 2026-10-10 run, REPORT.md's risks, the usability plan's task T2 ("put this article into the first story … overflow is handled (continued or autoflow)") and a pass through the text-flow journeys and the Text box card. The layout engine has drawn "(Continued on page N)" and "(Continued from page N)" since TF-09 (J-TF-008), but nothing in the app could turn them on: the only way was the agent or a journey. Ranked alternatives, not taken this run: the Format panel showing mixed attributes for a mixed selection (needs a new range query; more than one session); UI-26's chain tags and link targets on both pages of a spread; an AccessKit node and tooltip for the overflow badge, and text-flow entries in the right-click menu.
- Chosen (UI-27): the Text box card gets Publisher's two checkboxes, "Include 'Continued on page…'" and "Include 'Continued from page…'", under "Box n of m in this story". The tooltip shows the notice with the real page label and says it is generated, takes one line of the box and keeps its page number up to date. They are greyed out, with the reason, on a box that is not linked, on the first box (no "from"), on the last box (no "on") and on master pages; a flag that is already on stays clickable so it can be cleared. Several selected text boxes change together as one undo step (BeginGroup/EndGroup, as z-order does) and the checkbox shows a mixed state when they disagree. One `SetTextFrame` per box; no core, layout, engine or query change, and the `.npub` format is untouched. Known limit, as in the layout: the notice is drawn only when the neighbouring box is on another page; a same-page chain reserves the line but shows nothing.
- Agents: an Explore agent mapped the Text box card, the checkbox pattern (the Picture card's "Decorative"), the chain helpers from UI-26, the runner's steps and the queries that see the notice; a Plan agent designed the placement, the disabled rules, the multi-selection behaviour and the journey's assertions; a general-purpose agent wrote UI-TX-004 (disabled no-ops on an unlinked box, the notice after Flow onto New Pages measured through `frame_text` and `frame_lines` on the first and second box, the greyed-out "from" on the first box and "on" on the last, undo and redo, two boxes selected with Cmd+A changed as one undo step, save → New → Open through the dialogs, the notices in the exported PDF, and the engine round trip); a review agent checked the diff against ARCHITECTURE.md's conventions and the UI-26 antagonistic findings. It found no blockers and five points, all taken: the tooltip promised a notice for a neighbour on the same page (it now says nothing is shown yet and why), a flag kept enabled only so it can be cleared showed a preview with a literal "N" (it now says why no notice is shown), two toggles in one frame became two undo steps (grouped), "an earlier page" should be "another page" (chains can run backwards), and a mixed selection now resolves to on. The lead planned, implemented, merged the findings and committed.
- Found on the way, not fixed this run: the canvas has no Shift-click to add to the selection (a plain click replaces it; only Select All makes a multiple selection on the canvas), although the inspector's hint says it does; the runner can scroll the page list down but not back up. The `long-text.txt` fixture flows onto 10 pages from a 6.5 × 3 in box.
- Base branch: `claude/agent-access` (the most recent branch; it already contains UI-26 and the agent work from pull requests #1 and #2). Baseline before the change: fmt clean, clippy clean, tests pass, 149/149 journeys on Linux. After: fmt clean, clippy clean, tests pass, 150/150 journeys on Linux. PARITY UI-27 gets checked once CI is green on all three OSes.
