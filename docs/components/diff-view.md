# Diff view

What changed between two texts, line by line: removed lines in the danger ink, added ones in the ok ink, the words that changed inside a line marked more strongly, and long stretches that stayed the same folded into one line that says how many.

## Use it for

Reviewing a change before it is kept: two versions of a note, a file on disk against the one being edited, what a sync would bring in.

## Code

```rust
#[derived(Notes::diff, from = [saved, draft])]
diff: Derived<Diff>,

fn diff(saved: &Text, draft: &Text) -> Diff {
    Diff::new(saved.as_str(), draft.as_str())
}

if let Some(diff) = self.diff.get() {
    ui.muted(t!("{added} added, {removed} removed", added = diff.added(), removed = diff.removed()));
    ui.diff(t!("Changes to the note"), diff, 20);
}
```

`Diff::new` works out the shortest way from one text to the other as soon as it is made. That grows with the texts, so it belongs in a derived value. Each line shows its number in the old text and in the new, then a sign, then the line itself in mono. Three unchanged lines are kept around each change; longer stretches fold away. When two texts differ in more than about a thousand places, whatever is left past that is shown as all removed, then all added.

The view is as many rows tall as the app says and draws only the lines that show, so a diff of a very long file costs no more on screen than a short one.

## Mouse and keys

The wheel scrolls it. Once Tab reaches it, Up and Down move a line, Page Up and Page Down a screenful, Home and End to the ends. Side by side, opening a folded stretch, and stepping from change to change are not done.

## Narrow windows

Lines that don't fit are cut at the right edge; the numbers and signs always show.

## Screen reader

The diff is a list; each line says whether it was added or removed and its number, then its text. A folded line says how many lines it stands for.
