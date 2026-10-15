# Color field

A field for a color, after a swatch of it.

## Use it for

A color a person types or pastes: a brand color, a label's color, a theme's accent. A color picker adds an area and a hue slider for finding one by eye.

## Code

```rust
ui.labelled("Accent", |ui| ui.color_field(Catalog::accent));
```

The fact is an `Oklch`, a color as OKLCH lightness, chroma and hue, kept in whole numbers so sliders can set each part. The field accepts hex (`#3366cc`, `3366cc`, `#36c`) and OKLCH (`oklch(0.63 0.26 290)`, with the lightness also as `63%`, or just the three numbers), and shows the color as hex. The swatch follows what is typed and shows an empty box while it doesn't read as a color. As in a date field, what is typed is read on Return or when another field is reached, and the color is set by one edit; what doesn't read is said and stays to be fixed. A chroma past what the screen can show is kept and drawn reduced until it fits.

## Keys

Return reads what is typed; Escape puts back what was there.

## Narrow windows

Twelve columns: the swatch and seven characters of hex.

## Screen reader

A text field with the hex as its value, named by what it is labelled with.
