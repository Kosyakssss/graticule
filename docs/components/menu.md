# Menu

A button that opens a menu of commands, anchored under it, as the drawn context menu is.

## Use it for

A handful of commands that belong together but don't each need a place on screen: a document's export formats, a row's less common actions, the sort orders of a list.

## Code

```rust
ui.menu("Edit", |ui| {
    ui.button(Cmd::Press);
    ui.button(Cmd::Bold);
    ui.button(Cmd::Say);
});
```

The entries are ordinary controls, so the commands come from the registry like any other: each entry shows its label, its key when it has one, and when it can't run, why, as an entry that can't be chosen. Choosing an entry runs what its control would. The controls themselves aren't drawn and Tab doesn't stop on them; the menu is how they are reached, and they stay in the bar, the palette and the context menu as every command does.

## Keys

Space or Return opens the menu with its first entry that can run focused; Up and Down move between entries; Return chooses; Escape closes it.

## Narrow windows

The button keeps the width of its label; the menu opens under it, or above when there is no room below, never past the window.

## Screen reader

A button named by the label that says it opens a menu.
