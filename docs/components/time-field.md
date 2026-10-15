# Time field

A field for a time of day.

## Use it for

When something happens in a day, with no day of its own: opening hours, a daily reminder, a quiet period.

## Code

```rust
ui.labelled("Opens at", |ui| ui.time_field(Catalog::opens));
```

The fact is a `Time`, minutes after midnight with no zone. It accepts `15:30`, `15.30`, `1530`, `15`, `3pm`, `3:30 pm`, `noon` and `midnight`, and is always shown as `15:30`. As in a date field, what is typed is read on Return or when another field is reached, and the time is set by one edit.

## Keys

Up and Down step the hour when the caret is in the hours and the minute when it is in the minutes, going round midnight. Return reads what is typed; Escape puts back what was there.

## Narrow windows

Seven columns.

## Screen reader

A time field, named by what it is labelled with.
