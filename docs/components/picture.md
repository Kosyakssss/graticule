# Picture

Vector art made ahead of time: an icon, an ornament, a logo. Pictures are drawn as SVG files and compiled into the app by Engraver, so they cost nothing to load. Each is drawn once at the size it is shown and comes from the GPU's atlas after that. Its colors are the theme's inks, so it follows the appearance like everything else. The box sits on the grid; the shape inside it is free.

## Use it for

Something that is quicker to see than to read: a warning beside a message, a folder beside a name, a play button. Graticule ships a set of icons for everyday things; an app adds its own from a folder of SVG files.

## Code

```rust
ui.picture(&icons::WARNING);
ui.picture(&icons::STAR).height(3);
```

It is one row tall beside text unless `.height(rows)` makes it taller, and as wide as its shape needs.

## Your own pictures

Draw them in the light theme's colors, each of which stands for an ink: ink #1a1a1d, muted #636368, faint #7f8085, rule #b7b7ba, link #2b58b1, visited #7241a0, danger #a9231e, warn #825100, ok #197037, field #efeeec, background #f9f8f6. Black means ink and white means background. Give each file a `<title>`, which becomes its label. Engraver refuses a file without a title, or one with any other color, a gradient, transparency or text that is still text, and says which and why. `cargo run -p engraver -- FOLDER` shows what it makes of a folder.

## Screen reader

An image, named by its label ("Warning").
