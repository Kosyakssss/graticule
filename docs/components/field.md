# Field

A one-line field that types into a `Text` in the facts. The text is the app's; the caret and the selection inside the field are the field's own. Each change arrives in `update` as an `Edit`, so typing is undoable like any command, a word at a time.

## Use it for

Short text the user types: a title, a name, a search.

## Code

```rust
ui.field(Todo::draft, t!("Something to do")).on_submit(Cmd::Add);
```

`on_submit` names the command Return runs.

## Keys

The platform's own text editing: the arrows, Home and End, a word at a time with the word modifier, Shift with any of them to select, Backspace and Delete, the word and line deletions, select all, cut, copy and paste. Return runs the submit command. Escape leaves the field. Up and Down go to the start and the end.

## Narrow windows

The field takes the width it is given, at least sixteen columns; text longer than the field scrolls inside it to keep the caret in view.

## Screen reader

Says "text field", its placeholder when it is empty, and the text.
