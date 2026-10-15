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

**Here.** A tool is an enum and its states are enums inside it, so impossible combinations can't be written: `Tool::Select(Select::Idle | Select::Pointing { .. } | Select::Translating { .. } | Select::Brushing { .. } | Select::Resizing { .. })`, `Tool::Draw(Draw::Idle | Draw::Drawing { .. })`, and so on. Handling an event is a pure function from the tool, the event and the board to the next tool and the change to make, so tools are tested like everything else, by laws over generated pointer sequences. Which tool is active is a session fact and every tool is a command in the registry, so the bar, the palette and the keymap reach them.

## Camera

**tldraw.** The camera is a record: `x`, `y` and `z` (zoom). Screen points convert to page points and back through it; wheel and pinch pan and zoom around the pointer.

**Here.** The same, as a session fact, with `Camera::to_page` and `to_screen`. Its screen space is the canvas box the kernel gives it, which sits on whole cells like everything else; inside the box anything goes (AGENTS.md, rule 1).

## Snapping

**tldraw.** While moving or resizing, edges and centres snap to other shapes' edges and centres, gaps snap to equal spacing, and guides show what snapped. The threshold is in screen pixels, so it feels the same at every zoom.

**Here.** `snap::offset` takes the moving box and the boxes near it and returns the smallest nudge, on each axis, that lines up an edge or centre within the threshold, with the lines it lined up for drawing guides. Equal gaps and handles come after.

## Culling and many shapes

**tldraw.** Shapes outside the viewport are not rendered.

**Here.** Drawing asks a spatial index for the shapes that touch the visible page box, so a million shapes cost what the visible ones cost (spec §13.4's budget). The index is built off the main thread as a derived value of the shapes.

## Also in tldraw, later here

Groups, frames and pages; locking and read-only; shared styles where the next shape takes the last one's; pasting anything and dropping files; side effects on record changes beyond bindings; pressure-sensitive strokes, which port perfect-freehand (MIT) with its notice; rubber-band selection, transform handles and a minimap. Export to SVG comes with the core, since a board must render without a screen.
