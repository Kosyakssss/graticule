# Fieldset

Controls grouped under a heading, such as the settings of one kind. A check group is a fieldset of check boxes.

## Use it for

Gathering the controls that belong together on a long page, so the eye can find them and a screen reader can name them.

## Code

```rust
ui.fieldset("Writing", |ui| {
    ui.check("A yes-or-no fact", self.ticked, Cmd::Tick);
    ui.switch("Wrap long lines", Catalog::wrapping);
});
```

## Screen reader

A group, named with its heading.
