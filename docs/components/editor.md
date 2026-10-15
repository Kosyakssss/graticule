# Editor

A box of plain text to read and edit, of any length: a note, a letter, a source file, a log of a hundred megabytes. It shows the text in mono, each line wrapped at the editor's width, with every cursor and selection the user has made.

## Use it for

Text that is the point of the screen, where a text area would be too small: the body of a document, a file being edited, a script. For a few lines inside a form, use a text area.

## Code

```rust
#[derive(Facts, Default)]
struct Notes {
    body: Doc,
}

ui.editor(t!("Note"), Notes::body);

Cmd::Edit(edit) => edit.apply(self),
```

The text is a `galley::Doc`, a field of the facts. What is typed, deleted, moved or clicked in the editor arrives in `update` as an `Edit`, and the `Doc` decides what it means at each of its cursors. Every change is a Ledger operation, so undo takes it back with the cursors where they were, and saving writes it. The app's own commands change the same `Doc`: `select_matches` and `replace_all` are how the Galley face finds and replaces.

`ui.editor_with_find(t!("Text"), Galley::text, Galley::find)` also underlines every place the `Text` at `find` appears on the lines that show. Going to the next one is the app's command, since it may be anywhere in the text: `Doc::find_from` works it out in a job on a copy and `Doc::go_to` selects it, refused if the text changed meanwhile.

Lines folded away in the `Doc` don't show: the line they are folded under ends with how many there are, faint, and the cursor steps over them. `Doc::fold(lines)` folds any lines, which is how a syntax tree's blocks would be folded; `Doc::block` finds the block of indented lines at the cursor, in a job, and `Doc::fold_found` folds it; `unfold` and `unfold_all` open them again. A cursor that lands inside a fold, by a find or a click, opens it.

The editor is as tall as the room it is given, at least three rows, and builds only the lines that show, so a very long file costs no more on screen than a short one. It follows the primary cursor as it moves.

## Mouse and keys

A click puts the cursor there and Shift with a click selects up to it; dragging selects. The wheel scrolls, by the lines that show. Once it has the focus, the keys are the platform's own: the arrows move by a character or a line, with the word keys by a word, Home and End to the ends of a line, the line keys or Page Up and Page Down by a screenful, Shift with any of them selects, Backspace and Delete delete a character or a word, Return starts a new line with the line's indent, Escape keeps only the primary cursor and then lets go of the selection, and select all, copy, cut and paste work as everywhere else. Typing words is one undo step per word.

Up and Down move by the rows a long line wraps into, keeping the column; Page Up and Page Down count whole lines. What an input method is composing shows underlined at the primary cursor, and its candidate window opens there; what it commits goes in at every cursor.

## Narrow windows

A long line wraps after the last space that fits, or mid-word when there is none, as a text area's does. A line taller than the whole editor shows its first rows.

## Screen reader

The editor is a text box with its label; each line that shows says its number and its text, and how many lines are folded away under it.
