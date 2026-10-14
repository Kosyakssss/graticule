# Tabs

Pages of one panel, with a tab for each and the chosen one's page under them. The tabs are the options of an enum in the facts, so which page shows is a fact like any other: saved with the session, undone with the rest.

## Use it for

A few views of the same thing that don't need to be seen at once: the general, appearance and keys settings of one document.

## Code

```rust
ui.tabs(Catalog::pane, |ui, pane| match pane {
    Pane::General => ui.switch("Wrap long lines", Catalog::wrapping),
    Pane::Appearance => ui.radio("Show as", Catalog::shape),
    Pane::Keys => ui.muted("Keys are bound in the keymap file"),
});
```

The fact is an enum with `derive(Choice)`; each option's name is its tab's label. Switching tabs is an edit and one undo step. Only the chosen page is described, so the others cost nothing.

## Keys

Tab reaches each tab; Space or Return opens it.

## Narrow windows

The tabs wrap onto more rows, each one whole.

## Screen reader

A tab list, each tab with its name and whether it is the chosen one.
