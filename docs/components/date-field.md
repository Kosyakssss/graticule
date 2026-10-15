# Date field

A field for a day, typed as a date or in words.

## Use it for

A day the person knows: a deadline, a birthday, the day an invoice is due. A date picker adds a calendar beside it for when where the day falls matters.

## Code

```rust
ui.labelled("Due", |ui| ui.date_field(Catalog::due));
```

It accepts `2026-06-03`, `today`, `tomorrow`, `yesterday`, a weekday (`tue` is the coming one, today included; `next tue` the first after today; `last tue` the last before it), `+3` or `-2`, `in 3 days`, `2 weeks ago`, `next week`, `next month`, and a month and a day in either order with an optional year (`jun 3`, `3rd june 2027`). Like a number field, what is typed stays the field's own until Return or until another field is reached; then it is read and the day is set by one edit, or what doesn't read is said and stays to be fixed. It is always shown as `2026-06-03`. Days are counted from today in the user's time zone. A day and a time together in one field (`next tue 3pm`) is not done.

## Keys

Up and Down step the day by one. Return reads what is typed; Escape puts back what was there.

## Narrow windows

Twelve columns, enough for the date it shows.

## Screen reader

A date field, named by what it is labelled with.
