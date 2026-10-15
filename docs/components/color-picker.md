# Color picker

A color field with a button after it that opens a color area and a hue slider under it.

## Use it for

A color that is sometimes typed and sometimes found by eye.

## Code

```rust
ui.color_picker(Catalog::accent);
```

It is a color field and a popover holding a color area and a hue slider, for the same `Oklch`. Every change in the popover is its own edit, and the field shows the result at once. The popover stays open while colors are tried; Escape or a click elsewhere closes it.

## Keys

The field's keys, then Tab to the button; Space or Return opens the popover with the focus on the area, Tab moves to the hue slider, and Escape closes it.

## Narrow windows

The field and the button go under each other when they don't fit side by side; the popover opens under the button if it fits, else above.

## Screen reader

A text field with the hex as its value, then a button that says it opens a dialog and whether it is open.
