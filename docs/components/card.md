# Card

A group of components in a box under its title, for things that belong together, such as one account's details. A card is never a button: what you can act on inside it has its own button or link.

## Use it for

Showing several things side by side or one after another, each with its own parts.

## Code

```rust
ui.card("Account", |ui| {
    ui.text("ada@example.org");
    ui.button(Cmd::Press);
});
```

## Narrow windows

A card is as narrow as its widest part allows, plus two columns each side.

## Screen reader

A group, named with its title.
