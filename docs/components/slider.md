# Slider

A whole number within a range, as a handle on a track with the number after it.

## Use it for

A value that is felt rather than typed: a volume, a zoom, how much of something.

## Code

```rust
ui.labelled("Volume", |ui| ui.slider(Catalog::volume, 0..=100));
```

The fact is a whole number; the range is the part of it the slider covers. Dragging moves the handle and the number with it, and letting go sets the number by one edit, so a whole drag is one undo step.

## Keys

Tab reaches it. Left and Down step it down by one, Right and Up step it up; Page Up and Page Down step by a tenth of the range; Home and End go to its ends.

## Narrow windows

The track gives up width first; the number after it keeps room for the widest number in the range, so the handle doesn't jump as the number grows.

## Screen reader

A slider, with the number, and the least and most it can be.
