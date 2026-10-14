# Engraver and pictures

`crates/engraver` compiles SVG files into pictures; `kernel::picture` holds the types it writes and `components::draw_picture` draws them.

## Where the work happens

Everything that can be done ahead of time is done in the build script, so the kernel has one simple job: fill paths in an ink.

- **usvg** parses the file and resolves styles, `use`, transforms and shapes to paths. Engraver bakes each path's absolute transform into its points.
- **Strokes become outlines** with tiny-skia's stroker (dashes first, then the stroke with its caps, joins and miter limit). The kernel never strokes, so a picture looks exactly as drawn at every size and the atlas key needs only the picture, the layer and the scale. The cost is longer path data, about half a kilobyte per icon.
- **Colors become inks.** Each color must sit within `NEAR` (12 steps across red, green and blue) of one of the light theme's inks, or of black (ink) or white (background). The table is Engraver's own (`INKS`) because Engraver can't depend on the kernel; it is the drawing convention for authors, and the kernel then paints each layer in whatever its theme says that ink is, so pictures follow the appearance. When the light theme's values change, `INKS` changes with them; existing pictures keep working because the names don't move. The cursor, hover and selection inks are left out on purpose: they are states, not colors to draw with.
- **Refused, with the reason**: no `<title>` (or a blank one), a color that is no ink, gradients, patterns, transparency, clips, masks, filters, embedded images and live text. A refusal fails the build and names the file.

The generated file has one `static` per picture, named after the file in capitals, and `ALL`, every picture by file name. `Picture` compares and hashes by address, so a `Kind::Picture` in the tree costs a pointer and diffs as cheaply as any other kind.

## Drawing

A picture node is `rows` tall (one by default) and as many columns as its aspect needs at that height, rounded up; the shape is scaled to fit the box and centred, with its origin on a whole pixel. Each layer goes through `Text::shape` with a key made of the picture's address, the layer's index and the scale, so a picture is rasterised once per size into the glyph atlas and is one quad per layer after that.

## Why the face is a report

The kernel's build script runs Engraver on `crates/kernel/icons`, so a package dependency from Engraver to the kernel would be a cycle. `engraver FOLDER` therefore prints what it made of each file; seeing the pictures needs an app that draws them, and Specimen's Picture page draws every built-in icon at one and two rows in both appearances.
