# Icon button

A button that shows a picture instead of words. The command's label is still what the status line shows when the pointer is over it and what a screen reader says, so the picture never has to explain itself. A close button is an icon button with the close icon.

## Use it for

Commands everyone knows by their picture, where words would crowd the row: close, add, search, settings.

## Code

```rust
ui.icon_button(&icons::CLOSE, Cmd::Back);
```

## Keys

Tab reaches it; Space or Return presses it.

## Screen reader

A button, named with the command's label.
