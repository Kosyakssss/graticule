# Check

A checkbox: a yes-or-no fact and its label. Clicking it runs a command, which changes the fact; the box only shows what the facts say.

## Use it for

A fact that is true or false, such as whether an item is done.

## Code

```rust
ui.check(&item.title, item.done, Cmd::Toggle);
```

## Keys

Tab reaches it; Return or Space runs its command.

## Narrow windows

The label wraps beside the box.

## Screen reader

Says "checkbox", the label and whether it is ticked.
