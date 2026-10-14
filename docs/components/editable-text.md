# Editable text

Text that reads as text, with a faint line under it to say it can be changed, and becomes a field when it is clicked or reached with Tab.

## Use it for

A name or a title shown where it is used, changed in place: the title of a document, the name of a list.

## Code

```rust
ui.editable(Catalog::title, Message::from("Untitled"));
```

It types into a `Text` in the facts, an edit a word at a time, like any field. Return finishes the edit and leaves it.

## Keys

Tab reaches it; then it is a field, with the platform's own text editing. Return or Escape leaves it.

## Narrow windows

It is as wide as its text, down to eight columns; longer text scrolls inside it while it is being edited.

## Screen reader

Says "text field" and the text, since that is what it is to someone not looking.
