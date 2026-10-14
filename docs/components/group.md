# Button group

Buttons and toggle buttons joined side by side with no room between them, read as one control. Segments are a group over a fact: one of a few named options, each a toggle button, the chosen one pressed in.

## Use it for

Commands that belong together, such as the parts of a toolbar, and choosing one of two to five options that are all worth seeing at once. For more options, or when room is short, use a radio group.

## Code

```rust
ui.group(|ui| {
    ui.button(Cmd::Press);
    ui.toggle(self.bold, Cmd::Bold);
});
ui.segments("Show as", Catalog::shape);
```

The fact behind segments is an enum with `derive(Choice)`; choosing one is an edit of it and one undo step.

## Keys

Tab reaches each button in turn; Space or Return presses it.

## Screen reader

A group; each part is a button, and a segment says whether it is the one chosen.
