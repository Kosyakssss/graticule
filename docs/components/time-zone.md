# Time zone

A combo box over the world's time zones, by their names: `Europe/Lisbon`, `America/Sao_Paulo`.

## Use it for

Where a person or a meeting is, when times are shown or scheduled across zones.

## Code

```rust
ui.time_zone(Catalog::zone, Message::from("Time zone"));
```

The fact is the zone's name as `Text`. The names come from the time zone database Graticule already writes dates with, read once; the field lists the first eight that contain what is typed, ignoring case, so `lisb` finds Lisbon and `sao` finds São Paulo's zone. Like any combo box it takes any text: an app that needs a real zone checks the name where it uses it.

## Keys

A combo box's: Down to the completions, Return or a click to take one.

## Narrow windows

A field's, with the completions under it as wide as the longest.

## Screen reader

A text field named by its placeholder; each completion is an option.
