# The kernel's design

Status: S1 of the first slice (spec §17), written and awaiting maintainer review. Nothing here is built yet. Decisions marked **Proposed** are engineering choices this page makes; questions only the owner could answer went to the spec's §15, and the owner's answers are in §21.

This page says how the kernel turns facts into pixels and input into commands, well enough that S2–S7 can be built from it without guessing. It is held to one test: the to-do app in [`docs/api.md`](../api.md) compiles and runs as written. Where it can't, §16 says so and names the smallest change to `api.md`.

## 1. The shape in one paragraph

The app's facts are plain Rust values that live on the main thread. A command reaches `update`, which changes them; Ledger's types write down every change as an operation while it happens, and plain fields are compared before and after. The kernel then calls `view`, which builds a flat tree of components with stable ids; the kernel lays the tree out in whole cells, compares it with the last one by id, repaints only what changed into a texture it keeps, and hands that to the GPU. Input finds its target in the last tree, becomes a command and starts the loop again. Nothing runs when nothing happens.

## 2. Crates

| Crate | Package | Depends on | Holds |
|---|---|---|---|
| `crates/ledger` | `ledger` | nothing of ours | `Facts`, `List`, `Map`, `Text`, `Key`, `Selection`, `Edit`, `Op`, the journal, the undo tree, saving. No screen. |
| `crates/macros` | `graticule-macros` | nothing of ours (proc-macro) | `derive(Facts)`, `derive(Commands)`, `derive(Form)`, `derive(Choice)`, `t!` |
| `crates/kernel` | `kernel` | `ledger` | the shell (window, event loop, input), the tree, layout, painting, hit testing, the registry and keymaps, `Cx`, effects, jobs, live and derived values, the components |
| `crates/specimen` | `specimen` | `kernel` | Specimen's places, offscreen renders (S7) |
| `crates/graticule` | `graticule` | all of the above | the prelude and `graticule::run`, which opens the app or, with `--specimen`, Specimen over it |
| `apps/todo` | `todo` | `graticule` | the to-do app from `api.md` |

`graticule` exists only because Specimen is a mode of every app and draws with the kernel: the kernel can't depend on Specimen, so the one crate apps import sits above both. It holds no logic beyond `run`. Ledger depends on nothing of ours, so any Rust program can keep a ledger (spec §13.2); the derive emits kernel code only for fields marked `#[live]`, `#[derived]` or `#[resource]`.

### Key types and traits

| Name | Crate | What it is |
|---|---|---|
| `Facts` | ledger | Implemented by `derive(Facts)`. Paths, the comparison of plain fields, applying operations and edits, saving and loading, attaching containers. |
| `Record` | ledger | A `Facts` type with a `#[key]` field, so it can live in a `List`. |
| `Find<T>` | ledger | "These facts can find a `T` by its key." Generated for every `List<T>` and `Map<_, T>` field. Labels and `available` use it. |
| `List<T>`, `Map<K, V>`, `Text` | ledger | The containers of the document and session parts. Each journals its own changes. |
| `Key<T>` | ledger | A 128-bit id: the device's replica id and a counter. `Copy`. |
| `Selection<T>` | ledger | A set of `Key<T>` plus the focused one. Session state, pruned when keys disappear. |
| `FactPath<F, T>` | ledger | A typed path from `F` to a `T` inside it: `Todo::draft` is a `FactPath<Todo, Text>`. Generated as associated constants. |
| `Op`, `Journal`, `Transaction` | ledger | What a change did, the record of a transaction's operations, and the scope that collects them. |
| `Edit` | ledger | A change the kernel asks for on a path: typing, a selection, a form value. Arrives through `Cmd::Edit`. |
| `Ledger` | ledger | The history: steps, the undo tree, the command log, snapshots. |
| `App` | kernel | `available`, `update`, `view`, `states`, `start`. |
| `Commands<F>` | kernel | Implemented by `derive(Commands)`: names, labels, subjects, which variant carries `Edit`, encoding for the log. |
| `Available`, `Outcome` | kernel | `Yes` / `No(Message)`; `Changed` / `Nothing`. |
| `Cx<C>` | kernel | What `update` may ask for: keys, the time, effects, messages. |
| `Ui<'a, C, S = ()>` | kernel | The view's builder. `S` is the subject type of the row being built. |
| `Tree`, `Node`, `Id` | kernel | The flat tree, its nodes and their stable ids. |
| `Area` | kernel | A rectangle in whole cells. The only geometry layout can speak. |
| `Message` | kernel | Text for the user, made by `t!`: an id, the English source and its arguments. |
| `Effect<C>` | kernel | Work asked for as data, with the command that brings its result back. |
| `Jobs<C>`, `Live<T>`, `Derived<T>` | kernel | Background work, values written by other threads, values computed in the background. |
| `Path`, `Failure` | kernel | A file path the user sees (shown shortened in the middle) and a failure with a message. |
| `States<F>` | kernel | The named states Specimen shows. |

## 3. A frame, start to finish

