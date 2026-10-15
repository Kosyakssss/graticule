# Chart

Numbers drawn: lines, bars and candles on a canvas, with a scale up the left and along the bottom whose labels sit on whole cells. A sparkline is the smallest chart, a trend in a table's cell.

## Use it for

Seeing the shape of numbers that change: prices over days, rain by month, how long builds took. A sparkline beside each row of a table, where the shape matters more than the values.

## Code

```rust
let plot = Plot::new(8)
    .bars(months.iter().enumerate().map(|(k, m)| (k as f64, m.rain)), Ink::Selection)
    .line(months.iter().enumerate().map(|(k, m)| (k as f64, m.warmth)), Ink::Link)
    .names(months.iter().map(|m| m.name.as_str()));
ui.chart(t!("Rain and warmth by month"), &plot);

row.spark(t!("Trend"), &quote.closes);
```

A `Plot` is drawn as each series is added, so a plot with many points belongs in a derived value, made on another thread whenever its data changes: `#[derived(Prices::plot, from = [prices])] plot: Derived<Plot>`, then `if let Some(plot) = self.plot.get() { ui.chart(…, plot) }`.

`line` joins points, `bars` stands a bar from zero at each x, and `candles` draws a period's open, high, low and close at each place from 0, in the ok ink where it closed higher and the danger ink where it closed lower. A plot is as many rows tall as `Plot::new` says, and its scale is chosen for that height so each labelled round number falls in the middle of a row, with a faint rule across at each; `names` puts words along the bottom in place of numbers. Every series is drawn in a semantic ink, so the theme restyles it.

## Mouse and keys

None yet: a chart shows. Readouts under the pointer are not done.

## Narrow windows

A chart is as wide as it is given and as tall as it says. Labels along the bottom that would touch are left out.

## Screen reader

The chart's label says what it shows; each scale reads its labels in order. A sparkline says how many values it has and its first and last.
