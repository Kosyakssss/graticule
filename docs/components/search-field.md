# Search field

A field for words to search for, after a magnifying glass. The app filters by the text as it changes; the field itself only holds it.

## Use it for

Narrowing what a page shows: the rows of a list, the settings of a long form.

## Code

```rust
ui.search(Catalog::query, Message::from("Words to look for"));
```

The text is a `Text` in the facts, and each change is an edit, like any field. The app reads it in its view to choose what to show.

## Keys

What a one-line field does, and Escape empties it while it has text, as search fields do on every platform. When it is empty, Escape leaves it.

## Narrow windows

The magnifying glass keeps its two columns; the text scrolls inside the rest.

## Screen reader

Says "search", its placeholder when it is empty, and the text.
