# Graticule: spec

The source of truth for what to build: Graticule (model DK-01), a framework for native desktop apps in Rust, its units and the demo apps that prove it. Working rules are in [`AGENTS.md`](../AGENTS.md); the same ideas in plain words are in [`guide/`](guide/README.md). The build plan and its status are in §17.

Every decision below is the owner's unless marked **Proposed** (engineering choice, unconfirmed) or **Open** (§15). Visual details (exact spacing, weights, colors, glyphs) are designed with the owner component by component in Specimen; this document records principles and decisions.

## 0. Ground rules

- One Rust codebase: the units, the services and the demo apps, in one cargo workspace. Changing the kernel rebuilds every app in the same step, and a breaking change fixes every app in the same change.
- Graticule is made of **units** (§13), each with a model code. Every unit is a library that apps build in and a face of its own that you can run, an app or a command-line tool, so every unit also proves Graticule. Where a unit's core doesn't need a screen, it doesn't depend on the kernel.
- A clean start. Graticule began inside Feedreader; its architecture is rewritten here, and the parts that meet the operating system are moved in one at a time with their laws and the traps they avoid (`docs/internals/`), not rewritten. Feedreader is paused and later depends on Graticule like any other app.
- Desktop first: macOS, then Linux once macOS works, then Windows. The API never uses a concept only macOS has, so each new platform is work, not a rewrite. Mobile comes later.
- No `unsafe` in our code; the workspace denies it, with two approved exceptions (`docs/internals/unsafe.md`): the native context menu on macOS, and the macOS spellchecker if no safe binding reaches it. Every other exception is justified to the owner before it lands.
- License: MPL-2.0 (`LICENSE`). Changes to Graticule's own files stay open; apps built on it may use any license.
- Tests are laws (§11). Everything else is verified by running the apps and looking at Specimen.

## 1. Purpose

### 1.1 Goal
A framework for native apps that feel like the best interfaces of the past, at today's speed: easy and fast by default, close to no-code in code. The win condition: the demo apps (§14), from a notes app to a sequencer, an image viewer and a map, are each small in app code, beautiful, and within budget, and a person learning Rust can write one of their screens.

### 1.2 How a framework reaches everything
Electron can build almost anything not because it has every feature but because it has escape hatches all the way down. Graticule does the same: a few primitives that compose, and a ladder of escape hatches (§6.4), each one still on the grid and still in the theme's inks. Completeness is judged along axes, not by listing apps; a framework is complete when it reaches the hard end of every axis, at least through an escape hatch:

| Axis | Easy end | Hard end |
|---|---|---|
| Time | static | hard real-time (an audio callback) |
| Space | fits the screen | infinite and zoomable |
| Data size | dozens | millions, streaming without end |
| Precision | cells | pixels, physical units, 16-bit and HDR color |
| Input | keys and mouse | pen pressure and tilt, MIDI, gamepads, multitouch |
| Authoring | reading | direct manipulation, structural editing |
| People | one window | many windows, devices, live collaboration |
| Trust | our own code | third-party content (web), third-party code (plugins, Later) |
| Output | screen | print and PDF, image export, second screens, audio |
| OS presence | a window | menu bar items, file types, Quick Look, Spotlight, Shortcuts |
| Script | Latin | right-to-left, Chinese, Japanese, Korean, mixed directions |

### 1.3 Non-goals
A browser engine, a 3D or game engine (the GPU escape hatch covers the honest cases), wrappers around native widgets, mobile before the desktop is done, and anything an app gets by the nature of the architecture (voice control, AI agents driving an app: the registry and automation already make them possible).

## 2. Principles
The seven in `AGENTS.md`, in order: the grid is law and inside a box is freedom; faster than any display can show, silent when idle; app code a person can write; facts, commands, a view; everything visible, inner workings included; proven by law; native, open, private.

## 3. The model: facts, commands, a view

### 3.1 What the app writes and what Graticule owns
- The app supplies **facts**, **commands** (the only way facts change), a **view** (the screen described from the facts), **states** (named sets of facts for Specimen) and **effects** as data.
- Graticule owns everything else: the shell (windows, the event loop, input, focus, hover, dragging, scrolling, text input, clipboard, menus, web views, file dialogs, accessibility), the components' own interaction state, the components, styles and themes, layout, painting, the tools and the budgets.
- The view is a pure function of the facts. Painting never writes into the app's facts.

### 3.2 Four kinds of state
Facts hold four kinds of state, marked on each field, and Ledger (§13.2) treats each differently:

| Kind | Mark | What it is | Undo | Sync | Saved |
|---|---|---|---|---|---|
| **document** | none (the default) | what the user is making: items, text, shapes | yes | yes | yes |
| **session** | `#[session]` | where the user is: a half-typed field, the open file, the filter, the selection | typing only (§13.2) | no | on this device |
| **live** | `#[live]` | values written by other threads (§3.5) | no | no | no |
| **resource** | `#[resource]` | large loaded things: a scan, a shader | no | no | by reference |

- The document part is built from Ledger's types (`List`, `Map`, `Text` and plain values, which are registers), so every app is ready to sync. A `Vec` in the document part is a compile error that points at `List`: a list synced as a whole loses one of two concurrent additions without a word.
- State that only a component needs (a scroll offset, a split's width, hover, a field's caret and selected text) is the kernel's, kept by the component's id; a field's id is the path it edits, so a `Text` in the document never carries a caret. State the app reads in `update` is facts. Both are saved in one session snapshot by Ledger; they differ only in who owns them.
- Windows and navigation: the facts are shared by all of an app's windows; each window has its own session state. Places, tabs, back and forward, and windows are building blocks the kernel provides and each app picks from. Places are always visible when an app has them. Back and forward exist; **Proposed:** their history is visible, like a breadcrumb, never hidden state.

