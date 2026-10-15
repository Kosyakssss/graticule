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

**Here.** `Binding` is a record: the arrow, which of its ends, the shape it points at, and the anchor normalised to that shape's box. Instead of callbacks, the board has one pure step, `Board::settle`, run after every change a board command makes: every bound arrow end is placed where its binding says. Because it runs inside the same transaction, the arrow's move is part of the same undo step, and undo restores both. Deleting a shape removes its bindings and leaves the arrows' ends where they last were. The law: after any sequence of moves, resizes, deletions and undos, every bound end sits on its anchor.

## Tools

**tldraw.** Tools are a state chart: a tree of `StateNode`s, one branch active at a time, each with `onEnter`, `onExit` and handlers for pointer and key events. An event enters at the root and passes down the active branch until a node handles it. The select tool alone has states for idle, pointing at a shape, translating, brushing a selection box, resizing, rotating and editing text.

**Here.** Smaller than a state chart, because the kernel hands the app a whole drag as data (`kernel::Drag`: where it was pressed, where it is, the trail, whether it moved past a click's wobble, whether Shift was held). The state a tool is in is then a pure function of the board, the chosen shapes, the tool and the drag: `Intent::of` returns an enum, `Choose`, `Translate`, `Brush`, `Resize` or `Create`, so impossible combinations can't be written and no state is kept between events. The view calls it on the drag under way to show what letting go would do, and the command calls it on the finished drag to do it, so the two can never disagree. `Board::perform` makes the change as one transaction; `Intent::preview` performs it on a small board of only the shapes it touches and the arrows held to them, which costs what the drag moves, not what the board holds. Which tool is in hand is a session fact and every tool is a command in the registry, so the bar, the palette and the keymap reach them. The bound-arrow law drives gestures through every tool. This is simpler than the state chart planned at first; hovering, double clicks and keys inside a tool (Escape drops a drag, which the kernel does) are what a state chart would add, and none needs one yet.

## Camera

**tldraw.** The camera is a record: `x`, `y` and `z` (zoom). Screen points convert to page points and back through it; wheel and pinch pan and zoom around the pointer.

**Here.** The same, as a session fact, but the kernel's (`kernel::Camera`), not Plotter's: the conversion needs the canvas's size in pixels, which only the kernel knows, and any canvas can use it. `Canvas::camera(path)` makes what the canvas draws page units, one per screen point at zoom 1; the wheel pans it and, with the primary key, zooms around the pointer, each as one change to the fact, as the time axis does with its view. Drags arrive in page units. The canvas box sits on whole cells like everything else; inside it anything goes (AGENTS.md, rule 1). Strokes and words stay the same size on screen at every zoom.

## Snapping

**tldraw.** While moving or resizing, edges and centres snap to other shapes' edges and centres, gaps snap to equal spacing, and guides show what snapped. The threshold is in screen pixels, so it feels the same at every zoom.

**Here.** `snap` takes the moving box and the other boxes and returns the smallest nudge, on each axis, that lines up an edge or centre within the threshold, with the lines it lined up for drawing guides; arrows are not snapped to. Equal gaps and handles come after.

## Culling and many shapes

**tldraw.** Shapes outside the viewport are not rendered.

**Here.** `Index` is a tree of boxes packed from the bottom up, sixteen to a box, by sort-tile-recursive packing, built off the main thread as a derived value of the board. A query costs what it finds. The tools find shapes through it: what is under the pointer, what an arrow starts and finishes on, what a box chooses and what a moving shape can snap to. It knows the top of the stack, so a new shape goes on top without a look at every shape. The whiteboard draws only what is in sight on the largest screen through the camera (a fixed 3840 by 2160 points, since the facts don't know the window's size), as a derived value of the board, the index and the camera, in slices of 512 from the bottom of the stack; a drag draws again only the slices holding what it moves. The box around the chosen shapes is derived too. While the background catches up after a change, a board of up to 4,096 shapes is drawn and indexed directly, and a larger one shows the last finished drawing, which the camera still moves, so the main thread's work never grows with the board. Shapes that arrive in sight are drawn a moment after the pan that brings them. The law that hit testing agrees with what is drawn checks the index against looking at every shape.


## Also in tldraw, later here

Groups, frames and pages; locking and read-only; shared styles where the next shape takes the last one's; pasting anything and dropping files; side effects on record changes beyond bindings; pressure-sensitive strokes, which port perfect-freehand (MIT) with its notice; rotation and handles on every side, and a minimap. Export to SVG comes with the core, since a board must render without a screen.
