# Radio group

One of a few named options, as radio buttons one under another under a heading. Every option is visible, and choosing one is a single click.

## Use it for

Choosing one of a handful of options where each deserves its own line, or where there are too many for segments.

## Code

```rust
ui.radio("Show as", Catalog::shape);
```

The fact is an enum with `derive(Choice)`, whose options are labelled by `#[label("…")]` or by their names in words. Choosing one is an edit of it and one undo step.

## Keys

Tab reaches each option; Space or Return chooses it.

## Narrow windows

Each label wraps beside its button.

## Screen reader

A group named with the heading; each option is a radio button with its label and whether it is chosen.
