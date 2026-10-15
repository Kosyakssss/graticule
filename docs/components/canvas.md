# Canvas

A box on the grid with a space of its own, holding lines, shapes, dots and words drawn at runtime in the theme's inks and the style's weights.

## Use it for

Charts, readouts and small diagrams: what cells can't show. A document of shapes people edit, an endless board or a map belongs to Plotter.

## Code

```rust
ui.canvas("Visits this week", 8, |c| {
    c.space(0.0..7.0, 100.0..0.0);
    c.polyline(points.iter().copied()).ink(Ink::Link).stroke(Weight::Bold);
    c.dot(points[6]).ink(Ink::Ok).key(LATEST);
    c.press(LATEST, Cmd::Say);
    c.text((0.2, 92.0), "80 visits on Sunday");
});
```

The canvas is as wide as its column and as many rows tall as it is given; given 0 rows, it takes the height the page has to spare, as an editor does. Its space runs from 0 to 1 across and down until `space` says otherwise; giving `y` from high to low makes it grow upward, as a chart's does. Lines and polylines are stroked in the style's line, and polygons, rectangles, dots and paths are filled, in ink, until told otherwise. A dot is round and the same size however the space is stretched. `over(from, to, |ui| …)` lays an overlay over a box of the canvas's space, on the cells nearest it, for a field that types into a shape where it is. `image(&image, from, to)` stretches a decoded picture over the rectangle between two corners and is hit anywhere inside it. `.clip(from, to)` on a shape or words shows only the part inside that box of the space, as a frame on a board hides what reaches past its edge; only that part is hit.

The shapes are drawn off the main thread into the canvas's own texture, and drawn again only when what the canvas holds, its size, the style or the theme changes. With many shapes, build a `Drawing` in a job and hand it over with `c.drawing(&drawing)`, so the view never builds them.

## Pressing shapes

A shape is named by its key, its place in the drawing until `.key()` names it. `c.press(key, cmd)` runs a command when that shape is pressed. Shapes are hit against their outlines, and a thin line counts a little around it, so it can be hit.

## Endless pages and drags

```rust
ui.canvas("Board", 0, |c| {
    c.camera(Board::camera);
    c.on_drag(Cmd::Drew);
    if let Some(drag) = c.drag() {
        c.line(at(drag.from), at(drag.to)).ink(Ink::Muted);
    }
});
```

With `c.camera(path)` the canvas looks through a `Camera` fact onto an endless page: what it draws is in page units, one to a screen point at zoom 1, the wheel pans the camera and the wheel with the primary key zooms it around the pointer, each one change to the fact. With `c.on_drag(cmd)`, a press, any moves and a let-go on the canvas run `cmd` with a `Drag` when it is let go (a closure, `move |drag| Cmd::Dragged(track, drag)`, says which of several canvases it was on): where it was pressed and let go and every place between, in the canvas's space, whether it moved further than a click's wobble and whether Shift was held. `c.drag()` gives the drag under way, so the view can show what letting go would do; Escape drops it. Plotter's whiteboard is built this way.

A canvas also says how big it was when last laid out: `c.size()` in points, and, with a camera, `c.showing()`, the part of the page in view. A map of a larger canvas looks through `c.look(camera)`, a camera the view works out rather than a fact, so the wheel leaves it alone, and `c.steers(path)` makes a press on it, or a drag let go on it, move the camera at `path` so that point is in the middle of its canvas. Plotter's map is built this way. `c.pans()` makes a drag on a canvas with a camera pan it: the canvas shows the camera moving while the drag is under way, and letting go makes it one change to the fact, "Pan", which undo takes back; a press that doesn't move still runs the canvas's drag command. Atlas is built this way.

A camera set to `Camera::WHOLE` shows whatever `c.fits(area)` names, whole and centred, worked out from the canvas's size each time it is laid out, so a picture just opened fits any window; the first zoom or pan starts from that view and keeps a camera of its own. `c.fits` comes before `c.camera`. `c.looking()` gives the camera as the canvas shows it, `WHOLE` worked out and a pan under way included, for a paint hook that draws through the same camera. Words drawn with `.backed()` sit on a patch of the background, so they read over a picture. Lightbox is built this way.

## Keys

A canvas with shapes to press is one tab stop. The arrows move between those shapes, in the order their presses were given, Home and End go to the first and last, and Return runs the command of the one reached, which is ringed.

## Narrow windows

The canvas keeps its rows and narrows with its column; its space stretches to fit.

## Screen reader

A canvas is a canvas named by its label. When a shape to press is reached, its value is the command's label.
