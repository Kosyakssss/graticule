# Text area

A field of several lines that types into a `Text` in the facts. It wraps at its width and grows with what is typed, so nothing hides inside it.

## Use it for

Short text of more than one line: a note, a description, a message. A document belongs to Galley.

## Code

```rust
ui.text_area(Catalog::notes, Message::from("A few lines"), 3);
```

The last number is how many rows it shows when it is empty or nearly so. Typing, as in every field, is an edit a word at a time, and one undo step each.

## Keys

What a one-line field does, and: Return starts a new line, and the primary key with Return runs the submit command. Up and Down move between the rows as they show, keeping the column; at the first or last row they go to the start or the end. Home and End go to the ends of the row, and the primary key with Up or Down to the ends of the text.

## Narrow windows

The text wraps after the last space that fits, or within a word when a word is wider than the field, and the field grows taller.

## Screen reader

Says "text area", its placeholder when it is empty, and the text.
