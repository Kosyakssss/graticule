# Galley: the editing core

`crates/galley` holds what an editor needs below the screen. It depends on Ledger and nothing of the kernel's, so any program can edit text with it.

## Storage and history

A `Doc` keeps its text in a rope (ropey, MIT), so inserting into a hundred-megabyte file costs what inserting into a line does. It is a Ledger field like `Text`: every replacement records an `Op::Splice` while a transaction is open, and undo, redo, saving and replays work on those operations without knowing about ropes. Splices name places in bytes, as `Text`'s do, because that is what Ledger's operations already carry; `Doc` turns them back into characters when it applies one and refuses one that doesn't land on character boundaries or doesn't find the text it expects to remove, so a stale operation fails rather than corrupting the text. Saving writes the whole text as one string, which is fine for the sizes the demo apps keep in their facts; a file-backed document would save the file instead.

## Positions and selections

Everything the app sees counts characters, not bytes, so a position never falls inside a character. Motions step over grapheme clusters, what a reader sees as one character (an accented letter written as two code points, an emoji with a skin tone), by running `unicode-segmentation`'s grapheme cursor over the rope's chunks; words are runs of one class (letters and digits, punctuation, space), the way Helix and Vim see them.

`Selections` holds at least one span, kept in order, with overlapping or identical spans merged after every change, and one of them primary. Spans that only touch stay apart, so two adjacent selected words remain two.

## Changes made at once

`Changes` is every replacement of one edit, as ranges of the text before it, sorted and never overlapping. The `Doc` applies them from the last to the first so earlier ranges still mean what they meant, and each selection is mapped through them: a position after a change shifts by what it added or removed, one inside a replaced range moves to the end of the replacement, and one at the very place of an insertion moves after it, so a cursor stays after what was typed at it. Typing, deleting, a new line (indented like the line it breaks) and replacing every match are all a `Changes`, so each is one undo step at every cursor.

## Find

`find_all` walks the rope's bytes once with a Knuth–Morris–Pratt table: time grows with the text, never with the text times the pattern, and nothing is copied out of the rope. On a long text it belongs in a job, like every scan.

## Laws

`crates/galley/src/laws.rs`: any sequence of edits at any cursors, over text with combining marks, emoji and line breaks, undoes to the start and redoes to the end through its recorded operations; and the selections are valid after every step.
