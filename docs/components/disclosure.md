# Disclosure

A section that opens in place: a label with a chevron, and under it what it holds while it is open.

## Use it for

What most people don't need most of the time: advanced settings, the details of an error, the older items of a history. Several in a row make an accordion.

## Code

```rust
ui.disclosure("More settings", Catalog::open, |ui| {
    ui.switch("Wrap long lines", Catalog::wrapping);
    ui.check("A yes-or-no fact", self.ticked, Cmd::Tick);
});
```

Whether it is open is a `bool` in the facts. Opening and closing it is an edit and one undo step. What it holds is described only while it is open.

## Keys

Tab reaches it; Space or Return opens or closes it.

## Narrow windows

The section's contents are indented under the label and wrap within it.

## Screen reader

A button that says whether it is expanded, and the group it opens.
