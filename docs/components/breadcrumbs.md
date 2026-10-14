# Breadcrumbs

Where something sits, as the levels that lead to it, from the top down. The levels above the current one are links back up.

## Use it for

A place deep in a hierarchy: a folder in a project, a setting in a group of settings.

## Code

```rust
ui.breadcrumbs(Catalog::trail, &TRAIL);
```

`TRAIL` holds the names of the levels from the top, and the fact, a `u32`, says how deep the current one is, counting from 0. Going back up to a level is an edit and one undo step.

## Keys

Tab reaches each link; Space or Return goes there.

## Narrow windows

A long trail is shortened in the middle, keeping the top and the current level, and "…" marks where.

## Screen reader

A navigation landmark with a link for each level above, then the current one.
