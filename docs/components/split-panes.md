# Split panes

Panes side by side with a divider between each two, which the user moves to give one pane more room and the other less. A mail app's folders, messages and the message open; a file manager's places and files.

## Use it for

Views that show a list and the thing chosen in it at once, or tools beside a canvas, where how much room each part gets is the user's to say.

## Code

```rust
ui.split(|ui| {
    ui.pane("Folders", cols(24).min(16), |ui| folders(ui));
    ui.pane("Message", Fill.min(40), |ui| message(ui));
});
```

Each pane is a column with a name, which its divider and the screen reader use, and a starting width in whole columns or `Fill`. A split takes the rows the page has to spare. The width the user gives a pane is kept with the window, not the facts, so it is not undone and comes back when the app opens again.

## Mouse

Dragging a divider moves it. The pane with a fixed width changes; the one that fills takes the rest.

## Keys

Tab reaches each divider, and Left and Right move it by a column.

## Narrow windows

When the panes don't fit side by side, each at its narrowest, they stack, sharing the rows, and the dividers go.

## Screen reader

Each pane is a pane with its name; each divider is a splitter named for the panes on either side of it.
