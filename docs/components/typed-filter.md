# Typed filter

A field that filters records by their own fields: `people>1000000 country:spain`. The fields it knows are the record's, so it needs no setting up, and it offers them as they are typed.

## Use it for

Narrowing a table of records that have several fields worth filtering by: quotes by sector and price, files by kind and size, tasks by who and when.

## Code

```rust
#[session]
query: Text,
#[derived(City::matching, from = [cities, query])]
matching: Derived<Order<City>>,

ui.filter(&self.cities, Atlas::query);
ui.table(self.matching.of(&self.cities), Atlas::picked, |row, city| { … });
```

The rows that pass are an order worked out on another thread, like a table's sort, and the two chain: `Order::by(cities, sort, …).matching(cities, query)`.

```rust
impl City {
    fn matching(cities: &List<City>, query: &Text) -> Order<City> {
        Order::of(cities).matching(cities, query.as_str())
    }
}
```

Each term must hold. `country:spain` holds when the field contains the word, `country=spain` when it is the word, and `people>1000000`, `>=`, `<` and `<=` compare numbers, which may be written with commas. A word on its own holds when any field of words contains it, a term after `-` holds when it doesn't, and words in double quotes may hold spaces: `country:"united kingdom"`. Case is ignored. Days and times, and words such as `this-week`, are not read yet.

## Mouse

Like a search field: the magnifying glass, and the fields offered under it while a word is typed, each chosen by a click.

## Keys

Down reaches the fields offered and Return takes one. Escape empties the field.

## Narrow windows

The field takes the width it is given; what is wrong with a query wraps under it.

## Screen reader

The field's placeholder names the fields it can filter by. A query that can't be read says why, in a line under the field: a field the records don't have, a term with nothing after it, or a comparison with something that isn't a number.
