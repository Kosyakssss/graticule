# Label

A control with its label above it. Muted text after the control describes it, and an error after it says what is wrong; a screen reader reads both with the control, so they are never only for the eye.

## Use it for

Fields, and any control whose purpose isn't said by its own words. Settings forms label their text fields this way.

## Code

```rust
ui.labelled("Something to say", |ui| {
    ui.field(Catalog::draft, Message::from("Type here")).on_submit(Cmd::Say);
    ui.muted("It shows in the message line");
});
```

An error goes in the same place, with `ui.error(…)`, while it applies.

## Narrow windows

The label wraps above the control.

## Screen reader

The control is named with the label, and described by the muted text and the error after it.
