# Scroll edges

A rule at the top or bottom of a list that says there is more beyond it, above or below.

## Use it for

Nothing: every list has them. A list with more rows above draws one at its top; one with more below draws one at its bottom.

## Code

```rust
ui.list(&self.items, Notes::chosen, |ui, item| ui.text(&item.title));
```

There is nothing to write. The rules come with the list, alongside its scroll bar.

## Keys

None of their own.

## Narrow windows

They span the list's width, beside the scroll bar.

## Screen reader

Nothing extra: a screen reader says how many rows a list has and which one is focused.
