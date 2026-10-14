# Meter

A level within its range, such as how full a disk is, as a bar with the percentage beside it. Unlike progress, it is not work that finishes: the level can go up and down.

## Use it for

Levels, fill and use: disk space, a battery, a quota.

## Code

```rust
ui.meter(0.62);
```

The level runs from 0 to 1; anything outside is held at the nearer end.

## Screen reader

A meter, with its percentage.