1. **Input.** The shell receives an event from winit. Pointer events are hit-tested against the last tree (§8). A key goes first to the focused field, then to the keymap (spec §3.4, and Feedreader's trap: "the focused field comes before the registry").
2. **Command.** The target resolves to a command value: a button's command, a row's command with its subject filled in, a keymap sequence's command with the current subject, a kernel command (undo, copy, focus the next field), or an `Edit` wrapped in the app's `Edit` variant.
3. **Available.** The kernel asks `available` again, right before running, because the facts may have moved since the control was drawn. `No(reason)` goes to the status line and nothing else happens.
4. **Update.** The kernel opens a transaction, calls `update`, and commits (§13). The commit returns which top-level fields changed, which keys disappeared and whether the step goes into the undo history.
5. **Wake.** If anything changed, the kernel requests a frame. Several events between two frames each run their command immediately; the tree is built once.
6. **Build.** At the frame, the kernel pins each live value's latest snapshot, calls `view`, and gets a new tree (§5).
7. **Layout** in cells, reusing every subtree that didn't change (§6).
8. **Compare** with the last tree by id and collect the damaged cells (§7).
9. **Paint** the nodes that touch damaged cells into the kernel's own texture, then copy it to the window's surface and present (§7).
10. **After the frame.** The accessibility tree is updated with the nodes that changed, only while an assistive technology listens. The input method's caret is placed. Scope's ring buffer records the stage times.

A frame that only changes how something looks (the pointer moved onto a button, a press, focus moving) skips steps 6–8: the tree is the same, so only the nodes whose interaction state changed are repainted.

Frames come only from input, a commit, a live value, a finished job or derived value, a resize, or a change of appearance. The only timer in the loop is for dates written as how long ago, and it fires only when one would read differently (`docs/internals/shell.md`). The caret does not blink (spec §4.5: no time-based animation; idle is zero).

## 4. Facts

### 4.1 Plain values on the main thread

Facts are ordinary Rust structs. `self.items`, `item.title` and `self.draft` are read directly, with no lookups, so the view and `available` cost what a struct read costs. The app gets `&mut` access only in `update`; `view`, `available` and `start` take `&self`, so the type system guarantees that the view never writes back.

### 4.2 Two kinds of field, by how a change is seen

`update` may change the facts in two ways, and both are recorded without the app doing anything:

- **Containers** (`List`, `Map`, `Text`, `Selection`) record each change into the open transaction's journal as it happens: `push` writes an insert, `remove` a delete with the removed value, `Text` a splice with the removed text. The cost is the size of the change.
- **Plain values** (`String`, `bool`, numbers, enums, `Option<Key<T>>`, small structs) are **registers**. `derive(Facts)` generates a snapshot of the top-level plain fields, taken before `update` and compared after it; inside a list, `List::change` compares the one record it hands out. A difference becomes a `Set` operation. The cost is the number of plain fields at that level, never the size of a list.

A field's type must implement `ledger::Field`, which containers and plain values do. `Vec<T>` in the document part doesn't, and its compile error says to use `List` (through `#[diagnostic::on_unimplemented]`). In the session part a `Vec` is allowed and is a register: it is cloned and compared on every update, so it should stay small, and Assay's `update` budget says so when it doesn't.

### 4.3 Kinds are routing, not types

The four kinds of state (spec §3.2) don't change how a field looks on the main thread: a `Text` in the document and a `Text` in the session are the same type with the same operations. The kind decides where Ledger sends the operations:

| Kind | Journaled | Goes to |
|---|---|---|
| document | yes | the undo history, the CRDT on Ledger's thread, saving, sync |
| session | yes | the undo history (inside steps, §13.2), the session snapshot |
| live | no | nothing; written by another thread (§14) |
| resource | no | nothing; replaced as a whole, which bumps its version |

### 4.4 Containers know where they are

An operation must name what it changed. Each container carries a `ContainerId`, its path from the root (`items`, or `items/⟨key⟩/tags`), assigned by `Facts::attach` when facts are loaded, built from `states()`, or pushed into a list. `List::push` attaches the new record's containers. Records inside a list can only be changed through `List::change`, which re-attaches what it handed out, so nothing escapes.

A container replaced by assignment (`self.items = List::new()`) is the one change that does escape: its operations would have been recorded by its own methods, and the new one never had them called. After `update`, each `List`, `Text` and `Selection` compares the path it was attached at with the path the derive reaches it by (one comparison per container), and a mismatch means it was replaced. Recording the replacement would need the old contents, which are gone by then, and keeping a copy before every command would cost each command as much as the list holds; so the kernel re-attaches the facts and says it as a failure in the message line ("“Open” replaced a List<Block> by assigning a new one, which undo can't take back; change it with its own methods, like replace or set"), and Assay reports it. `List::replace` and `Text::set` are the recorded ways to replace everything.

### 4.5 The transaction is per thread

`update(&mut self, cmd, cx)` has no handle that `self.items.push(...)` could write to, and the api shouldn't grow one. So the open transaction lives in a thread-local slot that the kernel sets around `update` (and Ledger around undo, redo and incoming operations). Outside a transaction, containers change without recording anything: that is how `states()`, `Todo::sample` and tests build facts. The type system already confines `&mut` facts to `update`, so there is no path by which a real change escapes the journal. This is safe Rust (`thread_local!` with a `RefCell`).

### 4.6 Paths

`derive(Facts)` generates one associated constant per field, `Todo::draft: FactPath<Todo, Text>`, named after the field so that app code reads `Todo::draft`. It holds a `PathId` (stable, from the field's name), the kind, and two function pointers that read and write the field. Fields and associated constants live in different namespaces in Rust, so `self.draft` and `Todo::draft` don't collide; the constant carries `#[allow(non_upper_case_globals)]`.

Paths compose for nested fields: `Todo::items.at(key).then(Item::title)` is a `FactPath<Todo, String>` through a key. An erased path (`ErasedPath`) is a short inline array of steps (field, key), at most eight, so it is `Copy` and never allocates.

**The rule for the view: a path where the kernel writes, a reference where it only reads.** A field writes its text, so it takes `Todo::draft`; a list writes its selection, so it takes `Todo::selection` (§16 changes `api.md` here); a list only reads its items, so it takes `&self.items`.

### 4.7 Versions

Each top-level field has a version, a `u64` kept by the kernel beside the facts (not inside them), bumped when a commit's operations touch that field, when a live value publishes, when a resource is replaced, or when a derived value arrives. Derived values compare versions to decide whether to recompute (§14). Nested versions (one per record field) wait until a derived value needs them.

## 5. The tree and its ids

### 5.1 Storage

A `Tree` is a `Vec<Node>` in building order with parent, first-child and next-sibling indices, plus three side arenas reused every frame: text (one `String` that labels and values are copied into), commands (a `Vec<C>`) and subjects. The kernel keeps two trees and swaps them each frame, so the last tree is always there to compare with and to hit-test against. In steady state no frame allocates: the vectors keep their capacity.

A `Node` holds its `Id`, its component (`enum Component { Page, Row, Columns, Column, List, ListRow, Field, Check, Link, Button, Text, … }` with each one's inputs), its layout inputs (width, `Fill`, minimum, maximum), its `Area` after layout, a hash of its own content, a hash of its subtree, the index of its command if it has one, its subject, and the source location that created it.

### 5.2 Ids

Every `Ui` method is `#[track_caller]`. A node's id is a hash of its parent's id, the call site (`Location::caller()`: file, line, column) and either the row's key or, for repeated calls from the same site under the same parent, the count of earlier ones.

**Proposed: call site, not position.** With position alone, an `if` that adds a component above shifts the id of everything after it, so carets, scroll positions and focus jump to the wrong component. With the call site, only components created by the same line can be confused, and those are counted. `Location` is a static that costs nothing in release builds, so ids use it always; development builds also keep it in the node for Scope (spec §3.5, "Code" in §13.6).

A field's id is its path rather than its call site, so its caret follows the fact it edits, not the line of code that shows it.

Two nodes with the same id in one frame are a bug in the kernel; a law checks it (§19).

### 5.3 Building

The view is called once per frame that needs it. `Ui<'a, C, S>` holds the tree being built, a reference to the facts (as `&dyn Any`, downcast by each path; the facts' type is always the app's), the scroll positions from the last frame, and the current subject. It is generic only over the command type: labels and availability are filled in by the runner after the view returns, because the runner knows the app's type and `Ui` doesn't need to. Hover, press and focus are not in `Ui` at all; they repaint a node without building the tree again.

Each method pushes a node and returns a small builder that refers to it by index, so `ui.field(…).on_submit(Cmd::Add)` changes the node in place, with no drop logic and no allocation.

A command shown on screen is asked `available` once per build of the tree, which happens only after a commit or a scroll, because `available` is a pure function of the facts.

### 5.4 Lists build only what shows

`ui.list(&self.items, …)` calls the row closure only for the rows that fit the list's height in the last layout, plus one. If layout finds that the height changed (a resize, or the first frame), the kernel runs the view again before painting, at most once a frame; the view is pure, so running it twice is safe. The list scrolls by whole items: its position is the key of the item at its top, with that item's index kept as a fallback for when it is removed, so items arriving above never move what you're reading. A row taller than the whole list is shown cut at the list's bottom rather than left out. Scrolling by lines inside a tall row, and a scrollbar estimated from measured row heights, come with the components that need them (build-plan item 4).

This is what keeps a 10,000-item to-do list at the same frame cost as five items.

A table is a list whose first child is a row of heads. The heads come from calling the row closure once more, for the top row shown, with the row builder making heads instead of cells, so the app names its columns in one place; an empty table has no heads. The order a table shows its rows in is not the kernel's: it is a derived value (§14) the app works out from the list and a `Sort` fact, which a head's press sets by an edit. The kernel only reads it, by key and by place, both for drawing and for the keys that move and choose, so sorting a million rows never runs on the main thread. A list shown in an order scrolls by place rather than by the key at its top, since after sorting again the key at the top is somewhere else. Column widths the user dragged are the window's, by the id of each head, which follows the head's name.

A tree is the same list again. Its items are one flat list, each naming the item it is inside, and which branches are open is a fact the app keeps; `Order::tree` walks them into rows on another thread and gives each row its depth, whether it is an open or closed branch, and the place of the row it is inside. Each row's first child is a twig, indented by its depth, whose press sets the open branches by an edit with its key added or taken out. Right and Left are kernel commands with the platform's keys for trees: they press the focused row's twig, or move the selection into an open branch or up to the row it is inside, both read from the order rather than from what is built.

## 6. Layout in cells

### 6.1 Units

All layout is integers: `Area { col, row, cols, rows }` in `i32` cells, moved in from Feedreader's `grid.rs`. There is no pixel type anywhere in layout, so code that places a box can't leave the grid (spec §4.1). The shell converts an `Area` to device pixels by multiplying by the cell size, which is whole device pixels at every integer scale.

### 6.2 Two passes

1. **Measure**, bottom-up, given a maximum width in columns: each node returns its minimum and preferred size in cells. Mono text measures by counting characters (wide characters count two, spec §4.1). Sans text is shaped (cached by text and style) and rounded up to whole cells. Text that wraps reports the rows it needs at that width.
2. **Place**, top-down: each container hands its children `Area`s.

The containers the to-do app needs:

- `page`: the app's area inside the window's own chrome (title, action bar, status line, message line, which the kernel draws around every app).
- `row`: children left to right; widths from their minimums, `Fill` takes the rest.
- column (the default inside `page`): children top to bottom. A list row lays its components left to right, like a `row`, because a row of a list is read across: the to-do row is a checkbox and a link side by side.
- `columns` and `column(width)`: fixed widths, `Fill`, `.min()`, `.max()`.
- `list`: a scroll area of rows (§5.4).

### 6.3 The narrow form

When a `row` or `columns` can't fit its children's minimums plus the gaps, it stacks them top to bottom. That is the stated narrow form of every container, so nothing is cut off silently (spec §4.1). The window's minimum size is the layout's minimum in that stacked form, so the window can't get narrower than the narrowest layout that still fits.

**Proposed:** long text in a row (the "Long titles" state) wraps onto more rows of the same row, rather than being shortened, because the taste laws forbid clipping. Paths are the exception: they shorten in the middle (spec §7.3).

Two containers hide children instead of stacking them, and both say so. Breadcrumbs hide levels from the middle, keeping the top and the current one, and draw "…" where they were. An overflow row (and the places along the top, which are one) hides its actions from the end and shows its last child, "More…", only when something is hidden. Layout decides what is hidden by giving it an empty area; nothing else is told. Activating "More…" opens the drawn menu with an entry for each sibling whose area is empty, pointing at that sibling's id, so choosing an entry activates the hidden control as if it had been clicked: its command, its edit or its reason for being unavailable.

A split stacks too, but its dividers go with it: a divider between panes stacked one above the other would move nothing. Stacked panes keep their own heights while they fit, and share the split's rows evenly when they don't.

The width a user gives a pane or a table column is the window's, like where a list is scrolled: the runner keeps it by the node's id and copies it into the tree before the view runs, so the view reads it as the pane's starting width and stays a function of the facts and the window. It is not undone and is kept with the rest of the window's whereabouts. A divider resizes the pane with a fixed width beside it, the left one unless only the right one is fixed, and never past the other pane's minimum; the keys read the width the user last gave, since between a key and the next layout the laid widths are a frame old.

### 6.4 Reuse

A subtree whose subtree hash and maximum width are the same as last frame keeps its layout; only its offset can change. A change in one row re-measures that row and its ancestors, not its siblings. A full layout happens on resize, theme or style change, and on the first frame.

## 7. Comparing trees and repainting

### 7.1 What changed

After layout, each node of the new tree is matched with the node of the same id in the last tree: first by walking both in order (the common case, since most frames add or remove little), then through an id-to-index map (reused, cleared, not reallocated) for whatever didn't line up. A node is damaged if it is new, gone, moved (its `Area` changed), or its own content hash changed. Its damage is its old `Area` plus its new one. Interaction changes (hover, pressed, focus) damage the node directly, without a rebuild.

Damage is a short list of cell rectangles, merged into bands of whole rows. Toggling one item damages one row; moving the cursor in a list damages two rows.

### 7.2 Painting from a kept list of quads

The window's surface textures are not guaranteed to keep their contents from frame to frame, so every frame must fill the whole surface. **Proposed:** the kernel keeps the frame as a list of quads in a GPU buffer (Feedreader's quad pipeline: glyphs and character graphics from the atlas, fills, lines), with each node owning a range of it. Each frame:

1. Walk the tree, skipping every subtree whose `Area` misses the damage, and rewrite the quad ranges of the nodes that hit it. A node whose quads no longer fit its range gets a new range; freed ranges are reused.
2. Upload only the changed ranges.
3. Let the GPU draw the whole list into the surface and present.

CPU work is proportional to the damage. The GPU redraws everything, which is what GPUs are for: a full screen of text is tens of thousands of quads. This replaces an earlier draft that painted the damage into a kept texture of the whole window and copied it to the surface each frame. At 5K that texture is about 60 MB, so the copy reads 60 MB and writes 60 MB every frame, while drawing the quads writes the surface once; on Apple's tile-based GPUs, overlapping quads are resolved in on-chip memory, so drawing everything costs about half of copying. Boxes on the grid don't overlap, so drawing order matters only for overlays, which are a second list drawn after the first. S3 measures the GPU time at 5K on the M1.

A further step, which the owner asked to have measured: most of a Graticule screen is cells, so the grid's frames, rules and mono text could be kept the way terminals keep theirs, as one small record per cell (a character, its inks, its style) that a single shader turns into pixels, with quads only for what isn't cell-shaped (Ghostty's renderer, MIT-licensed, is the model to learn from).

**Measured in S3, on the Windows development machine, release build, a 5K window at 2x (5120 × 2880, 263 nodes, 5,054 quads):** building the tree 0.05 ms, layout 0.08 ms; a frame where one item was toggled repaints one node and uploads 16 quads (1 KB) in 0.006 ms; a frame where nothing changed paints nothing; the first full frame paints 253 nodes in 0.9 ms and uploads 393 KB. The kept quad list is the renderer. A cell buffer could only win on GPU time, which at this size is one instanced draw per layer; that comparison, and the GPU time at 5K, need the M1 reference machine and are still open in the build plan.

The quads are kept in two layers, under and over: a list row's selection and hover fills go under, everything a component draws of itself goes over, because a row's fill must lie beneath its children's text even when the children's ranges come first in the buffer. Each node owns a range in each layer; a node that changed rewrites its range, or moves to a bigger free one, and blanks what it no longer uses; a node that is gone frees its ranges. Only the span between the first and last changed quad is sent to the GPU. When the glyph atlas fills up and starts over, every kept glyph quad points at the wrong pixels, so the kept list starts over with it.

### 7.3 Anchored overlays

An overlay (spec §4.4) is a node of kind `Overlay` whose anchor is the node that opened it or the cell the pointer was on. It and everything inside it are marked `over` when the tree is built. Layout leaves overlays out of their parent's flow, so they take no room, and places each one after the rest of the tree: as wide as its content wants plus a column each side, under its anchor if it fits, else above, else wherever the window has room, and never past the window's edges. Kept quads have two more layers for them, under and over again, drawn after the page's two, so an overlay covers whatever it lies on however the buffer ranges fall. The hit cells are filled twice, page first, then overlays; an overlay's own box marks its cells as covered, so a click on its background reaches nothing beneath. It is painted as a box on the grid with the style's hard shadow, like a button.

### 7.4 The mark after a change you didn't watch

The runner keeps the keys of rows changed by something other than the user's own command: each slice of an undo or redo, and the commands that come back from jobs. A key is taken from an inserted item, or from the first key on the path of a changed field; removals leave nothing to mark. The look of a list row whose key is kept says `marked`, and the row is drawn with a bar at its right edge in the link ink. The next key, click, scroll or input method event forgets them all before it is handled, so an undo pressed with a key marks what it changed and the following input clears it. The mark's look is a maintainer's choice.

### 7.5 Dragging

Two kinds of drag come into the runner, and both say in the status line what letting go would do, as the command's label and, when it can't run, why.

Files dragged from the system arrive from winit as one hover event per file, then a drop per file, or a cancel. The tree keeps the nodes that take files with the command a file makes (`ui.on_files`); the target is the innermost of them that holds the node under the pointer, or the only one there is. Over the window the system owns the drag, so on Windows and macOS the pointer isn't reported while files hover and the target is decided where the pointer last was; an app with one target never notices. Each file runs its own command, through `available` like any other.

A row is dragged within its list when the list has a mover (`on_move`). A press on the row lifts it; nothing moves until the pointer reaches another row of the same list, so a click is still a click. The place it would land is counted in the list before the move: past a row below it, or at a row above it. That row's look says `landing`, above or below, and is drawn with a line in the ink across its edge. Letting go runs the mover's command through `available`, Escape puts the row back, and `List::move_to` makes the move one change that undo takes back whole. Only the pressed row moves, not the whole selection; carrying several chosen rows together is left until an app needs it. The tree keeps the mover as a boxed closure, which costs nothing when the app passes the command itself, as it should.

Dragging out of the window is not possible with winit, which can receive a drag but not start one.

### 7.6 Textures painted off the main thread

Canvases, raster images and paint hooks each draw into a texture of their own, and the main thread only places it: one quad of kind 2 naming the texture, in the node's own range like any other quad. What goes into the texture is worked out on the presenter thread, which already owns the GPU, just before it draws the frame (`Gpu::settle`): a canvas's drawings are rasterized there with zeno into straight-alpha RGBA, an image is uploaded there with its mip levels, and a paint hook runs there. Painting a node only asks for this, through the text store that already carries the atlas's uploads, and asks only when the stamp of what decides the pixels changed: the drawings' hashes, the box's size, the weights and the theme's inks. A drawing works out its hash once, so a drawing made in a job costs the view nothing however many shapes it holds.

The GPU draws quads in runs that share a texture. The kept layers know their runs too, worked out again only when a layer changed. A canvas's texture is let go of when its node leaves the kept list; an image's lasts as long as the app holds the image, which the presenter thread sees through a weak reference.

A node's hash takes in what it draws from the tree's side tables, a color slider's color as much as a canvas's drawings, since the kept list repaints a node only when its hash changes.

A law (§19) checks that patching the quad list gives the same pixels as building it from scratch. It compares the quads themselves, layer by layer, which is enough because boxes on the grid never overlap.

## 8. Hit testing

After layout, the kernel fills a grid of the window's size in cells, each cell holding the index of the topmost node with an interaction there (a button, a check, a link, a field, a list row). It is refilled after every layout: at 200 × 60 cells that is 12,000 writes, a few microseconds, so tracking damaged cells isn't worth its code yet. Finding what is under the pointer is one array read, well under the 1 µs budget. Hit targets are at least one cell, which the taste laws require anyway.

A hit gives a node; the kernel walks up to the nearest node that acts (a check's command, a field's focus, a row's selection) and collects the nearest row's subject on the way. A canvas gets the point inside its box and tests its shapes against their outlines with zeno, the topmost first; a stroke counts a little around it and a filled shape a little past its edge, so thin lines and small dots can be hit. Only shapes with a command to press are hit, and the one hit is the one the arrows have reached too (`aim`), so a click and the keyboard leave the canvas in the same state.

## 9. Commands

### 9.1 What `derive(Commands)` generates

For `enum Cmd`, an `impl<F> Commands<F> for Cmd where F: Find<Item>` (one bound per subject type) with:

- a name per variant, from the variant (`Remove` → `remove`), used by keymap files, the command line and the log;
- a label function: `#[label("Remove “{title}”")]` reads `title` from the subject, found through `Find<Item>`; `#[label(done: "…", else: "…")]` chooses by the subject's `done` field; `short: "Remove"` is what a control shows inside a row, where the row already shows its item, while its accessible name stays the full label (the owner's decision). A name in braces is looked up first among the variant's arguments, then among the subject's fields; a name that is neither is a compile error;
- which argument is the subject, and its type;
- the other arguments of every command the user can reach, each with its name (the field's, with underscores as spaces, or none in a tuple variant), its kind and how to read it from text, through the `Argument` trait; and `make`, which rebuilds any such command from its subject and those values. `Argument` covers `String`, `bool`, the whole numbers (`i32`, `i64`, `u32`, `u64`, `usize`, read with or without the commas `grouped` writes) and `f64`; a type that isn't one is a compile error at the command, so a command can't ask for something the palette can't read. A command with arguments shows as its name and an ellipsis ("Rename…") in the bar, the palette and which-key; running it from any of them, or from a key, opens the palette asking for each argument in turn under the command's name, the argument and its kind, and Return reads it. A value that doesn't read is said in the message line and the palette keeps asking; Escape gives up. With every value read, the command is built with its subjects as it would be without arguments, so the selection, counts and `available` work the same;
- which variants are `#[internal]`, and which one carries `Edit` (the variant whose only field is `Edit`), so the kernel can wrap edits;
- encoding and decoding for the command log.

### 9.2 `available`, `update`, `Outcome`

- `available` is called for commands on screen, once per frame that built them and cached until the next commit, and right before running any command. The palette and which-key ask only while open. Assay measures its time.
- `update` runs inside a transaction. If it panics, the transaction is rolled back with the inverses the journal recorded, the message line says what failed, and the app keeps running.
- `Outcome` is `Changed` or `Nothing`. Ledger's containers return it (`push` → `Changed`, `change` and `remove` on a key that is gone → `Nothing`), and `Cx`'s messages return `Nothing`, which is what lets each arm of the to-do's `match` be one expression. The journal is the truth: a commit with operations is a change whatever `update` returned. Development builds check that the two agree, and Assay does too.

### 9.3 `Cx<C>`

`cx.key()` (a new `Key<T>`, inferred from where it goes), `cx.now()`, `cx.random()`; the effects (§12); `cx.say(message)` and `cx.say_failure(failure)` for the message line. Everything non-deterministic that `Cx` hands out is written into the command's log entry, so a replay gets the same keys, times and numbers back (spec §3.3).

### 9.4 Kernel commands

Undo, redo, focusing the next and previous control, opening the palette, cancelling, and the list's movements (up, down, extending up and down, first, last, choosing everything) are the kernel's own commands, in `builtin.rs`. They sit beside the app's commands, so they show in the palette and which-key, have labels and say why not ("Nothing to undo"); a keymap file can bind them by name (`palette`, `undo`, `next-control`, `extend-down`). The platform's conventions bind them (Cmd-Z, Tab, the arrows in a list, Cmd-A, Escape); everything else is bound only by keymap files. Copy, cut, paste and select all inside a field are the field's own keys (§11.2).

An app's keymap is `keymap.txt` beside its `Cargo.toml`, which `derive(Commands)` builds into the app. One binding a line, `keys: command`; a sequence is chords separated by spaces (`d d: remove`); a chord is modifiers and a key joined by `+` (`primary+shift+p`), where `primary` is Cmd on a Mac and Ctrl elsewhere. The user can change them: `keymap.txt` in the app's folder (beside its saved facts), when it is there, replaces the built-in keymap. "Edit the keys" (`edit-keys`) writes the built-in keymap there the first time and opens it with the system's editor. The runner watches that one file through the system's notifications, so it costs nothing while the file stays the same; on a change the waker wakes the window and the keymap is read again. A line the kernel can't read keeps the keys that were in use and says the mistake in the message line; a name that is no command is said too, and the rest of the file still applies. Deleting the file brings back the built-in keys. A window without a folder (Specimen's samples) has no file, and "Edit the keys" says so.

A **count** is digits typed before a binding, as in a modal editor: `3 d d`. A digit counts only where the keymap binds nothing to it, so an app can still bind a digit, and a count can't start with 0. While counting, the bar shows the which-key tree with the count in front. For a command with a subject, the count takes that many rows from the focused one down as its subjects, so it needs a `many` label and runs as one transaction under it ("Remove 3 items"); with several rows chosen already, or for a command that takes one item at a time, it is unavailable and says why. A command without a subject runs that many times as one transaction and one undo step, labelled "“Add” 3 times"; `available` is asked before each time, and when it says no the rest are dropped and the message line says after how many and why. A replay asks the same questions of the same facts, so it stops in the same place. The kernel's movements, undo and redo, the next control and the next theme or style repeat (`Builtin::repeats`); the rest run once. A count works with the platform's own keys too: `3` then Down moves three rows. Escape, the palette or a click let go of it.

**Modes** make a keymap modal. A line `[normal]`, `[insert]` or `[select]` starts the bindings of that mode; lines before any of them bind in every mode, and a keymap with none has no modes and works as before. A modal app starts in normal mode, and the built-in `normal-mode`, `insert-mode` and `select-mode` switch (unavailable, saying why, in an app without modes or in the mode already). Only insert mode lets typing and the field's own keys reach a focused field, and only there do Return and Space press a focused control before the keymap; in the other modes the keymap goes first and a focused control takes Return or Space only when nothing else does. In select mode the list's up and down extend the choice. The bar starts with the mode, as a typed mode value, so the user always sees which one is on. The mode is the window's, like the pending keys: not a fact, not undone, not saved.

An **operator** is a binding that ends in `motion`: `d motion: remove`. After its keys, the next press is read as a movement through the bindings of the mode and then the platform's keys (up, down, first, last and the extending ones), and the command takes every row from the focused one to where the movement lands as its subjects, as one transaction under its many label. A count multiplies the movement: `2 d j` takes three rows. A press that is no movement says so; a command without a many label can't be an operator and says why. Which-key shows `d motion` beside the command. `d d` beside `d motion` wins for the second `d`, as the exact binding. Operators act on lists for now; moving through text by words and lines comes with the fields in full (item 15), and those movements become ones an operator can take.

A key goes, in order, to the palette when it is open; to the focused field, which takes typed text and every key its editing model knows, even one that changes nothing, so Delete at the end of a field never removes an item; to a focused button, link or checkbox for Return and Space; to the keymap, where the first chord of a longer sequence waits and the action bar shows what can follow (which-key) until the sequence ends or Escape; and last to the conventions.

### 9.6 The help page and what's this

`derive(Commands)` keeps each command's `///` comment as its documentation (`Entry::doc`). The built-in "Show the help" (`help`) draws the registry in place of the app's page: every command the user can reach with its label as it reads here, its keys in the current mode and, when it can't run, why, followed by its documentation; then the keymap as a tree, one line per binding sorted by its keys, with a "More…" line for each shared prefix and the bindings under it indented; then Graticule's own commands. It is built by the kernel through the same `Ui` as any app, after the app's view has run, so labels and reasons are the ones the bar would show, and then the app's nodes are cleared. Pages don't scroll yet, so the help shows a window of lines: the arrows, Page Up and Down, Home and End move it while it is open, and so does the wheel; its title says which lines show. Which lines fit is reckoned from the window's rows and each line's length, so a reason that wraps, or columns stacked in a narrow window, take their room, and the last line always shows at End. Escape or the same command closes it.

The built-in "What's this?" (`whats-this`) makes the next click, or the next Return or Space on a focused control, describe instead of act: for a command, its label, its keys, its documentation and whether it can run now or why not; for anything else, what it is and that it has no command. The description goes to the message line, which the screen reader reads. Escape stops without describing.

The built-in "Show what was done" (`log`) draws, on the same kind of page, what the user did in this window, newest first, each with when as a date value (so it follows the date setting): every change that became an undo step under its label, every undo and redo ("Undid “Add”"), and every switch of the look. Typing that joins a step is not logged again, and moving around is not logged. The window keeps the last 500 in memory and doesn't save them; the command log in Ledger is the record that lasts.

### 9.7 Settings

Settings are derived from the app's data (spec §8.3). `derive(Form)` on a struct of facts makes each field marked `#[label("…")]` one setting (`kernel::Settings`): its label, its path inside the struct, its value now and its value in the struct's `Default`, and the control its type asks for (`kernel::Setting`): a switch for `bool`, a stepper for the whole numbers (stopping at the type's bounds, with the reason), a field for `Text`, and a row of options for an enum with `derive(Choice)`, each option labelled by `#[label("…")]` or its name in words. A type that is none of these is a compile error at the field.

`ui.form(Player::playback)` draws them top to bottom. A switch, an option or a step is pressed like any control, and the kernel sends an `Edit` setting the new value at the field's path, so it arrives in `update` like every edit, is one undo step and is logged under how it reads ("Speed: Double", "Volume: 51"); the field types like any field. The values the controls set live beside the tree's commands (`Tree::sets`). "Show only the changed settings" narrows every form to what differs from its defaults, and "Show every setting" brings the rest back. A form in `#[session]` facts changes without undo steps, as every session change does. Searching a long form waits for the typed filter field of item 15, and so do a slider for fractions and percentages.

### 9.5 Bindable everywhere

A component takes a command in one of two forms, chosen by type:

- a complete value, `Cmd::Add`;
- a constructor waiting for its subject, `Cmd::Toggle`, which is a `fn(Key<Item>) -> Cmd`.

The kernel has `trait Bind<C, Marker>` with `impl<C> Bind<C, Whole> for C where C: Commands<_>` and `impl<C, S, F: Fn(S) -> C> Bind<C, Subject<S>> for F`. The marker parameter keeps the two from overlapping, and the compiler infers it (Bevy's systems use the same trick). In a row, the kernel calls the constructor with the row's key while building and stores the finished command in the node.

## 10. Subjects and selection

### 10.1 Subjects are typed

`ui.list(…, |ui, item| …)` hands its closure a `Ui<'_, Cmd, Key<Item>>`. `Bind<C, Subject<S>>` is accepted only where the `Ui`'s subject type is `S`, so `ui.link(Cmd::Open)` inside an `Item` row (where `Open` wants a `Key<File>`) is a compile error, not a wrong command at runtime. Outside any row, `S` is `()` and only complete commands bind. Rows of rows take the innermost key.

### 10.2 Where a subject comes from

**Proposed**, following the platforms' behavior:

| Started from | Subject |
|---|---|
| A control inside a row | that row, always one |
| A context menu (later) | the row under the pointer; if it is in the selection, the whole selection |
| A keymap sequence, the palette, the menu bar | the focused list's selection if it has more than the focused row; otherwise the focused row |
| Nothing focused or selected | none: the command is unavailable, and says "Nothing is chosen" |

A command started on several subjects runs `update` once per subject, in list order, inside one transaction, so it is one undo step. Gathering the subjects in list order costs as much as the selection holds, so it happens only when a command runs; the bar, which-key and the palette are built from the chosen count and the focused row alone (`Runner::glance`). The owner decided (§21) how it reads and what happens when some subjects can't take it:

- A command runs on several subjects only if its label says what it is called then: `#[label("Remove “{title}”", short: "Remove", many: "Remove {count} items")]`. Without `many`, the command takes only the focused row, so no command shows a label nobody wrote.
- Subjects that `available` refuses are skipped, the others run, and the message line says which were skipped and why ("Remove “Call mum” was skipped: That item is gone").

### 10.3 Selection

`Selection<T>` is a set of keys plus the focused key, in the session part of the facts, defined by Ledger. Clicking, Shift-clicking, Cmd-clicking, the arrows and Cmd-A in a list change it, following React Aria's list behavior. The kernel sends each change as an `Edit` on the selection's path, through the app's `update` like every other change to the facts (§11), so an app could keep an invariant on its selection in the same place it keeps one on its text.

After every commit, Ledger removes the keys that the commit's operations deleted from every `Selection` of that key's type, locally or arriving through sync. Each removed key goes with the type of record its list holds (`Field::item_type`, found by following the operation's path), so a selection over another list keeps an equal key. The cost is the number of deleted keys.

When a command removes the focused row, the kernel focuses the row now in its place, as a selection edit after the command, so the keys keep working on the next item. It knows the place because it remembers where the focused row was each time it builds the list.

Changing only the selection is not an undo step (spec §13.2), but every step records the selection and focus from before it, and undoing restores them.

## 11. Edits through `update`

### 11.1 The one door

The kernel changes the app's facts in exactly one way: it sends `Edit` through the app's `update`. An `Edit` holds an erased path, a change and a gesture id, boxed so that it is one pointer wide and an app's command enum stays small:

- `Change::Splice`: replace a byte range of a `Text` with a string;
- `Change::Select(…)`: set, extend, toggle, focus or clear a selection;
- `Change::Set(Value)`: a form's value (later).

`edit.apply(self)` dispatches through `Facts::edit`, generated by the derive, and returns the `Outcome`. An app that wants to validate a value or keep an invariant reads `edit.touches(Todo::draft)` (or `File::body`) and does it there, which is how Galley's editor keeps `edited` current. An app can refuse an edit by not applying it; the field then keeps showing the facts.

### 11.2 Typing, step by step

1. A key reaches the focused field. The field's editing model (Feedreader's `field.rs`, moved in with the macOS key bindings and its law, §18) computes the splice and the new caret from the current text.
2. The kernel sends `Cmd::Edit(Edit { path: Todo::draft, change: Text(splice), gesture })`.
3. `available` says `Yes`; `update` calls `edit.apply(self)`; `Text` journals the splice.
4. The kernel gives an edit the gesture id of the one before it when it is the same field, the same kind (typing or deleting) and no word boundary has passed; moving the caret, pasting, cutting and anything else start a new one. On commit, Ledger merges steps with the same gesture id, so a word is one undo step. A slider needs no merging: while it is dragged the kernel only draws where the handle would land, and sets the number once, when it is let go.
5. The kernel moves the caret to where the field computed, or clamps it if the app didn't apply the edit.

The input method's unfinished text (preedit) is the kernel's, not a fact; only committed text becomes an edit. Return in a single-line field runs the field's `on_submit` command.

### 11.3 The caret belongs to the field

The caret and the field's own text selection are state that only the field needs, so they are the kernel's, kept by the field's id, which is its path (§5.2), as spec §3.2 says of component state. When a `Text` changes under a caret (a command's `take()`, an undo, sync), the kernel moves the caret through the splice. Undo steps record the caret with the focus, so undoing typing puts the caret back. `api.md`'s sentence that `Text` is "a value with a caret and a selection" changes accordingly (§16).

### 11.4 Drafts: number and tag fields

A number field edits a whole number and a tag field adds to a list of words, not a `Text`, so what a person types (`12*4+3`, or half a tag) has nowhere to go in the facts. The kernel keeps it as the field's draft, by the field's id, beside the caret, and draws it in place of the field's text. Nothing in the facts changes while typing, so undo never sees a half-typed sum. Return, an arrow in a number field, or leaving the field commits the draft by one edit; Escape drops it. A draft that doesn't work out to a whole number within the bounds, or a tag that is there already, is said and stays for the person to fix.

Leaving means the focus moving: Tab, a click in another field, automation focusing something else. Drafts are settled after every key, pointer event and accessibility action. Pressing a button or a link does not move the focus, and does not commit a draft either: committing it then would make one click two undo steps, the draft and the button's command, and an undo would take back only the second. A draft whose field is no longer in the tree, because the page it was on is gone, is dropped.

### 11.5 What is open, and what a calendar shows

A select's options, a popover's controls and a confirm row's question are open or not, and that is the window's state, not a fact: opening one is no edit and no undo step, and a reopened app shows everything closed. The runner keeps one `opened` id, so at most one thing is open, and copies it into the tree before the view runs, as it does the focus; the view builds what is inside only while `tree.opened` names it, so a closed select costs one node. Opening one moves the focus to its chosen option or first control. It closes on Escape, on a press or accessibility action outside it, and when the focus moves out of it after any key; a select of one closes when an option is taken, and a confirm row when its command runs. When it closes with the focus inside, the focus goes back to what opened it.

A combo box's completions use the focus instead of `opened`: the view builds them while the field or one of them has the focus, read from `tree.focus`, so leaving the field is all it takes to close them.

Controls in a list row are not tab stops of their own, since the list's keys reach them (§10.3). An overlay and an open question each start a scope of their own: `row_of` stops at them, so their controls take the focus even when what opened them sits in a row, as everything in Specimen's pages does.

A calendar shows the month of its chosen day until it is turned, by its buttons or by the arrows moving past the month's edge. The month it shows then is kernel state kept by the calendar's id, like a scroll offset, copied into the tree before the view runs; taking a day drops it, so the calendar follows its fact again. A day's id is made from the calendar's id and the day, so the runner can focus the day an arrow lands on before the view has built it. Of a calendar's 42 days only one is a tab stop: the focused one, else the chosen one when it shows, else the month's first.

Date and time fields keep a draft as number fields do (§11.4), read when Return is pressed or the field is left; the arrows read the draft, step it and set it at once.

A color is a struct of facts, not a value of its own, so a color slider is an ordinary slider on `path.then(Oklch::h)` and needs no code of its own in the runner. What it adds is paint: the tree's `shades` table names the sliders that set a part of a color, with the color and the part, and the slider's track is drawn from it. The same table gives the slider's keys the exact whole number, since its shown text is rounded for people. A color area is a control of its own because it sets two parts at once; while it is dragged the runner keeps where the mark would land (`mixing`), the way it keeps a slider's, and copies it into the area before painting, so the facts change once, when it is let go.

A menu button's entries are its children, laid out with no area, as an overflow's hidden ones are; pressing it opens them in the drawn menu (§7.3) with their keys and reasons, and Tab passes over them.

## 12. Effects

`cx.save_as("To do.md", Todo::as_markdown, Cmd::Exported)` pushes `Effect::SaveAs { name, write, reply }` into `Cx`. `write` is a `fn(&F) -> String` that makes the file's contents, and `reply` is a `fn(Result<Path, Failure>) -> C`, which `Cmd::Exported` is.

After the commit the kernel runs effects in order, in the same frame, once it has the window (`Runner::painted`): the save panel opens as a sheet on the window (rfd's asynchronous panel), without blocking the event loop; a job thread waits for the user's choice, and if they call it off, nothing comes back; a job takes a snapshot of the facts (§14), runs `write` on it and writes the file, shown with its progress in the status line, so exporting fifty million items never touches the main thread; the result comes back through the event loop as `Cmd::Exported(result)`, which goes through `available` and `update` like any command. `#[internal]` keeps it out of the palette, menus, keymaps and undo.

`cx.write(path, make, reply)` is the same without the panel, for a path the app already has: the job makes the folder if needed and writes there. `make` may be a closure, so it can pick out the part of the facts it writes.

The command log records the result command, not the effect, so a replay never opens dialogs or writes files: it feeds the logged result back in. Opening a link or file hands it to the system's opener (`open`, `explorer`, `xdg-open`); copying goes to the clipboard. Because a job takes a snapshot of the facts to another thread, `App` requires `Send`; facts are plain data, so this only rules out `Rc` and `RefCell` in them. Specimen runs `update` on a copy and shows the effects it collected instead of running them (`Runner::hold_effects`): Specimen (`specimen::Specimen`) keeps a runner for its own app (`specimen::Catalog`, which draws the Components and Style places with the same components as any app), one for the Changes place (`specimen::Changes`, see `docs/internals/proofs.md`) and one runner per named state of the app it was opened over, each with its own copy of the facts, history and focus, nothing saved, and hands the window to the one showing. It moves between places and states through host commands (`runner::Extra`): "Components", "Screens", "Show “Empty”" and so on, which every runner lists in its bar and palette beside its own, so the places are always on screen and Specimen needs no keys of its own and no second tree. Scope lists every effect with its timing.

## 13. How Ledger plugs in

### 13.1 The contract

The kernel runs a command and hands the result to Ledger's history:

```rust
let before = facts.snap();
let transaction = Transaction::begin();
app.update(cmd, &mut cx);
let mut ops = transaction.end();
facts.diff(&before, ErasedPath::ROOT, &mut ops);
let kept = history.commit(ops, label, gesture, place);

let moved = history.undo(&mut facts, place);
let moved = history.redo(&mut facts);
```

The transaction collects what Ledger's containers recorded while `update` ran; `diff` adds a `Set` for every plain field that changed. `label` names the step for the Undo command ("Undo “Add”", "Undo typing"), `gesture` joins the typing of one word into one step, and `place` is where the user was (the focused id and the caret), which `undo` and `redo` hand back so the kernel can put the user there again. `Kept` says whether the commit became a step, merged into the last one, only moved the selection, or kept nothing.

If `update` panics, the kernel applies the inverses of what was recorded so far, the message line says the command failed and that nothing changed, and the app keeps running. After a commit that removed keys, the kernel asks the facts to drop them from every selection (`Field::prune`), inside a transaction of its own whose operations join the step, so undo puts the selection back.

Saving, the command log and Ledger's thread come next in S5, with `Ledger::open` loading the facts and the kernel's interaction state before the first frame.

### 13.2 What Ledger does with a commit

- **Registers.** Compares the plain fields (§4.2) and adds `Set` operations to the journal.
- **Steps.** A commit is an undo step if it changed the document, or typed into any field. A step keeps every operation of the transaction, document and session alike, each with its inverse, plus the focus from before and the selection moves made since the step before it. Commits that only move around (selecting, opening a file, switching places) are not steps; their selection moves are kept until the next step. Undo takes back the moves since the step, the step, and the moves before it, so the facts are exactly as they were just before the step; redo puts them back, with the selection as it was when the step was undone.
- **Undo** applies a step's inverse operations to the facts through `Facts::apply`, as new operations, so sync sees an undo like any change. Undo and redo don't go through the app's `update`: the facts they produce are facts the app already had.
- **The command log** gets each commit that changed the facts: its commands (as `Value`s), the replica, key counter, time and seed `Cx` handed out, or just "undo" or "redo". `Runner::replay` runs a log again with those same values, so it gives the same facts; focus repairs are entries of their own, so a replay doesn't repair twice. In debug builds a command that returned `Outcome::Nothing` but changed the facts shows as a failure.
- **Off the main thread.** The commit's entry and operations go over a channel to Ledger's own thread (`ledger::Store`), which applies them to its own copy of the facts, appends them to the saved log and writes snapshots. The main thread's cost is the size of the change. The price is memory: the facts are held twice. Once sync is built, the same thread also keeps the CRDT.
- **Incoming operations** (sync, later) arrive from Ledger's thread already merged, as operations on keys, and `receive` applies them to the facts, so the main thread's cost is again the size of the change.

**Proposed:** the main thread's `List` is an ordered map from keys to records with no CRDT inside it, and operations name neighbours by key (`InsertAfter { list, after, key, value }`). The CRDT library (Loro, `ledger.md`) lives only on Ledger's thread, decides the merged order, and sends back what changed. That keeps the kernel independent of the library S5 chooses, and keeps every merge off the main thread.

### 13.3 Saving and reopening

The facts, session fields included, are saved by Ledger's thread from the operations it receives (`ledger.md` has the files). The kernel's interaction state (the focused id, its caret, each list's scroll anchor) is written as `place` when the runner closes. On launch, `Runner::open` opens the store in the app's data folder (`runner::folder`), which loads the last snapshot and the log after it before the first frame, then puts focus and scrolling back, so the app reopens where it was left. If the folder can't be used, the app runs unsaved and the message line says so.

## 14. Derived and live values, jobs

The to-do app needs jobs (for exporting and for bulk changes), tallies and bulk changes, so the first slice builds those in S5. Live and derived values came with build-plan item 13.

- **Jobs.** Every job runs on its own thread, has a name and shows in the status line while it runs, and in Scope. A job is an effect like any other, so Specimen holds it instead of running it and the command log records only its result. `cx.job(name, work, reply)` runs `work` once and hands its result to `reply`, which makes the command that comes back through `available` and `update`; the app passes in clones of the facts it needs. `App::start` gets `Jobs<C>` once, when the runner is made, for work that lasts as long as the app: `jobs.stream(name, source, Ticker::quotes)` runs `source` with the `Feed` of that `#[live]` field (reached through `Moving::feed`, typed by the field's path), and `jobs.job` is the same as `cx.job`. They start with the first painted frame, like every effect, because saving needs the window. A job whose thread ends without an answer is said in the message line ("“Market data” stopped before it finished"); a save says it through its own reply. Saving is one of these jobs: it clones the facts when it starts.
- **Live values.** `Live<T>` holds a shared slot that another thread publishes `Arc<T>` snapshots into without locks (the `arc-swap` crate's safe API). Publishing sets an atomic flag and wakes the event loop only if the flag was clear, so a feed ticking ten thousand times a second wakes the main thread at most once per frame. At the start of a frame the kernel pins each live value's latest snapshot; `now()` returns the pinned one, so a whole frame sees one value. A new snapshot bumps the field's version.
- **Derived values.** `#[derived(Candles::of, from = [quotes, chosen])]` registers a function and the fields it reads. After each commit and each live update, the kernel compares the versions of those fields with the ones the current value was computed from; if they differ, it takes snapshots of them and starts a job, dropping the result of any older job for the same value. The view reads the latest finished value and whether a newer one is coming.
- **How they are reached.** Ledger knows nothing of the kernel, so it carries them through one small trait, `ledger::Moving` (pin, wake with, arrive, coming), and three hooks on `Field` that `derive(Facts)` generates: `moving` hands each `#[live]` or `#[derived]` field to the kernel with its index, `derivations` lists each derived field with its name and the indexes it reads, and `derive_work` clones those fields and returns the work as a boxed closure for another thread. `kernel::Live<T>` keeps an `ArcSwap` slot, a fresh flag and the waker, and hands out `Feed<T>`, the writing end, to any thread; `kernel::Derived<T>` keeps the latest `Arc<T>` and whether a newer one is coming. The runner pins every live value at the start of each build and on each wake, bumps the versions of those that moved, and then compares each derived value's inputs with the versions it last asked for, starting a thread for each that differs; a result for versions no longer asked for is dropped, and one that lands bumps its own field's version, so a value derived from a derived value follows. The status line says "Working out “count”" while one is on its way. A runner without a waker (Specimen's proofs, Assay, the laws) works derived values out at once on its own thread, so what they see never depends on timing.
- **Tallies.** Every `List` keeps a count for each yes-or-no field and each choice of each plain enum field of its records, generated by `derive(Facts)` and updated with every change at constant cost. So `self.items.any(Item::done)` and `self.items.count(Item::done)` answer instantly at any size, and `List` offers no query that takes a closure: a question that has to look at every item is a job.
- **Bulk changes.** A change that has to look at every item (`retain`, `clear`) runs at once when the list is small enough to stay inside the budget. Measured in release builds: looking at an item costs about 3 ns and removing one, recorded, about 0.65 µs, so a list of up to 20,000 items is searched at once and up to 200 removals are made at once. Above either, `List::retain` hands the kernel a `ledger::Bulk` instead of changing anything: the search runs on its own thread over the list's chunks (a snapshot that costs one `Arc` per 512 items), and the removals land on the main thread in slices of 0.3 ms, one per frame, each item checked again as it goes (so an item unticked in the meantime stays). The status line shows the progress. It becomes one undo step when the last slice lands, and undo while it runs stops it and puts back what landed. Each slice goes into the command log with its count, so a replay lands the same slices at the same points. At 100,000 items, clearing a third of them takes about 75 slices with the slowest under 0.42 ms; the app's code is the same either way. A command run over many chosen items lands the same way: the runner runs subjects for 0.3 ms, and if any are left, keeps them in the `Background` with the step and runs the next ones each frame, checking each against `available` again first, since the facts may have changed in between; the whole run is one undo step, and undo while it runs stops it and puts back what landed. The first slice goes into the command log as the command with the subjects it ran and how many are left (`more`), and each later one as a `more` entry with its subjects, clock and how many are still left, so a replay runs exactly the same subjects at the same points. A slice that panics takes back everything the command did and says so. Undo and redo land the same way: `History::undo` and `redo` hand back the operations instead of applying them, and the runner applies them in slices of 0.3 ms, one per frame, the status line saying "Undoing “Clear finished”: 3,000 of 33,000". While they land, undo and redo wait, and a command lands the rest at once before it runs, so it never sees half a step. The first slice goes into the command log as the undo itself, and the rest as `turn` entries, which a replay skips because a replayed undo lands at once; a runner without a waker (proofs, Assay, the laws) also lands it at once.
- **Snapshots must be cheap.** A derived job's inputs cross to another thread, and copying a large list on the main thread would break principle 2. So live values and resources are shared by `Arc`, and Ledger's containers share their storage in chunks (copy on write), so a snapshot costs about the number of chunks, not the number of items. S5 builds `List` that way, even though the to-do app doesn't need it yet.

## 15. Budgets: where the time goes

| Stage (spec §12.2) | How this design meets it |
|---|---|
| Input to command | one array read for the hit, then the node's stored command |
| Updating facts for one action | the change itself, plus a comparison of the top-level plain fields |
| Finding what's under the pointer | one array read |
| Rebuilding and laying out a one-row change | the view is rebuilt (only visible rows), layout reuses every unchanged subtree |
| Painting the changed region | only nodes that touch the damage |
| Handing the frame to the GPU | only the changed quad ranges are uploaded; the GPU redraws the list |
| No allocation per frame | two trees and their arenas swapped and reused; inline strings in edits |
| Idle | no timers, no blinking caret, live values wake at most once per frame |

The first slice adds the laws for the grid, the frame budget, no allocation per frame and idle (spec §17); budgets are judged on the M1 in release builds, by the slowest 1% of frames.

One cost is still paid on the main thread the first time it comes up: shaping words and drawing their glyphs. A page of words seen for the first time takes about 1.3 ms to lay out and 0.8 ms to paint on a Windows desktop, against 50 µs and 100 µs once its words are known, which is why Assay's slowest single frame is the first opening of a component page. Shaping ahead of time on another thread belongs with Galley (build-plan item 23), which owns text.

## 16. The to-do app against this design

Walking through `api.md`'s to-do app line by line, everything compiles and runs as written except four things, each with the smallest fix:

1. **`as_markdown`, `Todo::sample` and `Todo::with_titles` don't exist.** The page calls the listing "the whole app" but never defines them. Fix: add a short `impl Todo` with all three to the listing. Building items outside `update` needs keys without `cx`; Ledger gives `Key::fixed(n)` for named states and tests, from a replica id no device uses, and `List` can be collected from an iterator.
2. **`ui.list(&self.items, &self.selection, …)` passes the selection by reference,** but the kernel changes the selection, and it can only send an `Edit` for something it has a path to. Fix: `ui.list(&self.items, Todo::selection, …)`, following the rule in §4.6.
3. **`Text` is described as "a value with a caret and a selection".** The caret is the field's (§11.3), because a `Text` in the document part is synced and a caret never is. Fix: one sentence.
4. **`Path` is Graticule's own file path type** (`Result<Path, Failure>`), not `std::path::Path`, which can't be held by value. No change to the code; the prelude exports it, and a line under "Reading it" says so.

Everything else is supported as written, by the parts above:

- `#[derive(Facts, Default)]`: the kernel's and Ledger's containers implement `Default` without requiring it of their items.
- `List<Item>`, `#[key]`, `Key<Item>`: `Record`, `Find<Item>` (§2).
- `available`'s `no_if`, `Yes`, `self.items.has`, `any`, `is_empty`: Ledger's `List` and the prelude.
- `update`'s arms as single expressions returning `Outcome` (§9.2), `cx.key()` inferred (§9.3), `cx.save_as(…, Cmd::Exported)` (§12), `cx.say` returning `Outcome::Nothing`, `edit.apply(self)` (§11).
- `ui.field(Todo::draft, …).on_submit(Cmd::Add)` (§4.6, §5.3), `ui.button(Cmd::Add)` and `ui.link(Cmd::Remove)` (§9.5), `ui.check(&item.title, item.done, Cmd::Toggle)` with the row's key (§10.1).
- The undo story ("type milk, add it, Cmd-Z: the item goes and milk is back"): the Add step holds both the `Text` splice from `take()` and the list insert (§13.2); the typing is the step before it.
- `states()` and `graticule::run::<Todo>(t!("To do"))`: `run` requires `App + Default` for the first launch, when there is no snapshot yet.

The first draft of `api.md` also had three lines whose cost grew with the list: `available` asked `self.items.none(|item| item.done)`, `ClearDone` ran `retain` on the main thread, and `Export` built the Markdown there. The owner decided that the fast way is the default way and that potentially heavy work runs off the main thread with its progress shown (§21). So `available` asks `self.items.any(Item::done)`, answered from the list's tallies; `retain` is a bulk change (§14); and `save_as` takes `Todo::as_markdown` itself, which runs in a job. The tallies, bulk changes and the job behind `save_as` are part of S5 and S6, not item 13, because the to-do app needs them.

## 17. Which step builds what

The first slice's steps each end with something visible. S3 needs facts and labels to render the to-do view, so the read side of facts and commands comes in S3 and the write side in S4 and S5.

| Part | Step |
|---|---|
| Workspace, the crates of §2 as skeletons, the lints (no `unsafe`), `apps/todo` | S2 |
| The shell, the window, the event loop, hidden until the first frame, launch timing | S2 |
| Fonts, text and shaping, the glyph atlas, character graphics, the GPU quad renderer | S2 |
| `Area` and the cell size; an empty window with the grid drawn | S2 |
| `derive(Facts)`, read side: paths, `Find`, `Record`, `attach`; `List`, `Text`, `Selection`, `Key` with their reading methods and plain in-memory changes | S3 |
| `derive(Commands)`, read side: names, labels, subjects; `available` and `Available`; `t!` in English with recorded ids | S3 |
| The tree, ids, `Ui` with typed subjects, `Bind` | S3 |
| Layout in cells, the narrow form, list virtualization, scroll anchoring | S3 |
| Comparing trees, damage, the kept quad list, painting; the quad list measured against a buffer of cells | S3 |
| Hit testing; hover, press and focus | S3 |
| Components: page, row, list, field, check, link, button, action bar, status line, message line | S3 |
| The accessibility tree from the tree | S3 |
| `update`, `Cx`, `Outcome`, running commands, kernel commands | S4 |
| Subjects resolved from rows, focus and selection; bulk commands; selection changes as edits | S4 |
| `Edit` and the field's editing model; caret and input method | S4 |
| Keymap files built into the app, sequences, which-key, the palette | S4 |
| The user's own keymap file, applied live, and "Edit the keys" | 12 |
| The journal and transactions, register comparison, `Op`, rollback on panic; jobs, tallies and bulk changes | S5 |
| The CRDT on Ledger's thread, chosen by measuring both candidates; chunked `List` storage | S5 |
| Steps, merging typing by word, the undo tree with selection, focus and caret; versions | S5 |
| The command log; saving and reopening with the session snapshot | S5 |
| `Effect`, the save panel, making and writing the file as a job, results as internal commands, `Path`, `Failure`, the message line | S6 |
| Specimen's Screens place, `States`, running `update` on a copy, showing effects, offscreen renders, `--specimen` in `graticule::run` | S7 |
| Live and derived values, versions per top-level field | build-plan item 13 |
| Context menus (the `unsafe` exception), drag and drop, canvases, pictures | later items |

## 18. What moves in from Feedreader and what is rewritten

Feedreader's Graticule (its `crates/graticule`) is immediate-mode: the app paints quads straight into a `Scene` every frame, records its own hit rectangles with app-defined hit types, and the whole screen is repainted each frame. Its contact with the operating system is good and tested; its architecture is what the spec replaces. So:

**Moved in during S2,** with their traps written into `docs/internals/` as they arrive (Feedreader's `docs/internals/graticule.md` already describes most of them):

- `shell.rs`, the parts that meet the system: creating the event loop and window, keeping the window hidden until the first frame, the launch stages and preflight, modifiers, scale and resize, the macOS menu bar through muda (the safe part; the context menu waits).
- `gpu.rs`: the wgpu quad pipeline, the atlas, offscreen rendering and capture.
- `text.rs`: parley and swash, shaping, measuring with trailing spaces, the glyph cache.
- `fonts.rs` and `assets/fonts`: bundled Plex, the scripts registered on first need, the fetched Chinese, Japanese and Korean fonts and their checksums.
- `chars.rs`: box drawing, blocks and braille drawn to the pixel.
- `grid.rs`: `Area`.
- From `ui.rs`: `Metrics` (the cell size), the OKLCH palette and the contrast function.

**Moved in later, when their step needs them:** scroll anchoring from `state.rs` and its law (S3); the editing model and key bindings of `field.rs` and its law, without its own undo history, which becomes Ledger's (S4); offscreen renders from `proof.rs` (S7); `menu.rs`'s context menu with `docs/internals/unsafe.md`, `reload.rs` with Bindery, `images.rs`, `picture.rs` with Engraver, `canvas.rs`, `dates.rs` (their build-plan items).

**Rewritten:**

- `Scene` and the painting API: replaced by the tree, layout and the painter. The drawing routines inside it (frames, rules, the checkbox, the scrollbar, field painting) are reused as the painter's code for each component.
- The `App` trait with its hooks (`press`, `drag`, `wheel`, `context`, `field`, `keep`, `restore`): replaced by commands, components and Ledger.
- `commands.rs` (`Registry`, `Chord`): replaced by `derive(Commands)` and keymap files. The chord labels can be reused.
- `state.rs`'s `Id` (an app-chosen hash): replaced by automatic ids (§5.2).
- `access.rs`: the accessibility tree is built from the tree's roles instead of guessed from what was painted.
- `kit/`, `sheets/` and `specimen.rs`: replaced by components with pages and by Specimen's places.

## 19. Laws this design proposes

New laws need the owner's approval (`AGENTS.md`). These are the ones the design leans on, by step:

- **S2:** idle is zero; no allocation per frame (already in the spec).
- **S3:** every node's `Area` lies within its parent's and on the grid, for generated trees and window sizes; no two nodes in a frame share an id; patching the quad list gives the same pixels as building it from scratch; the node found under a point is the topmost node painted there; a list's scroll position follows its item through insertions and removals (moved from Feedreader).
- **S4:** a command is never run when `available` says no; a bulk command is one transaction; field edits through `update` give the same text as the field's model (moved from Feedreader).
- **S5:** the laws already listed for Ledger in spec §13.2, plus: a list's tallies always equal a count over its items; a bulk change applied in slices gives the same facts as one applied at once; a transaction rolled back after a panic leaves the facts as they were; the `Outcome` an app returns agrees with the journal.

## 20. Things this page leaves for later, on purpose

- How an app's own components and per-window session state look in app code (`api.md`, "Open"). Until then, session fields are one copy per app, and the first slice has one window.
- Subjects that aren't keys (Ticker's `Choose(#[subject] Symbol)`): they need a `Subject` trait for small `Copy` values. Nothing in the first slice uses one.
- Versions finer than a top-level field.

## 21. The owner's answers

The questions this design raised were answered by the owner and are written into the spec:

- **Labels inside a row.** A control inside a row shows a short label ("Remove"); the screen reader, the palette and the menus say the full one ("Remove “buy milk”"). Written as `short:` in `#[label]` (§9.1).
- **Commands on several items.** The label says how many ("Remove 3 items"), written as `many:` (§10.2). Items that can't take the command are skipped, the rest run, and the message line says which were skipped and why.
- **The to-do app's keys.** The owner left demo apps' default keys to the maintainers, for good use. The to-do app's keymap: Delete and the sequence `d d` remove the focused or selected items, Space ticks the focused item, Cmd-Shift-P opens the palette.
- **Work that grows with the data.** The fast way is the default way; anything potentially heavy runs off the main thread, with its progress shown (§14, §16).
