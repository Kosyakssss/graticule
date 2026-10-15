# Confirm row

A button that asks before it runs: pressing it puts a question in its own place, with a button that runs the command and one that cancels. It is the grounded form of a dialog: nothing covers the page, and the question stands where it was asked.

## Use it for

A command that can't be undone or would lose work: emptying the trash, discarding a draft, signing out of a device. Anything that can be undone should just run, since undo is the better safety net.

## Code

```rust
ui.confirm(Cmd::Press, Message::from("Press it? This press can't be taken back."));
```

The button carries the command's label and is unavailable, with the reason said, whenever the command is. Asking changes nothing in the facts; the question is built only while it stands. The button that runs the command has the command's label too, so the answer names what happens.

## Keys

Space or Return asks, and the focus goes to the button that runs the command. Tab moves to Cancel; Escape, a click elsewhere or moving the focus away cancels, and the focus goes back to the button.

## Narrow windows

The question wraps, and the two buttons go under it when they don't fit beside it.

## Screen reader

A button; while it asks, an alert dialog named by the question, holding the two buttons.
