# Date picker

A date field with a button after it that opens a calendar under it.

## Use it for

A day that is sometimes known and sometimes looked for: type it, or open the month and take it.

## Code

```rust
ui.date_picker("Due", Catalog::due);
```

It is a date field and a popover holding a calendar, for the same `Day`. Taking a day in the calendar sets it by one edit and closes the calendar, and the focus goes back to the button. Both halves behave as they do on their own.

## Keys

The field's keys, then Tab to the button; Space or Return opens the calendar with the focus on the chosen day, and Escape closes it.

## Narrow windows

The field and the button go under each other when they don't fit side by side; the calendar opens under the button if it fits, else above.

## Screen reader

A date field, then a button that says it opens a dialog and whether it is open; the calendar is announced as a calendar is.
