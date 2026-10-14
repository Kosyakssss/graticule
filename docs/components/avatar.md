# Avatar

A person, as the first letters of their name in a box. A picture of the person comes with raster images (build-plan item 16).

## Use it for

Showing who wrote, changed or owns something, beside their name or where there is no room for it.

## Code

```rust
ui.row(|ui| {
    ui.avatar("Ada Lovelace");
    ui.text("Ada Lovelace");
});
```

## Screen reader

An image, named with the whole name.
