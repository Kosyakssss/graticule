# Values

Each kind of value has one look, used everywhere, so a reader knows what a thing is before reading it. A count and a date are data, in the monospace face. A version, a state, a product code, a piece of code and a mode each sit in a box one column wider than the text. These boxed values are badges: they say what something is and never what to do, so clicking one does nothing. What you can act on is a button or a link.

## Use it for

- `count`: how many there are, with its thousands grouped: 1,284.
- `date`: when something happened. Every date in every app is written the same way, chosen by the user: as the day and time (2026-06-01 10:00), or as how long ago (2 h ago, yesterday, 3 weeks ago). The commands "Write dates as the day and time" and "Write dates as how long ago" switch it for every app at once.
- `version`: a version, in a dotted box.
- `state`: what state something is in, filled with an ink that text is written in, such as ok, warn or danger. It is written in capitals.
- `product`: a product's code, like DK-01, in a heavy box.
- `code`: a piece of code or a query, on the field's surface.
- `mode`: the mode something is in, muted and underlined.
- `error`: what went wrong, in the danger ink.

## Code

```rust
ui.row(|ui| {
    ui.count(1284);
    ui.date(file.edited);
    ui.version("0.4.1");
    ui.state("offline", Ink::Danger);
});
ui.row(|ui| {
    ui.product("DK-01");
    ui.code("is:unread");
    ui.mode("rows");
});
ui.error("Couldn't reach the server");
```

## Dates that move

A date written as how long ago changes as time passes. The window wakes once, at the moment the first date on screen would read differently, and sleeps until then. When every date is written as the day and time, nothing wakes it.

## Screen reader

Text, read as it is written.
