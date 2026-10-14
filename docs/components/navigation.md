# Navigation

The places of an app along the top, the current one marked. Places that don't fit go into a "More…" choice at the end.

## Use it for

The few top-level places of an app that a person moves between: the folders of a mail app, the views of a planner.

## Code

```rust
ui.places(Catalog::folder);
```

The fact is an enum with `derive(Choice)`, the same as for tabs. Going to a place is an edit and one undo step.

## Keys

Tab reaches each place; Space or Return goes there. "More…" opens a menu of the places that are hidden.

## Narrow windows

Places that don't fit are hidden from the end, and "More…" lists them.

## Screen reader

A navigation landmark, each place a link that says whether it is the current one.
