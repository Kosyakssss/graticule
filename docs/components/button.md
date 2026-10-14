# Button

A button for a command. Its label comes from the command's label, so a button always says what it will do, in the same words as the bar, the palette and the menus. When the command can't run right now, the button is drawn faint and the status line says why.

## Use it for

The main thing a part of the screen is for. When a row has several commands, the lighter link reads better.

## Code

```rust
ui.button(Cmd::Add);
```

## Keys

Tab reaches it; Return or Space runs it. Its command can also have keys of its own in the app's keymap.

## Screen reader

Says "button", the command's label and, when it can't run, why.
