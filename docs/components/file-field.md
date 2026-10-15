# File field

A file's path, typed or pasted into a field, or chosen with the system's own file panel.

## Use it for

A setting or an argument that names one file: an attachment, a template, where something is read from.

## Code

```rust
ui.file_field("Attachment", Catalog::attachment);
```

The fact is a `Text` holding the path. Typing edits it like any field. "Choose…" opens the system's file panel; the window stays live while it is open, and the chosen path is set by one edit and one undo step, named for the file ("Attachment: notes.md"). Closing the panel without choosing changes nothing. Only one file panel opens at a time.

## Keys

The field's own text editing; Tab reaches "Choose…", and Space or Return opens the panel.

## Narrow windows

The field takes the room left beside the button and scrolls a long path inside it.

## Screen reader

A text field named by the label, and a button named "Choose a file for" the label.
