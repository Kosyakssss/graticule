# Choice

One of a few named options, as a button that shows the chosen one. Pressing it moves to the next option, and after the last back to the first, so nothing opens over the page.

## Use it for

A setting with a few options that a person flips through rather than compares, where segments or radio buttons would take too much room: a sort order, a speed, a layout.

## Code

```rust
ui.choice("Show as", Catalog::shape);
```

The fact is an enum with `derive(Choice)`, whose options are labelled by `#[label("…")]` or by their names in words. The button is as wide as the longest option, so it doesn't jump as it cycles. Each press is an edit of the fact and one undo step, named for the option it chose ("Show as: Grid").

## Keys

Space or Return chooses the next option.

## Narrow windows

The label wraps; the button keeps its width.

## Screen reader

A button named with the label and the option chosen now ("Show as: List").
