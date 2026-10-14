# Tag

A short word that sorts something into a group, such as design or urgent, in a thin box. Unlike a typed value, a tag's word is the user's own. Like a typed value, it says what something is, never what to do, so clicking it does nothing; tags you can remove are the tag field's, in wave 2.

## Use it for

Labels, categories and topics.

## Code

```rust
ui.row(|ui| {
    ui.tag("design");
    ui.tag("urgent");
});
```

## Screen reader

Text, read as it is written.
