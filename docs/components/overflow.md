# Overflow

Buttons and links on one row; what doesn't fit goes into a "More…" choice at the end, which lists it.

## Use it for

A toolbar whose actions should stay on one row however narrow the window: the commands above an editor, the actions of a selected item.

## Code

```rust
ui.overflow(|ui| {
    ui.button(Cmd::Press);
    ui.toggle(self.bold, Cmd::Bold);
    ui.button(Cmd::Tick);
    ui.link(Cmd::Follow);
    ui.button(Cmd::Say);
});
```

Everything inside is described as usual. Those that don't fit are hidden from the end, and "More…" appears only when something is hidden.

## Keys

Tab reaches what shows and "More…"; Space or Return on it opens a menu of the rest, where the arrows move and Return runs one.

## Narrow windows

That is what it is for: the row keeps its height and loses its last actions to the menu.

## Screen reader

A toolbar. What is hidden is still in the menu, with the same names.
