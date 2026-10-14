# Switch

An on or off fact, changed the moment it is clicked. A switch is for settings, which apply at once; a check box is for choosing things or answering a yes-or-no question as part of something larger.

## Use it for

Settings that turn something on or off: wrap long lines, show hidden files, play sounds.

## Code

```rust
ui.switch("Wrap long lines", Catalog::wrapping);
```

The fact is a `bool`; each change is an edit of it and one undo step. Settings forms draw their yes-or-no settings as switches.

## Keys

Tab reaches it; Space or Return flips it.

## Narrow windows

The label wraps beside the switch.

## Screen reader

A switch, with its label and whether it is on.
