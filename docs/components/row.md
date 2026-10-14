# Row

Components side by side, left to right, a column apart.

## Use it for

A field with the button that acts on it, or a few short pieces that read as one line.

## Code

```rust
ui.row(|ui| {
    ui.field(Todo::draft, t!("Something to do")).on_submit(Cmd::Add);
    ui.button(Cmd::Add);
});
```

## Narrow windows

When the components' narrow widths don't fit side by side, they stack, one under the other, in the same order.

## Screen reader

A group; each component inside says its own part.
