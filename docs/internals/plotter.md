# Plotter: what we take from tldraw, and how it maps onto Graticule

Plotter's model is tldraw (spec §13.4). This page is the study the build plan asks for: what tldraw does, read from its documentation and reference (tldraw.dev, October 2026), and what each part becomes here. tldraw's SDK is source-available under its own license, so nothing is copied; every piece below is our own, written for Ledger, the kernel's canvas and the grid.

## The store and its records

**tldraw.** Everything on a board is a record in one reactive store: a JSON object with an `id` and a `typeName`, validated against a schema. Shapes, bindings, pages, assets and the camera are all records. Record types have a scope: `document` records are saved and synced, `session` records stay on the device. Every change is tracked, which gives undo, persistence and sync in one place. Old documents load through migrations, per record or over the whole store, each with an id and an `up` step.

**Here.** This is Ledger already. A board is facts: `List<Shape>` and `List<Binding>` are document facts, the camera and the tool are session facts, and every change is a Ledger operation, so undo, saving, crash replays and sync come for free. Migrations are Ledger's (`docs/internals/ledger.md`). Nothing new is built for the store; that is the point of spec §13.4's "a second kernel built from the same parts as the first".

## Shapes

**tldraw.** A shape record holds where it is (`x`, `y`, `rotation`, a `parentId` that is a page, a frame or a group), its place in the stacking order (`index`), whether it is locked, its opacity, and its own `props`. A `ShapeUtil` per type says how the shape is drawn, what its geometry is (`getGeometry`, built from `Geometry2d` pieces: rectangles, ellipses, polylines, arcs, groups of them), how it resizes, and whether it can be bound to.

**Here.** `Shape` is one record with a `Kind` enum (rectangle, ellipse, text, a freehand stroke, an arrow, a frame), since Rust's enums give each kind its own data and a `match` is the one obvious place to say what each kind does. Geometry is a pure function of the shape (`Shape::geometry`), in page units as `f64`, used for bounds, hit testing, snapping and export alike, so what is hit is what is drawn.

## Order

**tldraw.** Shapes stack by `index`, a fractional index: a string that sorts between its neighbours, so inserting between two shapes changes only the new one, and two devices inserting at once never fight over renumbering.

**Here.** `order::between(a, b)` makes a key strictly between two others (or before the first, or after the last) from a small alphabet. The law: a key made between two always sorts between them, however many times the gap is split.

## Bindings

**tldraw.** A binding is a record linking two shapes, `fromId` and `toId`, with props of its own. An arrow's binding stores where on the target it points, as an anchor normalised to the target's box, and whether it should land exactly there or on the target's edge. A `BindingUtil` hears about changes: when the target moves (`onAfterChangeToShape`) the arrow end is recomputed; when one of the pair is deleted or copied without the other, the binding is undone and the arrow keeps its end where it was.

**Here.** `Binding` is a record: the arrow, which of its ends, the shape it points at, and the anchor normalised to that shape's box. Instead of callbacks, the board has one pure step, `Board::settle`, run after every change a board command makes: every arrow end bound to a shape that changed is placed where its binding says. It finds those bindings through the bindings list's links (`List::linked`, see `docs/internals/ledger.md`), so a move costs the arrows held to what moved, not every binding on the board. Because it runs inside the same transaction, the arrow's move is part of the same undo step, and undo restores both. Deleting a shape removes its bindings and leaves the arrows' ends where they last were. The law: after any sequence of moves, resizes, deletions and undos, every bound end sits on its anchor.

## Tools

**tldraw.** Tools are a state chart: a tree of `StateNode`s, one branch active at a time, each with `onEnter`, `onExit` and handlers for pointer and key events. An event enters at the root and passes down the active branch until a node handles it. The select tool alone has states for idle, pointing at a shape, translating, brushing a selection box, resizing, rotating and editing text.

