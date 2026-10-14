# Graticule: spec

The source of truth for what to build: Graticule (model DK-01), a framework for native desktop apps in Rust, its units and the demo apps that prove it. Working rules are in [`AGENTS.md`](../AGENTS.md); the same ideas in plain words are in [`guide/`](guide/README.md). The build plan and its status are in §17.

Every decision below is the owner's unless marked **Proposed** (engineering choice, unconfirmed) or **Open** (§15). Visual details (exact spacing, weights, colors, glyphs) are designed with the owner component by component in Specimen; this document records principles and decisions.

## 0. Ground rules

- One Rust codebase: the units, the services and the demo apps, in one cargo workspace. Changing the kernel rebuilds every app in the same step, and a breaking change fixes every app in the same change.
- Graticule is made of **units** (§13), each with a model code. Every unit is a library that apps build in and a face of its own that you can run, an app or a command-line tool, so every unit also proves Graticule. Where a unit's core doesn't need a screen, it doesn't depend on the kernel.
- A clean start. Graticule began inside Feedreader; its architecture is rewritten here, and the parts that meet the operating system are moved in one at a time with their laws and the traps they avoid (`docs/internals/`), not rewritten. Feedreader is paused and later depends on Graticule like any other app.
- Desktop first: macOS, then Linux. Mobile and Windows come later (§15).
- No `unsafe` in our code; the workspace denies it, with one approved exception (`docs/internals/unsafe.md`): the native context menu on macOS. Every exception is justified to the owner before it lands.
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
- The app supplies **facts** (plain data and enums), **commands** (the only way facts change), a **view** (the screen described from the facts), **states** (named sets of facts for Specimen) and **effects** as data.
- Graticule owns everything else: the shell (windows, the event loop, input, focus, hover, dragging, scrolling, text input, clipboard, menus, web views, file dialogs, accessibility), the per-component interaction state kept by stable id (scroll positions, hover, pressed, selection), the components, styles and themes, layout, painting, undo, the tools and the budgets.
- The view is a pure function of the facts. Painting never writes into the app's facts.

### 3.2 The tree
- The view builds a tree of components each time; building it is cheap and allocates nothing in steady state (reused flat storage, integer ids).
- Ids are automatic, from position and from the key of each item in a list, so the app never names a component to keep its state.
- The kernel compares each part with last time and reuses what didn't change, lays out only what is visible and repaints only the regions that changed. A cursor move repaints two rows.
- Every component records where in the source it was created, in development builds.
- One tree, read by everything: painting, hit testing, the accessibility tree, Specimen, Scope, the taste laws, automation, structural diffs, print. There is no second tree to keep in sync.

### 3.3 Commands, the registry and keys
- Commands are an enum with typed arguments. Each has a name, a label and availability. Availability is `Yes` or `No(reason)`: a disabled control says why in the status line.
- The registry is generated from the commands and feeds the action bar, which-key, the help page, the palette, the menu bar, context menus, automation and the OS adapters (§9).
- Keys live in **keymap files**, not in code: chords, sequences (`g g`), counts (`3w`), operator and motion (`d w`) and modes (normal, insert, select), enough to build a modal editor. Edits to a keymap file apply live. Graticule ships no keys of its own.
- At runtime the registry builds the tree of what is reachable from here by key; after a prefix, the action bar shows the next branch (which-key).
- Every command is undoable by default. Undo is a tree, kept by Ledger (§13.2).

### 3.4 Effects, jobs and the main thread
- The main thread only turns input into commands, updates the facts, builds the tree, paints and submits. It never does work that grows with the data.
- Anything that grows with the data (filtering, shaping long text, placing labels, decoding images, network, disk) runs as a **job** in the background. While it runs the screen shows what it has and says that more is coming; every running job is visible.
- **Live values**: a value written by another thread (an audio callback, a market feed, a download) without locks and read once per frame, coalesced to at most one frame per display refresh.