### 3.3 Commands
- Commands are an enum with typed arguments. Each has a name and a label; labels can take values from their subject and depend on its state, both versions written next to the command: `#[label(done: "Mark “{title}” not done", else: "Mark “{title}” done")]`. A control inside a row shows a short label, since the row already shows its item: the link says "Remove", while the screen reader, the palette and the menus say "Remove “buy milk”". The short label is written next to the long one: `#[label("Remove “{title}”", short: "Remove")]`. Messages and labels are written in English in the source and translated through ids (§7.5).
- `available(&self, &Cmd) -> Available` answers `Yes` or `No(reason)` before a command runs; a disabled control says why in the status line. The `match` is exhaustive, with no catch-all, so a new command can't become available by accident. The kernel asks only for commands on screen, and for the palette and which-key only while they are open; `available` has a time budget that Assay measures.
- `update(&mut self, Cmd, &mut Cx) -> Outcome` runs only after `available` said `Yes`. It is total: a command that can no longer apply (its item is gone) returns `Outcome::Nothing`, which Ledger doesn't record. Everything non-deterministic (new keys, the time, randomness) comes from `cx` and is logged, so replaying the log gives the same facts.
- **Subjects.** A list row, a table row or a shape declares its key as its subject. A command argument marked `#[subject]` is filled from where the command started: a control in a row takes that row; a context menu takes the row under the pointer, or the whole selection if that row is in it; a keymap sequence, the palette or the menu bar take the selection, or the focused row when nothing else is selected; with none of these the command is unavailable and says so. A command runs over several subjects only if its label says what it is called then (`many: "Remove {count} items"`); it is one transaction and one undo step, subjects that can't take it are skipped, and the message line says which and why. So the same `Remove` works from a link in a row, a keymap sequence, the palette and a context menu. Inside a row the view's builder knows the subject's type, so binding a command that wants a different subject doesn't compile. **Proposed** (`docs/internals/kernel.md` §10).
- **Selection** that the app reads is session state, a `Selection<K>` from Ledger: keys and the focused one. The kernel changes it only by sending an edit on its path, so a list takes `Todo::selection`, not `&self.selection`. Keys of items that no longer exist are pruned whenever the document changes, locally or through sync.
- **Edits.** Typing into a field, changing a selection and moving a slider are commands too, and the only way the kernel changes the facts: the kernel sends `Edit(edit)` over a path into the facts, and the app's `update` handles it, by default with `edit.apply(self)`, so the app can validate a value or keep an invariant (set `edited` when `title` changes) in one place. A gesture (dragging a slider, typing a word) is one transaction and one undo step.
- **Effects** are data, asked for through `cx`: open a dialog, write a file, send a request. Their results come back as commands marked `#[internal]`, which never appear in the palette, menus, keys or undo. Specimen intercepts effects and shows them instead of running them.
- The registry is generated from the commands and feeds the action bar, which-key, the help page, the palette, the menu bar, context menus, automation and the OS adapters (§9).

### 3.4 Keys
- Keys for app commands live in **keymap files**, not in code: chords, sequences (`g g`), counts (`3w`), operator and motion (`d w`) and modes (normal, insert, select), enough to build a modal editor. Edits to a keymap file apply live. Graticule assigns no keys to app commands; the platform's conventions (text editing, copy and paste, Tab between fields, Escape) belong to the system. The demo apps' keymap files are chosen by maintainers for good use, and the owner can change them.
- At runtime the registry builds the tree of what is reachable from here by key; after a prefix, the action bar shows the next branch (which-key).

### 3.5 The tree, the main thread and derived values
- The view builds a tree of components each time; building it is cheap and allocates nothing in steady state (reused flat storage, integer ids). Ids are automatic, from the line of code that creates each component (counted when one line creates several) and from the key of each item in a list, so a component that appears above another doesn't change the other's id.
- The kernel compares each part with last time and reuses what didn't change, builds and lays out only what is visible (a list calls its row code only for the rows on screen) and repaints only the regions that changed. A cursor move repaints two rows.
- Every component records where in the source it was created, in development builds.
- One tree, read by everything: painting, hit testing, the accessibility tree, Specimen, Scope, the taste laws, automation, structural diffs, print.
- The main thread only turns input into commands, updates the facts, builds the tree, paints and submits. It never does work that grows with the data.
- **The fast way is the default way.** The obvious code is fast at any size: a `List` keeps a count of each yes-or-no and choice field of its items, so asking "is anything done?" is instant with fifty million items, and it offers no question that would have to look at every item on the main thread. Changes that touch every item (`retain`, find and replace) run at once on a short list and in the background on a long one, with their progress shown, as one undo step; the app's code is the same either way.
- Anything that grows with the data (filtering, shaping long text, placing labels, decoding images, making an exported file, network, disk) runs as a **job** in the background. A job reads a snapshot of the facts it needs; Ledger's types share their storage, so the snapshot of a long list costs little. While it runs the screen shows what it has and says that more is coming; every running job is visible.
- **Derived values** (`Derived<T>`): a value computed by a job from named fields of the facts, recomputed only when those fields change. Facts keep a version per path, so typing in a field never recomputes a histogram of a scan. The view reads the latest finished value and shows that a new one is coming.
- **Live values** (`Live<T>`): a value written by another thread (an audio callback, a market feed, a download) without locks; the view reads a snapshot once per frame, coalesced to at most one frame per display refresh.

### 3.6 States
- A named state is a set of facts, listed by the app's `states()`. Specimen shows every app's states (§13.5); "send this view" saves the current facts to a file that opens in the same state on another machine.
- Saving, migrations, history and sync belong to Ledger (§13.2).

## 4. The grid and its freedoms

### 4.1 The grid
- The screen is paper ruled in cells. The cell comes from the monospace font: its width is the advance of IBM Plex Mono, its height the line, both whole points, so a cell is whole device pixels at every integer scale and an app keeps its column count when a window moves between a 1x and a 2x display; the font size is derived from the cell, not the other way around.
- Layout is in whole cells and the layout API can only express cells: rows, columns, fixed widths, `Fill`, weights, minimums and maximums, scroll areas and splits the user can resize. Pixel coordinates exist only inside a canvas, a picture or a component's own drawing.
- Narrow and wide are column counts. A layout states what happens when its minimums don't fit (it switches to its narrow form); nothing is cut off silently.
- Proportional text, pictures and canvases live inside boxes; only the box sits on the grid. Mono and Sans share a baseline on a row.
- Character graphics (box drawing, blocks) are drawn by Graticule, so they meet exactly at cell edges.
- Chinese, Japanese and Korean characters are two cells wide in mono, as in a terminal. Right-to-left layouts mirror the grid.

### 4.2 Laws, defaults and freedom
- **Laws**, enforced by types or checked as laws: the frame is on the grid; everything is drawn in semantic ink; every action is in the registry; frames stay within budget; every picture and canvas has a text label.
- **Defaults**, on unless an app visibly opts out: no time-based animation; nothing floats; hints in the status line; notifications off.
- **Freedom**: everything inside a canvas, a picture or a component's own drawing.

