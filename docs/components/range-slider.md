# Range slider

Two whole numbers, the ends of a range, as two handles on one track with the range written after it.

## Use it for

A span a person narrows by feel: opening hours, a price range, the years to show.

## Code

```rust
ui.labelled("Opening hours", |ui| ui.range_slider(Catalog::hours, 0..=24));
```

The fact is a pair of whole numbers, such as `(u8, u8)`, the low end first. Pressing the track takes the nearer handle; dragging only shows where it would land, and letting go sets the range by one edit and one undo step ("Set to 9–17"). Neither handle passes the other.

## Keys

Tab reaches each handle. The arrows step it by one, Page Up and Page Down by a tenth of the range, Home and End to as far as it can go.

## Narrow windows

At least thirteen columns of track, with the range after it.

## Screen reader

A group with two sliders, "From" and "To", each with its value and the range's ends.
