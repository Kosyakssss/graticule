# Columns

Columns side by side, each made with `column` and given a width in cells: a number, `Fill`, or either with a smallest width (`28.min(20)`, `Fill.min(40)`).

## Use it for

A label beside what it labels, or two panes of content that line up down the page.

## Code

```rust
ui.columns(|ui| {
    ui.column(12, |ui| ui.text("Name"));
    ui.column(Fill, |ui| ui.muted("Takes the rest of the width"));
});
```

## Narrow windows

When the columns' smallest widths don't fit, they stack.

## Screen reader

A group of columns; each says its own content.
