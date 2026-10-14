# Toggle button

A button that stays pressed in, on the selection's ground, while what it stands for is on. Clicking it runs its command, which turns the thing on or off.

## Use it for

A mode that applies until it is turned off: bold, wrapping long lines, showing hidden files. For a setting kept in the facts, a switch says the same more plainly.

## Code

```rust
ui.toggle(self.bold, Cmd::Bold);
```

## Keys

Tab reaches it; Space or Return presses it.

## Screen reader

A button, with whether it is on.