**Here.** Smaller than a state chart, because the kernel hands the app a whole drag as data (`kernel::Drag`: where it was pressed, where it is, the trail, whether it moved past a click's wobble, whether Shift was held). The state a tool is in is then a pure function of the board, the chosen shapes, the tool and the drag: `Intent::of` returns an enum, `Choose`, `Translate`, `Brush`, `Resize` or `Create`, so impossible combinations can't be written and no state is kept between events. The view calls it on the drag under way to show what letting go would do, and the command calls it on the finished drag to do it, so the two can never disagree. `Board::perform` makes the change as one transaction; `Intent::preview` performs it on a small board of only the shapes it touches and the arrows held to them, which costs what the drag moves, not what the board holds. Which tool is in hand is a session fact and every tool is a command in the registry, so the bar, the palette and the keymap reach them. The bound-arrow law drives gestures through every tool. A chosen shape has eight handles, its corners and the middle of each side (`plotter::HANDLES`, fractions of its box); a side's handle moves only that side. Text is written by a command, "Write…", which asks for the words in the palette: it rewrites the chosen text, or puts new text inside the one chosen shape, or at the top left of the view. Typing into the board itself would need a field inside a canvas, which the kernel doesn't have yet. This is simpler than the state chart planned at first; hovering, double clicks and keys inside a tool (Escape drops a drag, which the kernel does) are what a state chart would add, and none needs one yet.

## Camera

**tldraw.** The camera is a record: `x`, `y` and `z` (zoom). Screen points convert to page points and back through it; wheel and pinch pan and zoom around the pointer.

**Here.** The same, as a session fact, but the kernel's (`kernel::Camera`), not Plotter's: the conversion needs the canvas's size in pixels, which only the kernel knows, and any canvas can use it. `Canvas::camera(path)` makes what the canvas draws page units, one per screen point at zoom 1; the wheel pans it and, with the primary key, zooms around the pointer, each as one change to the fact, as the time axis does with its view. Drags arrive in page units. The canvas box sits on whole cells like everything else; inside it anything goes (AGENTS.md, rule 1). Strokes and words stay the same size on screen at every zoom.

## Snapping

**tldraw.** While moving or resizing, edges and centres snap to other shapes' edges and centres, gaps snap to equal spacing, and guides show what snapped. The threshold is in screen pixels, so it feels the same at every zoom.

**Here.** `snap` takes the moving box and the other boxes and returns the smallest nudge, on each axis, that lines up an edge or centre within the threshold, with the lines it lined up for drawing guides; arrows are not snapped to. It also spaces the box evenly along each axis: midway between its nearest neighbours in the same row or column, or one gap past the end of a run, as wide as the gap between the run's last two; whichever of lining up and spacing needs the smaller nudge wins, and the equal gaps come back for drawing. A row or a column is what overlaps the box across the other axis, looked up through the index in a band 1,200 screen points long each way, so the cost of a drag stays with what is near it on screen at any zoom. Snapping handles comes later.

## Culling and many shapes

**tldraw.** Shapes outside the viewport are not rendered.

**Here.** `Index` is a tree of boxes packed from the bottom up, sixteen to a box, by sort-tile-recursive packing, built off the main thread as a derived value of the board. A query costs what it finds. The tools find shapes through it: what is under the pointer, what an arrow starts and finishes on, what a box chooses and what a moving shape can snap to. It knows the top of the stack, so a new shape goes on top without a look at every shape. The whiteboard draws only what is in sight on the largest screen through the camera (a fixed 3840 by 2160 points, since the facts don't know the window's size), as a derived value of the board, the index and the camera, in slices of 512 from the bottom of the stack; a drag draws again only the slices holding what it moves. The box around the chosen shapes is derived too. While the background catches up after a change, a board of up to 4,096 shapes is drawn and indexed directly, and a larger one shows the last finished drawing, which the camera still moves, so the main thread's work never grows with the board. Shapes that arrive in sight are drawn a moment after the pan that brings them. The law that hit testing agrees with what is drawn checks the index against looking at every shape.


## The map

**tldraw.** A minimap in a corner shows the whole document small, with a box for the viewport; dragging on it moves the camera.

**Here.** The map is a pane beside the board, its own canvas with a camera the view works out to fit the board and the view's box, so it keeps its proportions. What it draws is a derived value of the board made off the main thread: each shape's box while there are up to 2,000, and past that a grid of 48 by 32 places, each marked when a shape's centre falls in it, so drawing the map costs the same at any size. The view's box comes from the board canvas, which knows its last size (`Canvas::showing`); the facts don't know the window. A press on the map moves the board's camera through `Canvas::steers`, which the kernel carries out because only it knows how big the board canvas is, so the point pressed lands in the middle.

## Strokes and pictures

**tldraw.** A freehand stroke keeps its points and draws through perfect-freehand, which turns them into the outline of a filled polygon whose width follows the pen's pressure or, without one, how fast the hand moved.

**Here.** The same, ported to `plotter::freehand` under perfect-freehand's MIT notice, with tldraw's settings for a mouse: the board keeps the points (`Kind::Draw`), and `Shape::stroke` makes the outline whenever it is drawn or exported, so the outline never needs saving or migrating. "Export a picture" draws the whole board into a PNG beside the board's file, two pixels to a page unit and no more than 8,192 along the longer side, in the light theme with a clear background, inside the job that writes the file. The kernel's canvas raster sets the words itself for this, with its own copy of the bundled fonts, since the window's text engine belongs to the main thread.

## Locking and styles

**tldraw.** A locked shape can be chosen but not moved, resized or restyled. The style panel shows the chosen shapes' shared style; picking one sets it on them and keeps it as the style the next shape is made in.

**Here.** `Board::lock` and `Board::restyle` change what the keys name and leave locked shapes alone, as moving, resizing and writing already did. The style for the next shape belongs to the app, not the board: the face keeps its ink as a session fact chosen by a select, and since the view never writes back, the face's `update` notices an edit that touches that fact and restyles the chosen shapes in the same step, so one undo takes back both. A locked shape shows no handles, since none would take a drag.

## Copy and paste

**tldraw.** Copying puts the chosen shapes and the bindings between them on the clipboard; pasting puts them back with new ids, and pasting text, images or URLs makes shapes of them.

**Here.** `Board::excerpt` is the chosen shapes and only the bindings whose two ends are both among them, and the face copies it as SVG, the same file a board saves as, so it pastes into anything that reads SVG and comes back whole through the metadata. `Board::paste` lays another board on top of the stack in its own order, with new keys from the app and its bindings remapped, so an arrow pasted with its boxes stays on them. Reading the clipboard is an effect, `cx.paste(Cmd::Pasted)`, so the text arrives as an internal command and the log keeps it; a replay pastes the same thing whatever the clipboard holds then. Text that isn't a board becomes a text shape. A pasted board, like a board file dropped on the window or chosen with "Import…", is read from its SVG in a job, since it may be large, and comes back as an internal command holding the board; the main thread only lays it down. Images wait on an image shape.

## Groups

**tldraw.** A group is a shape whose children name it as their parent; its bounds are its children's. A click chooses the outermost group under the pointer, and a second click goes inside it. A group left with fewer than two children is deleted, and ungrouping lifts its children to the group's parent.

**Here.** A group is not a shape but its own record, `Group`, holding only the group it sits in, since it has no place, size or ink of its own: it is drawn as nothing and its box is its shapes'. A shape's `group` and a group's `group` maybe hold a key, and the lists follow those fields as they follow bindings (`docs/internals/ledger.md`), so finding what a group holds costs what it holds. Every gesture and command works on shapes, never on groups: `Board::whole` widens shapes to every shape in their outermost groups, and choosing, moving and copying go through it, so the chosen shapes always hold whole groups and nothing else needs to know groups exist. Going inside a group with a second click, and resizing a group as one box, are not built. `Board::delete` takes apart any group left holding one thing, in the same step, so undo puts it back whole.

## Frames

**tldraw.** A frame is a shape that is the parent of the shapes inside it: they are drawn clipped to it and move with it. A shape dragged into a frame becomes its child, and dragged out goes back to the page; a new frame drawn around shapes takes them in. Deleting a frame deletes what it holds.

**Here.** A frame is a `Kind::Frame(name)` shape, and a shape's `frame` maybe holds the frame it is in, followed by the shapes list's links so a frame finds what it holds without looking at the board. Membership changes only where the tool knows where the pointer is: `Intent::of` asks the index for the topmost frame under the drop or under a new shape's middle, so the board never scans for frames, and the arrow keys move shapes without changing their frame. `Board::translate`, `excerpt` and `delete` take a frame's contents along (`Board::contents`), a locked frame holding its contents still. `Board::reframe` refuses to put a frame inside itself or what it holds. A frame is hit by its edge, not its inside, so the shapes it holds stay clickable whatever their place in the stack, and arrows don't hold to frames. Drawing clips each shape to `Board::window`, where its frames overlap: the kernel's canvas shapes and words take a clip box, and the SVG wraps them in a `clipPath`.

## Also in tldraw, later here

Pages; a read-only board, which waits on a way for an app to say that undo can't run, since undo would otherwise change a board meant to stay as it is; pasting or dropping images; side effects on record changes beyond bindings; pressure from a pen, which the stroke outline below already takes; and rotation. Export to SVG comes with the core, since a board must render without a screen.
