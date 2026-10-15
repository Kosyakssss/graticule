# Color area and slider

A color's lightness and chroma as an area to pick them from at its hue, and each part of the color as a slider whose track shows the colors it sets.

## Use it for

Finding a color by eye, or adjusting one part of it without the others: a lighter shade of the same hue, a quieter version of the same color.

## Code

```rust
ui.color_area(Catalog::accent);
ui.color_slider(Catalog::accent, Channel::Lightness);
ui.color_slider(Catalog::accent, Channel::Chroma);
ui.color_slider(Catalog::accent, Channel::Hue);
```

The area is 24 columns by 8 rows, painted by the kernel inside its box: chroma grows to the right, lightness upward, at the color's hue, with a mark where the color is. Where a lightness and chroma can't be shown, the area shows the nearest that can. Dragging shows where the mark would land and sets both parts when it is let go, by one edit.

A color slider is an ordinary slider over one part's whole numbers: lightness and chroma in thousandths, hue in tenths of a degree. It is set as any slider is, with its track drawn in the colors it sets.

## Keys

In the area, Up and Down step the lightness by a hundredth and Page Up and Page Down by a tenth; Left and Right step the chroma. Each step is one edit. The sliders have a slider's keys.

## Narrow windows

The area keeps its size; sliders are as wide as the room they have, like any slider.

## Screen reader

The area is a color well named "Lightness and chroma" with the hex as its value; each slider is a slider named by its part.