### 3.5 States
- A named state is a set of facts. Specimen shows every app's states (§13.5); "send this view" saves the current facts to a file that opens in the same state on another machine.
- Saving, migrations, history and sync belong to Ledger (§13.2).

## 4. The grid and its freedoms

### 4.1 The grid
- The screen is paper ruled in cells. The cell comes from the monospace font: its width is the advance of IBM Plex Mono, its height the line, both whole device pixels; the font size is derived from the cell, not the other way around.
- Layout is in whole cells and the layout API can only express cells: rows, columns, splits, weights, minimums and maximums. Pixel coordinates exist only inside a canvas, a picture or a component's own drawing. Narrow and wide are column counts.
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
- The inventory combines **HeroUI** (the current version, whose behavior rests on React Aria) and **Blueprint** (Palantir's toolkit for dense desktop interfaces). We take what each component is and how it behaves (states, keyboard, focus, what a screen reader says), never how it looks; React Aria's documentation is the behavior reference. Each component is mapped as kept, grounded (§4.4) or skipped, with the owner.
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
- Fields are Graticule's own, built to native behavior: the platform's input methods, dictation and emoji picker; the macOS text key bindings as the system's own table defines them; Tab and Shift-Tab between fields; undo per field; single- and multi-line.
- **Typing is a command.** The kernel sends field edits as commands, so they are undoable and visible like any other.
- Number fields evaluate expressions (`12*4+3`, `3 h 20 min`); date fields accept natural input (`next tue 3pm`).

### 7.3 Documents
Fields cover single lines and short text. Documents (large files, multiple selections, syntax, spelling, typesetting, modal editing) belong to Galley (§13.3). Long paths are shortened in the middle everywhere.

### 7.4 Search
- Every list, table and tree can be filtered by typing: fuzzy, with matched letters marked, filtered in the background at any size, results streaming in. **Proposed:** nucleo, Helix's fuzzy matcher.
- One **picker** behind the palette, opening files, "go to" and place search; one **find** for find in a document, exact or by pattern, with replace. **Proposed:** ripgrep's search crates for exact and pattern search; tantivy for full-text indexes an app keeps.
- **Typed filter fields** generated from the app's data types: `price>100 sector:tech date:this-week`, with completions.
- Search follows the language: accents fold (`e` finds `é`), words split correctly in Japanese and Thai, names sort as each language expects. **Proposed:** ICU4X.

### 7.5 Language
Every label and message is translatable data, with plurals and grammar handled by Fluent. Numbers, dates and currencies are formatted per language. Mixed directions are laid out correctly.

## 8. Interaction

### 8.1 Input
- Keys through the keymap (§3.3); the mouse and trackpad with the system's own momentum; pinch and rotate; pen pressure and tilt.
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
- **Linux:** Open, verified later on a real machine. Known: under Wayland there is no system context menu or global menu bar, so Graticule's own drawn menu is the native one; GNOME needs a drawn title bar; file dialogs through the XDG portal; input methods, clipboard, appearance, fontconfig and AT-SPI are available.
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
**Open:** budgets for energy (an hour of reading in Galley, measured with `powermetrics`) and for each app's binary size and memory, tracked next to frame time.

## 13. The units
Each unit has a model code, a library that apps build in, a face of its own that you can run, and its own laws. The faces are Graticule apps or command-line tools, so every unit also proves Graticule. Where a unit's core doesn't need a screen (Ledger, Galley's editing core, Plotter's document and geometry), it is a crate that doesn't depend on the kernel, and the parts that draw sit on top of it.

| Unit | Library | Face |
|---|---|---|
| DK-01 Kernel | the framework | `bindery new` gives a running empty app |
| LD-01 Ledger | the command log: saving, undo, history, replays, sync | `ledger`: show, replay, verify, compare and export an app's history |
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
One log of commands underlies saving, undo, rewind, crash replays and sync; Ledger is that log.
- **Library.** Facts save themselves, safe against crashes, and an app reopens where it was; the first frame after launch is drawn from the last snapshot before anything else answers. Undo is a tree, not a line: after undoing and making a new change the old branch is still there, and history can be browsed. Facts saved by an older version load in a newer one through declared migrations. A crash becomes a replay that the next launch can show; nothing is sent anywhere. Sync sends commands between the user's own devices, local-first and transport-agnostic: plain IP with TLS, pairing once, every address that ever worked tried in turn, devices on the local network found by Bonjour; the same network, Tailscale, Headscale, NetBird, ZeroTier, Nebula and the like are all just addresses. Encrypted between devices. Live collaboration through CRDTs (**Open**: Loro or Automerge). The core needs no screen; any Rust program can keep a ledger.
- **Face.** `ledger` on the command line: show an app's history, replay it to any point, verify it, compare two devices, export it.
- **Laws.** Replaying the log gives the same facts; undo returns to where you were and redo to where you left; devices that have seen the same commands agree, in any order of arrival; every named state of every older version loads.

### 13.3 GA-01 Galley
Named for the tray where typesetters set lines of type.
- **Library.** Enough to build an editor like Helix or Neovim: rope storage for very large files, multiple selections, history through Ledger, a hook for tree-sitter syntax trees, folding, the keymap grammar of §3.3, a command line in the status line with completion from the picker, splits and buffers, macros recorded from the registry, find and replace. Language servers are the app's, run as jobs. Spelling is a hook: Galley asks it about words and ranges and gets back mistakes and suggestions; the default answers through the macOS spellchecker, and an app can plug in anything else. Typesetting is good by default: proper quotes and dashes, hyphenation, paragraph breaking that avoids ugly gaps, any font a document asks for. Print and PDF at real size. The editing core (storage, edits, the keymap grammar) needs no screen.
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
  - **Universal laws**: given any app, Assay generates command sequences and checks that it never crashes, undo returns, every command is reachable, saved facts load, and every frame stays within budget.
  - **Taste laws**, over every state of every app at every size and in every style: every edge on the grid; contrast; at most a set number of type sizes and inks per screen; nothing clipped; no text overflowing; hit targets at least one cell; every screen also judged as it looks with each kind of color blindness.
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
1. License: AGPL-3.0 or MIT.
2. Windows support, and when.
3. The Linux story (§10), after testing on a real machine.
4. Whether the macOS spellchecker can be reached without `unsafe` through the AppKit bindings; if not, whether it becomes a second exception.
5. Proofs kept as images, as text descriptions of the tree, or both.
6. The CRDT for live collaboration (§13.2).
7. Budgets for energy, binary size and memory (§12.3).

## 16. Later
- Every Graticule app also running in a terminal or a browser tab, from the same tree. Parked until the framework stands on its own.
- Plugins: code written by others running inside an app in a sealed box (WebAssembly components using the same view API, with only the capabilities they are given).
- Specimen's sketch mode: placing components on the grid by hand and getting the view code out, one way only.
- Mobile.

## 17. Build plan
Status: done, partial, open, paused or later. Each step is committed when it works and adds to the guide when it changes what the owner sees.

### 17.1 Foundations
1. **Partial.** The documents: `AGENTS.md`, this spec, the guide's first page and `docs/api.md`. Open: the owner's reading and adjustment.
2. **Open.** The code we want to write (`docs/api.md`) approved by the owner: a to-do app, Galley's editor screen and Ticker's main screen, readable top to bottom.
3. **Open.** The workspace: a crate per unit (a core crate without the kernel where the unit has one), `apps/`, the lints (no `unsafe`), and the recipes for recurring jobs.

### 17.2 DK-01 Kernel
4. **Open.** The tree: automatic ids, comparison with the last tree, layout in cells only, painting of changed regions, hit testing, source locations. Laws from the first commit: the grid, the frame budget, no allocation per frame, idle is zero.
5. **Open.** Moved in from Feedreader, each with its laws and its traps in `docs/internals/`: the window and event loop, fonts and the text system, the GPU renderer and atlas, character graphics, fields with input methods and the macOS key bindings, selection, clipboard, menus and context menus (the `unsafe` exception), file dialogs, scroll anchoring, accessibility through AccessKit, the in-place reload, launch measurement.
6. **Open.** Commands: typed arguments, availability with reasons, keymap files applied live, sequences, counts, operators and modes, the which-key tree, typing as commands.
7. **Open.** Jobs and live values; the main thread freed of shaping and waiting; late reading.
8. **Open.** Anchored overlays; drag and drop.
9. **Open.** The component inventory from HeroUI and Blueprint, mapped with the owner, then built one at a time with its page and its Specimen entry.
10. **Open.** The escape hatch ladder: the paint hook, Rust shaders, GPU compute.

### 17.3 Units
11. **Open.** LD-01 Ledger: the log, saving and snapshots, the undo tree, migrations, crash replays, then sync. The `ledger` command.
12. **Open.** SP-01 Specimen: Components (the live overview and component pages), Screens, Style, Changes.
13. **Open.** SC-01 Scope: the ring buffer, its own window, rewind, code, facts, causes, layout reasons, commands, taste, effects, time.
14. **Open.** AS-01 Assay: the universal laws, the taste laws, the budgets, the `assay` report.
15. **Open.** EN-01 Engraver moved in; the built-in icon set.
16. **Open.** BD-01 Bindery moved in; `bindery new`.
17. **Open.** GA-01 Galley: the editing core, then the editor.
18. **Open.** PL-01 Plotter: the tldraw study written up in `docs/internals/`, then records, bindings, tools as state machines, camera, geometry, snapping, perfect-freehand, then the whiteboard.

### 17.4 Services
19. **Open.** Search, documents, data, time, media, networking, secrets, automation, one at a time as the apps need them.

### 17.5 The apps
20. **Open.** Twin.
21. **Open.** Ticker.
22. **Open.** Atlas and Patchbay.
23. **Open.** Lightbox.
24. **Open.** Four-track.

## 18. References
- [HeroUI](https://heroui.com) · [React Aria](https://react-spectrum.adobe.com/react-aria/) · [Blueprint](https://blueprintjs.com) · [U.S. Graphics Company](https://usgraphics.com)
- [tldraw](https://github.com/tldraw/tldraw) and [its license](https://tldraw.dev/legal/tldraw-license) · [perfect-freehand](https://github.com/steveruizok/perfect-freehand)
- [Helix](https://github.com/helix-editor/helix) · [nucleo](https://github.com/helix-editor/nucleo) · [tree-sitter](https://tree-sitter.github.io) · [ripgrep](https://github.com/BurntSushi/ripgrep) · [tantivy](https://github.com/quickwit-oss/tantivy) · [ICU4X](https://github.com/unicode-org/icu4x) · [Fluent](https://projectfluent.org) · [jiff](https://github.com/BurntSushi/jiff)
- [winit](https://github.com/rust-windowing/winit) · [wgpu](https://github.com/gfx-rs/wgpu) · [parley](https://github.com/linebender/parley) · [AccessKit](https://github.com/AccessKit/accesskit) · [muda](https://github.com/tauri-apps/muda) · [wry](https://github.com/tauri-apps/wry) · [lyon](https://github.com/nical/lyon) · [IBM Plex](https://github.com/IBM/plex)
- [rust-gpu](https://github.com/Rust-GPU/rust-gpu) · [CubeCL](https://github.com/tracel-ai/cubecl) · [Kani](https://github.com/model-checking/kani) · [proptest](https://github.com/proptest-rs/proptest) · [Tracy](https://github.com/wolfpld/tracy)
- [Loro](https://github.com/loro-dev/loro) · [Automerge](https://github.com/automerge/automerge) · [Protomaps](https://protomaps.com) · [OpenStreetMap](https://www.openstreetmap.org/copyright)
- Precedents: [Flutter](https://flutter.dev) · [GPUI](https://www.gpui.rs) · [Xilem](https://github.com/linebender/xilem) · [Storybook](https://storybook.js.org) · [Vaxry, "Electron ain't bad, actually"](https://blog.vaxry.net/articles/2025-electronAintBad)
