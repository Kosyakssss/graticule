# Text

Words on the page, wrapped at the width they are given. Three tones: plain text in the proportional face; `mono`, the monospace face, for data such as numbers, codes and paths; and `muted`, for text that says less than the text around it.

## Use it for

Labels, explanations, values. Text never runs a command; a button or a link does.

## Code

```rust
ui.text("Plain text");
ui.mono("42.0175");
ui.muted("Says less than the text around it");
```

## Narrow windows

Text wraps at word boundaries and grows downward, whole rows at a time.

## Screen reader

Reads the text.
