# Link

A lighter way to offer a command: underlined text that runs it. Like a button, its words are the command's label, and inside a list row its short form ("Remove", where the palette says "Remove “Call mum”").

## Use it for

Commands that are one of several on a row, where buttons would be loud.

## Code

```rust
ui.link(Cmd::Remove);
```

## Keys

Tab reaches it; Return or Space runs it.

## Screen reader

Says "link" and the command's full label.
