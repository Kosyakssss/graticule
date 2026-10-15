# Multi-select

Any of several named options, as a button that shows the chosen ones and opens the options in place under it, each ticked or not.

## Use it for

A filter or a setting where several options can hold at once: the folders to show, the days a reminder repeats, the kinds of file to include.

## Code

```rust
ui.multi_select("Show", Catalog::showing);
```

The fact is a `Vec` of an enum with `derive(Choice)`, kept in the order of the options. Each tick or untick is one edit and one undo step ("Show: tick “Sent”"), and the options stay open for the next one. The button names the chosen options, or says how many when their names would not fit, or "None".

## Keys

Space or Return opens the options; Up and Down move between them; Space or Return ticks or unticks one; Escape closes.

## Narrow windows

The button keeps the width of two options side by side; the label beside it wraps.

## Screen reader

A combo box named by the label, with the chosen options as its value; each option is a list box option, marked when ticked.
