# Table

A list whose rows are cells in named columns, under a row of heads. Like a list, it is scrolled by whole rows and builds only the rows that show, so a table of millions of rows costs what a screenful does. Pressing a head sorts by its column; the order is worked out in the background, never while drawing.

## Use it for

Records with several values each that the user compares down a column: files and their sizes, cities and their populations, quotes and their prices.

## Code

```rust
#[session]
sort: Sort,
#[derived(City::order, from = [cities, sort])]
order: Derived<Order<City>>,

ui.table(self.order.of(&self.cities), Atlas::picked, |row, city| {
    row.cell("City", &city.name);
    row.cell("Population", city.population);
    row.cell("Growth", city.growth).tone_by_sign();
})
.sort(Atlas::sort);
```

Every row gives the same cells in the same order; the first row shown names the columns. Words read from the left; numbers, days and times line up on the right in the monospace face, their thousands grouped. `tone_by_sign` draws a number in the ok ink above zero and the danger ink below.

The order is a derived value, worked out on another thread from the list and the `Sort` whenever either changes; until it arrives the table keeps the order it had:

```rust
impl City {
    fn order(cities: &List<City>, sort: &Sort) -> Order<City> {
        match sort.column() {
            "Population" => Order::by(cities, sort, |c| c.population),
            "City" => Order::by(cities, sort, |c| c.name.clone()),
            _ => Order::of(cities),
        }
    }
}
```

`Order::only` leaves out the items it says no to, which is how a table is filtered. A list takes an order the same way.

## Mouse

A click on a head sorts by its column, a second the other way, a third puts it back. Dragging the gap after a head makes its column wider or narrower. Rows are chosen as in a list.

## Keys

Tab reaches each head, and Return or Space presses it. In the rows, the keys are a list's.

## Narrow windows

Columns share the width, each at least as wide as its head. The columns that don't fit are left out from the right; the first always shows.

## Screen reader

Says "table" and how many rows it has; each head is a column header that says which way it is sorted, and each cell says its value.
