# Code field

A code of a fixed number of characters, one to a box, such as the six digits a sign-in sends.

## Use it for

A short code a person copies from elsewhere: a confirmation code, a PIN.

## Code

```rust
ui.labelled("Code", |ui| {
    ui.code_field(Catalog::pin, 6);
});
```

It types into a `Text` in the facts like any field, an edit a word at a time. Typing or pasting past the last box does nothing more.

## Keys

The platform's own text editing; the caret's box is underlined heavily.

## Narrow windows

The boxes keep their width: four columns each, three for the box and one between.

## Screen reader

A text field with the code typed so far.
