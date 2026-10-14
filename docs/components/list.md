# List

A list of the records in a `List` field, scrolled by whole rows, with its selection in a `Selection` field. The view describes one row; the kernel calls it only for the rows that show, so a list of fifty million items costs what a screenful does.

## Use it for

Any collection the user goes through: items, files, messages.

## Code

```rust
ui.list(&self.items, Todo::selection, |ui, item| {
    ui.check(&item.title, item.done, Cmd::Toggle);
    ui.link(Cmd::Remove);
});
```

A command with a `#[subject]` inside a row acts on that row's item; from the bar, the palette or a key, it acts on the chosen items.

To let the user put the rows in their own order, give the list the command that moves one, taking the row's key and the place it goes, counted from the top before the move:

```rust
ui.list(&self.items, Todo::selection, |ui, item| {
    ui.text(&item.title);
})
.on_move(Cmd::Move);
```

`update` can do it with `self.items.move_to(item, to)`.

## Mouse

A click chooses a row. With `on_move`, dragging a row shows a line where it would land and says the move in the status line; letting go moves it, and Escape puts it back.

## Keys

Up and Down move through it, Home and End go to the top and the bottom, Shift with the arrows chooses several, select all chooses everything, and Escape lets go of all but the focused one. The scroll wheel scrolls.

## Narrow windows

A list takes the rows the page has to spare, at least three.

## Screen reader

Says "list" and how many items it has, and for each row "list item" and what the row says.