### 4.3 Text on a canvas
- Content text (notes on a board, street names on a map, a clip's name on a timeline) lives in the canvas's space and follows its content: it zooms, rotates and runs along a street.
- What keeps it Graticule is typography, not position: canvas text uses the style's type scale, Plex and semantic inks.
- Readouts in screen space (rulers, axis ticks, measurements, snapping distances, a crosshair's values) snap to cells.

### 4.4 Overlays
- Nothing floats by default. An app may use **anchored overlays** (a popover, autocomplete, a color picker, a chart's readout) on four conditions: opened by an explicit action, anchored to what opened it, on the grid with the style's hard shadow, and closed by Escape or by clicking away. They never appear on their own and never cover what is being edited.
- Grounded forms are the default: a tooltip is the status line's hint, a toast the message line, a dialog an inline confirm row, a dropdown a cycling value or a choice that opens inline.

### 4.5 Motion
- No time-based animation by default. Short glows are allowed where they add clarity (a value that just changed), decided per component, always finite and never on what you're reading or dragging.
- After a change you didn't watch happen (an undo, a jump, items that arrived), the affected items carry a mark until your next input.
- Frames come from input, data or resize. Only a live canvas may ask for continuous frames, and only while live.

## 5. Look

### 5.1 Styles and themes
- A **style** is how things are drawn: line treatments, shadows, corners, the glyphs of checkboxes and tabs, density. It never changes where things are: the grid, the spacing rules and the ink roles are the same in every style. Graticule ships a default style; developers make their own, and the taste laws judge every style the same way.
- A **theme** is colors, defined in OKLCH, with contrast checked as a law: text inks reach 4.5:1 on their backgrounds, faint 3:1. Styles and themes are independent and switch at runtime.
- Styles and themes are data files, edited live in Specimen (§13.5).
- Everything drawn uses semantic ink (ink, muted, faint, rule, link, visited, danger, warn, ok; cursor, hover, selection, field; thin, regular or heavy weight), so a style or theme change restyles components, pictures, canvases and GPU paint alike.

### 5.2 Typed values
Each kind of value has one look, used everywhere: text, count, date, version, state, product code, code, mode, error. A badge labels what something is, never what to do. Dates are moments formatted one way everywhere, absolute or relative by one setting.

### 5.3 Design language
- **Principles.** Expose state and inner workings. Dense and calm. Everything visible, flat not hierarchical. Explicit over implicit, verbose over opaque. One grammar: rules, boxes, key labels, typed values, the grid. Performance is design. Timeless, not trendy; don't infantilize the user.
- **References.** U.S. Graphics Company (usgraphics.com) is the main reference: typed values, what you can act on is boxed with a hard shadow, the current place marked with a small dot, the sitemap as a drawn tree, the engineering page as a spec sheet, two typefaces with one split (proportional for prose and labels, mono for data). Alongside it the interfaces of the 1970s and 80s (Bloomberg Terminal, Lotus 1-2-3, Xerox Star, Symbolics Genera, Oberon, Norton Commander, Teletext, IBM 3270, classic Mac OS). The owner's swipe file lives outside the repository. We learn from these; we don't copy them.
- **First draft.** Feedreader's first draft (cell of Plex Mono at 13 pt and 1.6 lines, OKLCH inks, typed values, buttons with a hard shadow, links, places, tabs, grounded components, spec sheets, tables, trees, scrolling) carries over as the starting point, to be adjusted in Specimen.

## 6. Graphics

### 6.1 Components
- The inventory combines **HeroUI** (the current version, whose behavior rests on React Aria) and **Blueprint** (Palantir's toolkit for dense desktop interfaces). We take what each component is and how it behaves (states, keyboard, focus, what a screen reader says), never how it looks; React Aria's documentation is the behavior reference. Every component is built first, grounded (§4.4) where the spec asks for it; the owner then removes the ones Graticule doesn't keep.
- Beyond both, the components professional apps need and web kits lack: split panes and docking, with layouts saved as files; a virtual table for millions of rows (sort, group, resize, background filtering); tree; picker and find (§7.4); the code and text view (§7.3); forms and tables derived from the app's data types; the time axis (rulers, playhead, zoom, snapping, tracks as rows); charts on the canvas (lines, bars, candles, sparklines in table cells); node graphs with automatic layout; a diff view; a date picker.
- Every component has a page in `docs/components/` and lives in Specimen (§13.5).
- A component that panics draws an error box in its place; the rest of the app keeps working.

### 6.2 Pictures
Vector art made ahead of time: icons, ornaments, logos. Drawn as SVG and compiled by Engraver (§13.8) into shape data in the binary; drawn once per size into the GPU atlas, then one quad. Every picture has a required text label for screen readers. Graticule ships a built-in icon set.

### 6.3 Canvas
A box on the grid with its own coordinate space, holding shapes drawn at runtime: lines, curves, filled and stroked paths, text and images, in semantic ink and weights. Shapes have ids and are hit-tested against their outlines. The kernel's canvas is enough for charts and readouts; editable documents of shapes, infinite boards and maps belong to Plotter (§13.4). A canvas may ask for frames at display rate only while it is live; idle still costs nothing.

### 6.4 The escape hatch ladder
1. Components.
2. Canvas shapes.
3. A paint hook: the app paints a box with its own GPU commands.
4. **Proposed.** Shaders written in Rust (rust-gpu).
5. **Proposed.** GPU compute in Rust (CubeCL).

Every rung renders off the main thread into its own texture, which the main thread only places. Shaders receive the theme's inks as inputs, so custom GPU paint follows the theme and the appearance.

### 6.5 The web view
- Rule: **a web view for content you didn't write; Rust for everything you did.** Web pages, HTML email, embeds, DRM video and vendors' JavaScript SDKs (players, payment widgets, captchas) need a web view; nothing an app writes itself does.
- The web pane is the system web view placed over a box on the grid, navigating only within the zones it was opened for. It is made fast and calm, never a second UI toolkit.

## 7. Text

### 7.1 Fonts
- IBM Plex Mono and IBM Plex Sans (Open Font License) are the frame's only fonts, bundled. Plex Sans Arabic, Devanagari, Hebrew and Thai and Plex Math are bundled and load the first time text needs them; Plex Sans JP, KR, SC and TC are fetched by Bindery at a pinned commit and checked against pinned checksums. Mono for anything you compare, copy or align; Sans for prose and labels.
- Content may use any font a document asks for, loaded by name from the app or the system.
- The system's fonts are scanned on a background thread right after the first frame, so emoji and fonts named by documents are ready before they are needed and never pop in. Launch never waits for the scan.

### 7.2 Selection and fields
- Selection is on by default for all text, in reading order across components; controls opt out. Drag, double-click a word, triple-click a line, Shift-click to extend, Cmd-C to copy.
- Fields are Graticule's own, built to native behavior: the platform's input methods, dictation and emoji picker; the macOS text key bindings as the system's own table defines them; Tab and Shift-Tab between fields; single- and multi-line.
- **Typing is a command.** The kernel sends field edits as `Edit` commands through the app's `update` (§3.3), grouped by word, so they are undoable and visible like any other. Undo has one history, not one per field (§13.2).
- Number fields evaluate expressions (`12*4+3`, `3 h 20 min`); date fields accept natural input (`next tue 3pm`).

### 7.3 Documents
Fields cover single lines and short text. Documents (large files, multiple selections, syntax, spelling, typesetting, modal editing) belong to Galley (§13.3). Long paths are shortened in the middle everywhere.

### 7.4 Search
- Every list, table and tree can be filtered by typing: fuzzy, with matched letters marked, filtered in the background at any size, results streaming in. **Proposed:** nucleo, Helix's fuzzy matcher.
- One **picker** behind the palette, opening files, "go to" and place search; one **find** for find in a document, exact or by pattern, with replace. **Proposed:** ripgrep's search crates for exact and pattern search; tantivy for full-text indexes an app keeps.
- **Typed filter fields** generated from the app's data types: `price>100 sector:tech date:this-week`, with completions.
- Search follows the language: accents fold (`e` finds `é`), words split correctly in Japanese and Thai, names sort as each language expects. **Proposed:** ICU4X.

### 7.5 Language
Every label and message is translatable data, with plurals and grammar handled by Fluent. Labels and messages are written in English in the source; their ids come from the command or the call site, and a tool extracts them and reports ids that lost their source (after a rename) and texts that have no translation. Numbers, dates and currencies are formatted per language. Mixed directions are laid out correctly.

## 8. Interaction

### 8.1 Input
- Keys through the keymap (§3.4); the mouse and trackpad with the system's own momentum; pinch and rotate; pen pressure and tilt.
- Any input can drive any command or value: the registry makes every command and value bindable, so an app or a crate it adds can bind a MIDI knob, a gamepad or a hardware button to them. Device drivers are not part of Graticule.
- What you can act on looks actionable on sight, without hovering.

### 8.2 Drag and drop
Dragging within an app with a payload and drop targets, files dropped from the system, and dragging out a file that is written only when it is dropped. Snapping can confirm itself with the trackpad's haptic click.

### 8.3 Help and understanding
- The help page is the registry drawn as a tree, with keys.
- **What's this**: click anything to see its command, its keys, its documentation and why it is or isn't available.
- Settings are derived from the app's data types, searchable, with a view of only what differs from the defaults, undoable one by one.
- A visible log of what the user just did.
- What's new after an update is a page in the app, written by Bindery, never a popup.

## 9. Services
Batteries ship as separate crates; an app plugs in what it needs, and every app's binary size is budgeted. A capability becomes a service when most apps need it, apps usually do it badly, and doing it once lets Graticule guarantee something (speed, undo, privacy, visibility or correctness). If it falls out of the architecture, it is not built; it is kept possible.

- **Search** (§7.4).
- **Documents**: open, save, save as, autosave, versions, recent files, the unsaved mark, "changed on disk" with reload, projects as folders, huge files read on demand. Deleting moves to the Trash.
- **Data**: tables backed by background queries; charts from any column; exact decimal arithmetic for money; a rich clipboard that copies a table as a table, markdown or CSV depending on where it is pasted.
- **Time**: time zones and calendars (**Proposed:** jiff); relative times that cost nothing (each label wakes the app once, when its text would change, and labels on screen share one wakeup); background jobs grouped together and postponed in Low Power Mode.
- **Media**: images decoded off the main thread with cached thumbnails, color management and HDR where the display supports it; audio playback and recording on a real-time thread with waveform overviews and meters as live values; video through the platform's hardware decoder with frame-accurate seeking; QR codes.
- **Networking**: an HTTP client that is safe and polite by default, from Feedreader's fetcher: zones (a request reaches no deeper than whatever asked for it), retries with backoff, caching and conditional requests, a queue for offline, rate limits, every request visible in Scope.
- **Secrets** in the platform keychain; no token in a plain file.
- **Automation**: the registry as a command line (`app do export --pdf`), a URL scheme, Shortcuts actions, the Services menu, the share sheet and an MCP server for AI tools, local only. Thin adapters; nothing new is invented.
- **Print and PDF**: any screen or document as vector PDF at real size; physical units (mm, pt) exist only inside page boxes. Any component or screen copies as an image.

## 10. Native integration
- Native where the platform has a native; ours where it doesn't.
- **macOS:** the menu bar (app and Window menus, so system shortcuts and window tiling work), context menus generated from the registry (the one sanctioned `unsafe`), file dialogs, clipboard, input methods, cursors, appearance, accent color, reduced motion, increased contrast, Low Power Mode and thermal state, trackpad scrolling with the system's own momentum and haptics, accessibility through AccessKit, the system web view, menu bar items, background helpers and login items, file types and Quick Look, many windows and monitors, window restoration, full screen.
- **Linux:** after macOS works, verified on a real machine. Known: under Wayland there is no system context menu or global menu bar, so Graticule's own drawn menu is the native one; GNOME needs a drawn title bar; file dialogs through the XDG portal; input methods, clipboard, appearance, fontconfig and AT-SPI are available.
- Qt is not used: native only on KDE, heavy to ship, and its Rust bindings need `unsafe`.

## 11. Verification
- **Types** make mistakes impossible to write: layout code that can't leave the grid, state that can't contradict itself.
- **Laws** check behavior over generated inputs and command sequences (proptest): undo returns to where you were, field edits undo to the start and redo to the end, scroll positions follow their item, nothing moves under the reader, saved facts always load, every frame stays within budget.
- **Proofs** for the few cores where a mistake would be worst. **Proposed:** Kani, on grid arithmetic, field edit operations and Ledger's merging.
- **Every app is tested for free** against the universal laws, and every screen against the taste laws, by Assay (§13.7).
- Each unit carries its own laws, listed with it in §13.

## 12. Performance
Performance is design. Always faster, by reasonable means: nothing is broken or restricted only for speed.

### 12.1 Principles
- The work for an interaction is proportional to what changed, not to the size of the app or the document.
- The main thread never blocks on the network, the disk, the GPU or the display.
- No allocation per frame in steady state. Shaped text, glyphs and pictures are cached. Likely next documents are shaped in the background before they're opened.
- **Late reading**: because a frame takes microseconds, the kernel reads input as late as possible before the display's deadline and draws then, so what's on screen trails the hand by up to one refresh less.
- Idle is zero: no timers, no wakeups, no frames. The low-power GPU is preferred on Macs with two. Only changed regions are repainted.

### 12.2 Targets
Budgets apply to the slowest 1% of frames, on the reference machine (an M1), in release builds, checked automatically. The goals are long-term; today's numbers are Feedreader's Graticule.

| Stage | Today | Goal |
|---|---|---|
| Input to command | not measured | under 1 µs |
| Updating facts for one action | inside the frame | under 1 µs |
| Finding what's under the pointer | inside the frame | under 1 µs |
| Rebuilding and laying out a one-row change | whole screen each frame | under 20 µs |
| Painting the changed region | inside the frame | under 20 µs |
| Handing the frame to the GPU | not measured | under 50 µs |
| A whole frame for a small change | about 1.5 ms | under 100 µs |
| A full redraw (resize, theme) at 3,000 rows on a 5K display | about 1.5 ms on smaller screens | under 1 ms |
| Opening a document shaped in advance | not built | under 1 ms |
| Our part of launch | about 10 ms of 155 | under 5 ms |
| Allocation per frame, steady state | not checked | zero |
| Work while idle | none | none |

The M1's clock ticks about every 42 ns, so stages faster than that are measured over many repetitions.

### 12.3 Energy and size
Ambition: as far as reasonable means go. Two kinds of budget, tracked next to frame time:
- **Beat the platform's own app at the same job.** Galley uses less energy than TextEdit for an hour of writing (measured with `powermetrics`) and less memory; Lightbox less than Preview for the same image; Twin less than Finder for the same folder.
- **Hard ceilings.** **Proposed** numbers, set once the first slice measures the floor: every demo app under 10 MB on disk, not counting the fetched Chinese, Japanese and Korean fonts (today Feedreader's Specimen is 20 MB with line tables, and the bundled Plex is 6 MB); under 30 MB of memory while idle; zero wakeups while idle.

## 13. The units
Each unit has a model code, a library that apps build in, a face of its own that you can run, and its own laws. The faces are Graticule apps or command-line tools, so every unit also proves Graticule. Where a unit's core doesn't need a screen (Ledger, Galley's editing core, Plotter's document and geometry), it is a crate that doesn't depend on the kernel, and the parts that draw sit on top of it.

| Unit | Library | Face |
|---|---|---|
| DK-01 Kernel | the framework | `bindery new` gives a running empty app |
| LD-01 Ledger | the command log and the document's types: saving, undo, history, replays, sync | `ledger`: show, replay, verify, compare and export an app's history |
| GA-01 Galley | the text engine | `galley`, a text editor |
| PL-01 Plotter | the canvas engine | `plotter`, a whiteboard |
| SP-01 Specimen | the Specimen mode in every app | the catalog of Graticule's components |
| SC-01 Scope | the recorder in every app | a window that attaches to any running Graticule app |
| AS-01 Assay | the universal laws, the taste laws, the budgets | `assay`: every check over an app, as a report |
| EN-01 Engraver | the picture compiler | `engraver`: a folder of SVGs into shape data |
| BD-01 Bindery | packaging and the development loop | `bindery new`, `dev`, `install`, `update` |

### 13.1 DK-01 Kernel
- **Library.** `crates/kernel`: the tree, layout, painting, input, the registry and keymaps, jobs and live values, the shell around the operating system. Everything in §3–§8.
- **Face.** The empty app `bindery new` makes: facts, commands, one screen, one named state, Specimen wired in.
- **Laws.** The frame is on the grid; frames stay within budget; no allocation per frame; idle is zero; the view never writes into the facts.

### 13.2 LD-01 Ledger
Ledger keeps two records. The **command log** is local history: what the user did, for display, replay, rewind and crash replays. The **operations** are what those commands did to the document's types: they are what undo reverses and what sync sends.
- **Library.**
  - **Types.** The document part of facts is built from Ledger's types: `List` (ordered by fractional indexes, so concurrent insertions both survive), `Map`, `Text` (a CRDT for text) and plain values as last-writer-wins registers on hybrid logical clocks. `update` changes them, and each change becomes an operation. The facts the app reads are plain values on the main thread, and each change is written down as an operation while `update` runs. The CRDT underneath is Loro, chosen by measuring it against Automerge (`docs/internals/ledger.md`); it lives only on Ledger's own thread, merges and syncs, and sends merged changes back as operations, so no merge ever runs on the main thread (`docs/internals/kernel.md` §13).
  - **Saving.** Facts save themselves, safe against crashes, and an app reopens where it was: Ledger's thread keeps a snapshot of the facts and a log of every change since, and writes a new snapshot every few thousand changes and on closing. **Proposed:** the snapshot is Ledger's own, not the CRDT's document, because it loads twenty-five times faster and the CRDT is only needed once there is a second device (open question 3); the first frame after launch is drawn from the last snapshot before anything else answers. Session state is saved on the device in the same snapshot as the kernel's interaction state (§3.2). Facts saved by an older version load in a newer one through declared migrations.
  - **Undo.** One history per document, a tree rather than a line: after undoing and making a new change the old branch is still there, and history can be browsed. A step is any command that changes the document, or any edit typed into a field (grouped by word); commands that only move around (opening a file, selecting, scrolling, switching places) are not steps. A step records every change its command made, in the document and in session fields alike, and carries the selection and focus as they were, so undoing restores them too. Cmd-Z always undoes the newest step of the current branch, whatever has focus: typing "milk", adding it and pressing Cmd-Z takes the item back out and puts "milk" back in the field. Effects (printing, requests, writing files) are not changes to facts and are never steps; irreversible ones ask first.
  - **Replays.** A crash becomes a replay of the command log that the next launch can show; nothing is sent anywhere.
  - **Sync.** Operations travel between the user's own devices, never commands: a command's meaning depends on the facts where it runs, an operation's doesn't. Local-first and transport-agnostic: plain IP with TLS, pairing once, every address that ever worked tried in turn, devices on the local network found by Bonjour; the same network, Tailscale, Headscale, NetBird, ZeroTier, Nebula and the like are all just addresses. Encrypted between devices. Live collaboration is the same operations with authors attached.
  - The core needs no screen; any Rust program can keep a ledger.
- **Face.** `ledger` on the command line: show an app's history, replay it to any point, verify it, compare two devices, export it.
- **Laws.** Replaying the command log gives the same facts; undo returns to where you were and redo to where you left, selection included; devices that have received the same operations hold the same document, in any order of arrival; two concurrent additions to a `List` both survive; a selection never holds a key that no longer exists; every named state of every older version loads.

### 13.3 GA-01 Galley
Named for the tray where typesetters set lines of type.
- **Library.** Enough to build an editor like Helix or Neovim: rope storage for very large files, multiple selections, history through Ledger, a hook for tree-sitter syntax trees, folding, the keymap grammar of §3.4, a command line in the status line with completion from the picker, splits and buffers, macros recorded from the registry, find and replace. Language servers are the app's, run as jobs. Spelling is a hook: Galley asks it about words and ranges and gets back mistakes and suggestions; the default answers through the macOS spellchecker, and an app can plug in anything else. Typesetting is good by default: proper quotes and dashes, hyphenation, paragraph breaking that avoids ugly gaps, any font a document asks for. Print and PDF at real size. The editing core (storage, edits, the keymap grammar) needs no screen.
- **Face.** `galley`, a TextEdit-class editor with an optional modal mode. It is also the text demo (§14).
- **Laws.** Any sequence of edits undoes to the start and redoes to the end; selections stay valid through every edit; a keymap sequence means the same thing however it is typed; large files open and scroll within budget.

### 13.4 PL-01 Plotter
Named for pen plotters, which draw vectors.
- **Library.** A second kernel built from the same parts as the first: its document is facts, its edits are commands and transactions, its history is Ledger's. Its model is **tldraw**, the best canvas there is, learned from its documentation, its source code and the running app, and written our own way in Rust (tldraw's SDK is source-available under its own license, so nothing is copied). What we take: a store of typed records with migrations; bindings between shapes (an arrow that follows the box it points at); tools as state machines; transactions; the camera and the conversion between screen and page; geometry for hit testing; snapping to edges, centers, gaps and handles; culling of shapes off screen; fractional indexes for order; groups, frames and pages; locking and read-only; shared styles where the next shape takes the last one's; paste anything and drop files; export to SVG and PNG; side effects on record changes. Pressure-sensitive drawing ports perfect-freehand, which is MIT-licensed, with its notice. Infinite boards add a spatial index for millions of shapes, rubber-band selection, transform handles, snapping guides and a minimap. Map projections and tile streaming arrive with Atlas (§14) and work for anything with coordinates. The document and geometry need no screen; a board can render to SVG anywhere.
- **Face.** `plotter`, a whiteboard: open a board file, draw, export.
- **Laws.** A bound arrow stays bound through any move, resize or undo; order is stable under any insertion; export and reopen give the same board; hit testing agrees with what is drawn; panning and zooming stay within budget at a million shapes.

### 13.5 SP-01 Specimen
- **Library.** A mode of every Graticule app: on its own it shows Graticule's components; run from an app (`ticker --specimen`) it shows Graticule's components plus the app's own components and screens. Four places, always visible:
  - **Components.** A live overview page with every component in its default form, clickable and typeable. Each component's page: its documentation (`docs/components/`), live examples and the code for each, controls for its inputs, keyboard behavior, what a screen reader says, what it costs to draw, and the matrix of every state, style, appearance and width with nasty content (empty, very long, right-to-left, Chinese, emoji, huge numbers), generated from the component's inputs, with Assay's results on each.
  - **Screens.** Every app screen in each of its named states, live: clicks run the app's real code on a copy of the facts. Overlays for Tab order and what a screen reader would read.
  - **Style.** The live style and theme editor: every value of the style file, every component and screen updating as it moves, and the blast radius ("this affects 214 screens"). Saving writes the file.
  - **Changes.** Everything that renders differently since it was last approved: before and after, blink, a heatmap of differences, and the change described in words ("the Export button moved 2 columns right"), possible because everything is a known component on the grid. Each change is approved or rejected.
  - Reference images from a folder outside the repository can be pinned beside any sheet. Every sheet renders offscreen into a page, for review without a window.
- **Face.** The catalog of Graticule's components.
- **Laws.** Every named state of every app renders; an approved sheet that renders the same reports no change.

### 13.6 SC-01 Scope
- **Library.** The recorder in every Graticule app. The app only records cheap facts (timestamps, causes, ids) into a fixed ring buffer, always on, allocation-free and tens of nanoseconds a frame, and streams them over a local connection. History and source locations are recorded only in development builds.
- **Face.** A window of its own that attaches to any running Graticule app and does all the drawing, history and searching, plus overlays in the app for the grid and the outline and id of each component. It shows:
  - **Rewind**: a scrubber over the app's history; any moment saves as a replay file.
  - **Code**: Cmd-click a box and the editor opens at the line that created it.
  - **Facts**: the app's state as a tree, editable live.
  - **Why it redrew**: the cause of every frame (a key, data arriving, a live value).
  - **Why it's this size**: the layout's reasoning for a box, in cells.
  - **Commands**: every command, available or not and why, with its keys.
  - **Taste**: Assay's findings drawn on the screen.
  - **Effects and jobs**: every request, file write and URL opened, every running job, with timings.
  - **Time**: every stage of the last frames against the budget, and launch stage by stage.
- **Laws.** With Scope off, the frame time is the same as without Scope; with Scope on, the app stays within budget; Scope shows its own cost and subtracts it. Tracy remains for deep profiling (`--features profile`).

### 13.7 AS-01 Assay
Named for the test of what something is made of.
- **Library.** The checks every Graticule app is held to, callable from an app's own laws:
  - **Universal laws**: given any app, Assay generates command sequences and checks that it never crashes; undo returns; every command is reachable; a command `available` says `Yes` to neither panics nor does nothing; `available` stays within its time budget; no internal command is reachable by the user; saved facts load; and every frame stays within budget.
  - **Taste laws**, over every state of every app at every size and in every style: every edge on the grid; contrast; at most a set number of type sizes and inks per screen; nothing clipped; no text overflowing; hit targets at least one cell; no two items in one list share an accessible name; no component's part of the view exceeds its budget; every screen also judged as it looks with each kind of color blindness.
  - **Budgets**: frame time, allocation, idle, energy, binary size.
- **Face.** `assay` runs every check over an app and prints a report, like Clippy for Graticule apps; Specimen and Scope draw its findings on the screen.
- **Laws.** Assay's own: a check that passes on an app keeps passing when nothing that check reads has changed.

### 13.8 EN-01 Engraver
- **Library.** The picture compiler (§6.2), run as a build step: SVG files into compact shape data, colors mapped to semantic inks.
- **Face.** `engraver` compiles a folder of SVGs and shows what it made.
- **Laws.** A picture without a label is refused; every color lands on a semantic ink.

### 13.9 BD-01 Bindery
- **Library.** Packaging and the development loop.
- **Face.** `bindery new` starts an app. `bindery dev` rebuilds an app on every saved change, and the running app replaces itself with the new build in the same window frame and state. `bindery install` builds a native app (on macOS a signed `.app` with its command linked) and installs it; Bindery installs and updates apps from source, checking for new versions in the background and showing them without notifications, and fetches the fonts too large for the repository. macOS first, Linux and Windows later.
- **Laws.** A failed build never reaches the running app; a reload keeps the window frame and the state.

## 14. The demo apps
The faces of Galley, Plotter, Specimen and Scope are apps too. Beside them, six demo apps in `apps/` each push a different axis to its hard end. They are built on Graticule like any outside app would be, and any change to Graticule shows in them at once.

| App | Axes | What it proves |
|---|---|---|
| **Galley's editor** | text, output, windows | large documents, find and replace, any font inside the page, the keymap grammar, print and PDF |
| **Twin**, a two-pane file manager | OS presence, data size | Quick Look, dragging out to the system, file types, background work, huge folders |
| **Ticker**, a market board | data size, time | live values, million-row tables, candles on the canvas, the glow on change |
| **Atlas**, maps from OpenStreetMap | space | Plotter at its hardest: projections, zoom across twenty levels, streaming tiles, label placement, offline from a single file of vector tiles (Protomaps), with OpenStreetMap's attribution |
| **Patchbay**, a node editor | space, authoring | Plotter used by another app: nodes, wires, dragging, layouts as files |
| **Lightbox**, a photo and scan viewer | precision, GPU | the paint hook and Rust shaders, 16-bit levels, zoom and pan, measuring overlays |
| **Four-track**, a recorder and sequencer | real-time, time, authoring | the audio thread, waveform lanes, dragging clips, snapping, the time axis |

Order, by difficulty, because each forces new pieces into the kernel: Galley's editor and Twin; Ticker; Plotter's whiteboard, Atlas and Patchbay; Lightbox; Four-track.

## 15. Open questions
1. The Linux details (§10), after testing on a real machine.
2. Proofs kept as images, as text descriptions of the tree, or both; text for now, in the repository, one folder per platform (`docs/internals/proofs.md`).
3. Whether saved facts should be the CRDT's document from the start, or Ledger's own snapshot until a second device appears (§13.2, `docs/internals/ledger.md`); Ledger's own for now.
4. The four kinds of state as marks on fields or as separate structs (§3.2); marks for now.
5. Fractional display scales (Linux), where a cell can't be whole pixels and whole points at once.

## 16. Later
- Every Graticule app also running in a terminal or a browser tab, from the same tree. Parked until the framework stands on its own.
- Plugins: code written by others running inside an app in a sealed box (WebAssembly components using the same view API, with only the capabilities they are given).
- Specimen's sketch mode: placing components on the grid by hand and getting the view code out, one way only.
- Mobile.

## 17. Build plan
Status: done, partial, taken, open, paused or later. The plan is the whole of Graticule: everything in §1–§14 is built through the items below, and nothing in §16 is.

**How to work through it.** Take the first item, from the top, that isn't done or taken; mark it taken; build it to its finish line; commit; mark it done; take the next. An item's finish line is its own text, the laws of the unit it belongs to (§13) and the "Hit every surface" list in `AGENTS.md`; when it changes what the owner sees, it also adds to the guide. The sections run in order. Inside a section, items that don't need each other may run in parallel threads, each in its own checkout. A product or taste question goes to §15 and to the owner; the item waits for the answer, and work goes on with the next item that doesn't need it. Code is reviewed by a maintainer, or by a second agent on a different model, before it lands; when neither is reachable, work goes on and the review's findings are fixed when they come. Feedreader's source is public at https://github.com/Kosyakssss/feedreader; look for a checkout beside this repository (`../feedreader`) and clone it there if there isn't one.

### 17.1 Foundations
1. **Partial.** The documents: `AGENTS.md`, this spec, the guide's first page and `docs/api.md`. Open: the owner's reading and adjustment.
2. **Partial.** The code we want to write (`docs/api.md`): a to-do app, Galley's editor screen, Ticker's main screen and Lightbox's GPU box, revised after two rounds of review. Open: the owner's approval.
3. **Partial.** The workspace: a crate per unit (a core crate without the kernel where the unit has one), `graticule`, the one crate apps import, above the kernel and Specimen, `apps/`, the lints (no `unsafe`), and the recipes for recurring jobs. The workspace and the lints were built in S2; open: the recipes.

### 17.2 The first slice
The first slice builds the first part of most items below: the kernel, Ledger with saving and undo, the to-do app from `docs/api.md` and a minimal Specimen. The to-do app is not a demo app; it is the smallest app that runs through every layer, and its code is the yardstick for app code a person can read. The slice is done when the to-do app runs in a window on the grid; undo works, field typing included; the app reopens where it was left; `Remove` works from a row, from a keymap sequence and from the palette through its subject; one effect (export to a file) brings its result back as a command; and Specimen shows the app's named states. The laws for the grid, the frame budget, no allocation per frame and idle start with it. It is built in steps, one thread each, in this order:

- **S1. Written, awaiting maintainer review.** **The kernel's design**, written as `docs/internals/kernel.md` before any kernel code: the tree and its ids, layout in cells, comparing trees and repainting changed regions, hit testing, subjects and selection, edits through `update`, effects, derived and live values, paths and their versions, and how Ledger plugs in. Reviewed by a maintainer. The owner has answered the questions it raised.
- **S2. Done.** **First light.** The workspace and the lints; the window, event loop, fonts, text and GPU renderer moved in from Feedreader; an empty window with the grid drawn. Laws: idle is zero, no allocation per frame. Built and verified on Windows, the machine the work happened on; the macOS-only parts of the shell wait for a Mac (`docs/internals/shell.md`).
- **S3. Done, but for one measurement.** **The tree.** Layout in cells, comparison and repainting, hit testing, and the components the to-do app needs (page, row, list, field, checkbox, link, button, action bar, status line, message line). The reading side of facts and commands: `derive(Facts)` with paths, `List`, `Text` and `Selection` without operations, `derive(Commands)` with names and labels, `available`. The to-do view renders from fixed facts. The kept list of quads measured against a terminal-style buffer of cells, learned from Ghostty, and the faster one kept (`docs/internals/kernel.md` §7.2). The kept quad list is built and measured on the CPU, and is the renderer; the accessibility tree comes from the tree through AccessKit; laws: every box within its parent, no two ids alike, the node hit is the topmost, patching the kept quads equals painting afresh, a list keeps its place. Open: the cell buffer and the GPU time at 5K, measured on the M1.
- **S4. Done, but for one piece.** **Commands.** `update` with `Cx` and `Outcome`, subjects and selection, `Edit` from fields, a keymap file with which-key, the palette. Built: keys go to the palette, the focused field, a focused control, the keymap and then the platform's conventions; the field's editing model from Feedreader with the clipboard; the kernel's own commands (`builtin.rs`); subjects from the focused list, with bulk commands as one transaction that skip and name what they can't take; focus kept in place when its row is removed; the to-do app's `keymap.txt`. Laws: a command never runs when `available` says no, a bulk command is one transaction, field edits through `update` give the field model's text. Open: the input method's unfinished text is not drawn yet (committed text works).
- **S5. Done.** **Ledger.** Operations written down during `update`, registers, the CRDT library chosen by measuring both candidates on Ledger's own thread; the undo tree with selection, focus and caret; versions; the command log; jobs, a list's tallies and bulk changes in the background; saving and reopening where the app was left. Built: registers compared after `update`, rollback when `update` panics, the undo tree with selection, focus and caret (`ledger/src/history.rs`), the command log with replays, Loro chosen by measurement, saving and reopening where the app was left (`ledger/src/store.rs`), tallies, and bulk changes searched by a job and landed in slices (`ledger/src/bulk.rs`), with their laws. Versions moved to item 13, where derived values first read them; so did undoing a bulk change in slices (`docs/internals/kernel.md` §20).
- **S6. Done, but for one pass in the real window.** **Effects.** The save dialog, making and writing the file in the background, the result back as an internal command, the message line. The save panel needs the owner's pass on screen: offscreen renders can't open it.
- **S7. Done.** **Specimen, minimal.** The Screens place with the to-do app's named states, and offscreen renders. `--specimen` opens it: each named state live on its own copy, switched with "Show …" commands in the bar and palette, effects shown instead of run. `--render <folder>` moved into the specimen crate.

### 17.3 Tooling
Tooling comes before the apps, by the owner's decision, so every later piece is reviewed as pictures, measured and checked from its first commit.

4. **Partial.** SP-01 Specimen properly (§13.5): Components (the live overview and component pages), Screens, Style, Changes, reference images, offscreen pages. Built in parts: (a) the four places, always in the bar, and Specimen's own app with the live overview, done; (b) component pages, one per component from `docs/components/`, each with its live example and its awkward content, done; (c) Style, every ink of the theme in both appearances with its contrast, and the grid and type values, shown but not yet editable (editing comes with item 10), done; (d) Changes, every sheet compared with its approved proof and the differences said in words, approved one by one or all at once, in the window or with `--changes` and `--approve`, done but for before-and-after pictures, blink and a heatmap, which wait for open question 2 and a component that shows a picture; (e) offscreen pages, done: `--render` writes a page (`index.html`) with every state in both appearances and both sizes and what changed, and pins reference images from a folder outside the repository (`--references <folder>`) beside the state they are named after. Open: pinning reference images beside a sheet in the window, and the pictures on Changes, both of which need item 16's picture component; the component pages' inputs, cost to draw and the matrix of every state, style and width, which come with items 10 and 15.
5. **Done.** SC-01 Scope, small (§13.6): the ring buffer, its own window, why it redrew, time. Every app records each frame's cause and stage times into a fixed ring (39 ns a frame in release) and serves them on this machine only; `cargo run -p scope` attaches to a running app and shows Time and Why it redrew (`docs/internals/scope.md`). Its own-cost laws come with item 6's budgets.
6. **Open.** AS-01 Assay, small (§13.7): the universal laws over generated command sequences, the grid and contrast laws, the frame, allocation and idle budgets, the `assay` report.
7. **Open.** BD-01 Bindery's development loop moved in: `bindery new` and `bindery dev` with the in-place reload.
8. **Open.** EN-01 Engraver moved in; the built-in icon set.

### 17.4 Components and the kernel under them
The components are done properly before any demo app. Each is built with its page in `docs/components/`, its Specimen entry and its laws, and the kernel pieces it needs are built with it.

9. **Open.** The inventory: every component of HeroUI and Blueprint, grounded (§4.4) where the spec asks for it, plus the components of §6.1 that both lack, sorted into waves. All of them are built; the owner removes the ones Graticule doesn't keep afterwards, in Specimen.
10. **Open.** Styles and themes (§5): style and theme files edited live, switching at runtime, OKLCH with contrast as a law, semantic inks, the typed values.
11. **Open.** The rest moved in from Feedreader, each with its laws and its traps in `docs/internals/`: character graphics, fields with input methods and the macOS key bindings, selection across components, clipboard, menus and context menus (the `unsafe` exception), file dialogs, scroll anchoring, accessibility through AccessKit, launch measurement.
12. **Open.** Commands in full (§3.3, §3.4, §8.3): typed arguments, keymap files applied live, sequences, counts, operators and modes, the which-key tree; the help page, what's this, settings derived from the facts, the visible log of what the user did.
13. **Open.** Jobs and live values in full, with versions and derived values; undoing a bulk change in slices; the main thread freed of shaping and waiting; late reading (§3.5, §12.1).
14. **Open.** Anchored overlays, drag and drop, motion (§4.4, §4.5, §8.2).
15. **Open.** The components, wave by wave, including the fields in full (§7.2), filtering by typing, the picker, find and typed filter fields (§7.4), the virtual table, trees, split panes and docking, the time axis, charts, node graphs, the diff view, the date picker.
16. **Open.** Pictures and the canvas (§6.2, §6.3); the escape hatch ladder: the paint hook, Rust shaders, GPU compute (§6.4).
17. **Open.** Language (§7.5): labels and messages as Fluent data, the extraction tool, formatting per language, mixed directions, right-to-left mirroring.
18. **Open.** The rest of macOS (§8.1, §10): the menu bar and menu bar items, many windows and monitors, window restoration, full screen, file types and Quick Look, background helpers and login items, appearance, reduced motion, increased contrast, Low Power Mode and thermal state, pen, pinch and haptics, any input bound to any command; the web view (§6.5).
19. **Open.** SC-01 Scope complete: rewind, code, facts, why it's this size, commands, taste, effects and jobs.
20. **Open.** AS-01 Assay complete: every taste law, color blindness, the energy and size budgets.

### 17.5 Units
21. **Open.** LD-01 Ledger complete (§13.2): snapshots, migrations, crash replays, sync between devices, live collaboration, the `ledger` command.
22. **Open.** Proofs (§11): Kani on grid arithmetic, field edit operations and Ledger's merging.
23. **Open.** GA-01 Galley (§13.3): the editing core, then the editor.
24. **Open.** PL-01 Plotter (§13.4): the tldraw study written up in `docs/internals/`, then records, bindings, tools as state machines, camera, geometry, snapping, perfect-freehand, then the whiteboard.
25. **Open.** BD-01 Bindery complete (§13.9): `bindery install` and `update`, the fetched fonts, the what's new page.

### 17.6 Services
26. **Open.** The services of §9 (search, documents, data, time, media, networking, secrets, automation, print and PDF), one at a time, each when the first app needs it.

### 17.7 The apps
In the order of §14. Each app exercises every version of every component it uses, is checked by Assay, and meets its energy and size budgets (§12.3).

27. **Open.** Galley's editor and Twin.
28. **Open.** Ticker.
29. **Open.** Plotter's whiteboard, Atlas and Patchbay.
30. **Open.** Lightbox.
31. **Open.** Four-track.

### 17.8 Beyond macOS
32. **Open.** Linux (§10), verified on a real machine; open questions 1 and 5 answered there.
33. **Open.** Windows.

## 18. References
- [HeroUI](https://heroui.com) · [React Aria](https://react-spectrum.adobe.com/react-aria/) · [Blueprint](https://blueprintjs.com) · [U.S. Graphics Company](https://usgraphics.com)
- [tldraw](https://github.com/tldraw/tldraw) and [its license](https://tldraw.dev/legal/tldraw-license) · [perfect-freehand](https://github.com/steveruizok/perfect-freehand)
- [Helix](https://github.com/helix-editor/helix) · [nucleo](https://github.com/helix-editor/nucleo) · [tree-sitter](https://tree-sitter.github.io) · [ripgrep](https://github.com/BurntSushi/ripgrep) · [tantivy](https://github.com/quickwit-oss/tantivy) · [ICU4X](https://github.com/unicode-org/icu4x) · [Fluent](https://projectfluent.org) · [jiff](https://github.com/BurntSushi/jiff)
- [winit](https://github.com/rust-windowing/winit) · [wgpu](https://github.com/gfx-rs/wgpu) · [parley](https://github.com/linebender/parley) · [AccessKit](https://github.com/AccessKit/accesskit) · [muda](https://github.com/tauri-apps/muda) · [wry](https://github.com/tauri-apps/wry) · [lyon](https://github.com/nical/lyon) · [IBM Plex](https://github.com/IBM/plex)
- [rust-gpu](https://github.com/Rust-GPU/rust-gpu) · [CubeCL](https://github.com/tracel-ai/cubecl) · [Kani](https://github.com/model-checking/kani) · [proptest](https://github.com/proptest-rs/proptest) · [Tracy](https://github.com/wolfpld/tracy)
- [Loro](https://github.com/loro-dev/loro) · [Automerge](https://github.com/automerge/automerge) · [Protomaps](https://protomaps.com) · [OpenStreetMap](https://www.openstreetmap.org/copyright)
- Precedents: [Flutter](https://flutter.dev) · [GPUI](https://www.gpui.rs) · [Xilem](https://github.com/linebender/xilem) · [Storybook](https://storybook.js.org) · [Vaxry, "Electron ain't bad, actually"](https://blog.vaxry.net/articles/2025-electronAintBad)
