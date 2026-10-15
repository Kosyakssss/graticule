# Form

Settings drawn from a struct of facts: one control for each labelled field, chosen by its type.

## Use it for

A page of settings or options, such as a print dialog's, where every field is something a person sets.

## Code

```rust
ui.form(Catalog::printing);
```

`printing` is a struct with `derive(Facts, Form)`, and each field marked `#[label("…")]` is one setting. A `bool` is a switch, an enum with `derive(Choice)` is joined toggles, a `Text` is a field, and a whole number is a number field between a lower and a raise button. Every change is one edit and one undo step, named for what it set ("Copies: 3"). The built-in commands "Show only the changed settings" and "Show every setting" narrow every form to what differs from the struct's `Default`.

## Keys

Each control's own: Tab moves between them.

## Narrow windows

Each setting keeps its row; the controls inside it wrap like any row.

## Screen reader

Each control is named by its setting's label.
