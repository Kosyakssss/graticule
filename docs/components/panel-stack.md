# Panel stack

Drilling into something and back, in place: each level replaces the one before it, and the trail above says how to get back.

## Use it for

Settings that nest, a file browser in a narrow window, any place where a person goes in one level at a time.

## Code

```rust
ui.breadcrumbs(Catalog::depth, &LEVELS);
ui.muted(match self.depth {
    0 => "Everything you can change",
    1 => "Who you are here",
    _ => "How you sign in",
});
ui.button(Cmd::Deeper);
```

A panel stack is not a component of its own: it is a fact for how deep you are, a view that describes the level it says, a command to go in, and breadcrumbs to come back out. Every step in and out is one undo step.

## Keys

The command that goes in has whatever keys the keymap gives it; the breadcrumbs are reached with Tab.

## Narrow windows

One level shows at a time, so it fits wherever its contents do.

## Screen reader

The breadcrumbs say where you are; the level's own contents follow.
