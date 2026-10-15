# Select

One of several named options, as a button that shows the chosen one and opens the options in place under it. Nothing floats: the options push what is below them down while they are open.

## Use it for

A choice among more options than segments or a cycling choice suit, where a person wants to see them all before choosing: a folder to move to, a format, a language.

## Code

```rust
ui.select("Move to", Catalog::folder);
```

The fact is an enum with `derive(Choice)`. Pressing the button opens the options with the chosen one ticked and focused; choosing one sets the fact by one edit and one undo step ("Move to: Archive") and closes them. Escape, a click elsewhere or moving the focus away closes them without a change.

## Keys

Space or Return opens the options; Up and Down move between them, Home and End to the first and last; Space or Return chooses; Escape closes.

## Narrow windows

The button is as wide as the longest option and keeps its width; the label beside it wraps.

## Screen reader

A combo box named by the label, with the chosen option as its value and whether it is open; each option is a list box option, marked when chosen.
