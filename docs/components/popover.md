# Popover

A button that opens a few controls in an overlay anchored under it, on the grid with the style's hard shadow.

## Use it for

A small piece of work that belongs to one button and would crowd the page if it were always shown: sharing, a filter's details, a quick setting.

## Code

```rust
ui.popover("Rename", |ui| {
    ui.field(Catalog::title, Message::from("Title"));
    ui.button(Cmd::Press);
});
```

What the closure describes is built only while the popover is open; it is ordinary view code, and its controls run commands and edit facts like any others. Opening it focuses its first control. It closes on Escape, on a click outside it, or when the focus moves out of it, and nothing in the facts changes by opening or closing it. Following spec §4.4 it opens only when pressed, hangs from its button, and never covers what is being edited.

## Keys

Space or Return opens it; Tab moves through its controls, and past the last one out of it, which closes it; Escape closes it and puts the focus back on the button.

## Narrow windows

The overlay is as wide as its content wants, under the button if it fits, else above, never past the window's edges.

## Screen reader

A button named by its label that says it opens a dialog and whether it is open.
