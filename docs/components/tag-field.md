# Tag field

Words that tag something, each shown as a tag with a × that removes it, followed by a field to type another.

## Use it for

A few short labels a person makes up as they go: the tags of a note, the people on a message, the keywords of a picture.

## Code

```rust
ui.tag_field("Tags", Catalog::labels);
```

The fact is a `Vec<String>`. What is typed is the field's own until Return adds it as a tag; moving to another field adds it too. A tag that is there already is said and not added twice. Each tag added or removed is one edit and one undo step, named for the tag ("Tags: add “urgent”").

## Keys

The field's own text editing. Return adds what was typed; Backspace in the empty field removes the last tag; Escape drops what was typed. Tab reaches each tag, and Space or Return removes it.

## Narrow windows

The tags and the field share the row; the field keeps at least twelve columns.

## Screen reader

Each tag is a button named "Remove" and the tag; the field is named by the label.
