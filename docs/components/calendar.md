# Calendar

A month of days, a week to a row from Monday, with the chosen day filled and today underlined. Days of the months on either side fill the first and last weeks, in the faint ink.

## Use it for

Choosing a day when where it falls in the week matters: a due date, a booking, a day to look at. When the person knows the date, a date field is quicker; a date picker offers both.

## Code

```rust
ui.calendar("Due", Catalog::due);
```

The fact is a `Day`, a day with no time and no zone. Taking a day sets it by one edit named after the label, "Due: 2026-06-03". Which month shows is the calendar's own: the month of the chosen day until it is turned, by its buttons or by moving past the month's edge. Turning it is no edit, and taking a day brings it back to following the chosen one. Today is the day in the user's time zone.

## Keys

The calendar is one tab stop, on the chosen day, or on the month's first when the chosen day isn't showing. The arrows move a day or a week, Home and End go to the week's ends, and Page Up and Page Down go a month back or on; moving past the month turns it. Return or Space takes the focused day.

## Narrow windows

It is 27 columns wide and 8 rows tall, whatever the window; where there is less room it goes under what is beside it, as anything in a row does.

## Screen reader

A grid named by its label; each day is a cell named in full, "Wednesday 3 June 2026", and says whether it is the chosen one. The turning buttons are "Previous month" and "Next month".
