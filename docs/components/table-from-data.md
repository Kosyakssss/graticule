# Table from data

A table made from the records themselves: a column for each field, headed by its name. Nothing is described twice; a new field in the record is a new column.

## Use it for

Looking through records whose fields all matter, early in an app or in tools where the data is the point: a log, an inspector, an import being checked. When some fields need hiding, words of their own or a tone, describe the cells with a [table](table.md) instead.

## Code

```rust
#[session]
sort: Sort,
#[derived(City::order, from = [cities, sort])]
order: Derived<Order<City>>,

ui.records(self.order.of(&self.cities), Atlas::picked).sort(Atlas::sort);
```

`Order::sorted` sorts by the field a column is named for, numbers as numbers and words without regard to case, with items that have nothing in the field last:

```rust
impl City {
    fn order(cities: &List<City>, sort: &Sort) -> Order<City> {
        Order::sorted(cities, sort)
    }
}
```

A field named `people` is headed "People". Numbers line up on the right with their thousands grouped, yes-or-no fields say Yes or No, an enum says its option's name, and a list says how many it holds. The record's own key is left out.

## Mouse, keys and narrow windows

As a table's.

## Screen reader

As a table's: a column header for each field, and each cell says its value.
