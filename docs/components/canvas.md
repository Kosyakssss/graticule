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

The canvas is as wide as its column and as many rows tall as it is given. Its space runs from 0 to 1 across and down until `space` says otherwise; giving `y` from high to low makes it grow upward, as a chart's does. Lines and polylines are stroked in the style's line, and polygons, rectangles, dots and paths are filled, in ink, until told otherwise. A dot is round and the same size however the space is stretched.

The shapes are drawn off the main thread into the canvas's own texture, and drawn again only when what the canvas holds, its size, the style or the theme changes. With many shapes, build a `Drawing` in a job and hand it over with `c.drawing(&drawing)`, so the view never builds them.

## Pressing shapes

A shape is named by its key, its place in the drawing until `.key()` names it. `c.press(key, cmd)` runs a command when that shape is pressed. Shapes are hit against their outlines, and a thin line counts a little around it, so it can be hit.

## Keys

A canvas with shapes to press is one tab stop. The arrows move between those shapes, in the order their presses were given, Home and End go to the first and last, and Return runs the command of the one reached, which is ringed.

## Narrow windows

The canvas keeps its rows and narrows with its column; its space stretches to fit.

## Screen reader

A canvas is a canvas named by its label. When a shape to press is reached, its value is the command's label.
