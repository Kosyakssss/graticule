# Callout

A message boxed into the page, marked with a bar and an icon in its ink: information in the link ink, a warning, an error in the danger ink, or success. It stays where it is put, in the flow of the page; a message about something that just happened belongs in the message line instead.

## Use it for

Something the reader should know before going on: that a setting has a cost, that a file is read-only, that the disk is nearly full.

## Code

```rust
ui.callout(Ink::Warn, |ui| {
    ui.text("The disk is nearly full. Old versions are removed first.");
});
ui.callout(Ink::Link, |ui| {
    ui.text("Changes are saved as you make them.");
});
```

## Screen reader

A warning or an error is an alert; information and success are a note. Either way, what is inside is read in order.
