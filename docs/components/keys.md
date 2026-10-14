# Key label

The keys that run a command, drawn as key caps. They are read from the keymap as it is when the page is drawn, so they change when the keymap does and are never typed by hand. A command with no key says "no key".

## Use it for

Telling someone in a sentence which keys do a thing. The bar, the help page and the palette already show every command's keys.

## Code

```rust
ui.row(|ui| {
    ui.text("Press");
    ui.keys(Cmd::Press);
    ui.text("to press the button");
});
```

## Screen reader

Text: the keys, written as the platform writes them.
