# Progress

How far a piece of work has come, as a bar with the count beside it: 37 of 120. The bar fills in the link ink.

## Use it for

Work the app counts itself, such as files copied so far. A job's own progress is in the status line already, with its name, so a page needs this only when the work is the page's subject. Nothing moves while nothing changes: there is no bar that moves by itself, since the status line says when work is running.

## Code

```rust
ui.progress(37, 120);
```

## Narrow windows

The bar shrinks to eight columns; the count always stays beside it.

## Screen reader

A progress bar, with its percentage and the count.
