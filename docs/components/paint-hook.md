# Paint hook

A canvas painted with the app's own GPU commands, under whatever is drawn on it: the rung of the ladder past shapes.

## Use it for

What neither components nor shapes can draw: a scan with its levels applied, a waveform of millions of samples, a shader. Reach for it last; most charts are shapes.

## Code

```rust
#[resource]
rings: PaintHook,

ui.canvas("Rings painted by a shader", 8, |c| {
    c.paint(&self.rings);
    c.text((0.02, 0.15), "painted on the GPU");
});
```

A paint hook is a type of the app's that implements `Paint`: `stamp` returns a number that changes whenever what it paints does, and `paint` paints the whole of a texture the size of the box, given the GPU (`wgpu`, re-exported by the kernel) and the theme. Colors are written with straight alpha. Keep it in the facts as a `#[resource]` made with `PaintHook::new`, or, when what it paints follows the facts (a camera, levels), make it in the view from them; the stamp still decides when it paints. `Target::gpu` names the device, so a hook keeps what it made on it, a pipeline or an uploaded texture, and makes it again on another.

It runs off the main thread, on the thread that hands frames to the display, and only when its stamp, the box's size or the theme changes; the main thread only places its texture. Whatever else is drawn on the canvas goes over it, and the canvas's label, keys and screen reader are as any canvas's.

## Narrow windows

The texture is as large as the box, so the hook paints again at the new size.

## Screen reader

A canvas named by its label.
