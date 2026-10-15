# Image

A raster image, a photograph, a scan or a screenshot, in a box of cells: as many rows tall as it is given and as wide as its shape asks.

## Use it for

Pictures that are pixels. Icons, logos and ornaments are pictures drawn as vectors (see Picture); charts and diagrams are drawn on a canvas.

## Code

```rust
#[resource]
hills: Image,

ui.image("Green hills under an evening sky", &self.hills, 6);
```

An `Image` holds decoded pixels and is shared by reference, so keep it in the facts as a `#[resource]`: it is never copied into history. `Image::decode` reads PNG and JPEG; decode in a job, since it takes as long as the image is big. An image not loaded yet holds its place, blank.

The image is sent to the GPU once, off the main thread, with smaller copies of itself so it stays smooth when shown small, and let go of when the app drops it. It is drawn as large as fits in its box without changing its shape.

## Narrow windows

The box narrows with its column and the image shrinks inside it, keeping its shape.

## Screen reader

An image named by its label.
