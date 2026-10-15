# Picker

A field to type in over the items that match what is typed, best first, with the letters typed marked in each. The same search as the palette's, for the app's own things: files to open, places to go to, symbols to add.

## Use it for

Choosing one of many things by name, when the user knows roughly what it is called: opening a file, going to a page, finding a symbol.

## Code

```rust
#[session]
filter: Text,
#[derived(File::found, from = [files, filter])]
found: Derived<Order<File>>,

ui.picker(self.found.of(&self.files), Editor::filter, |ui, file| {
    ui.text(&file.name);
    ui.muted(&file.folder);
})
.on_choose(Cmd::Open);
```

Which items match, and in what order, is worked out on another thread whenever the list or the filter changes, so a picker over millions of things never stalls the window; until the new order arrives it shows the last one. `Order::found` keeps the items whose words, given by the function, hold the letters typed in order, and puts first the ones where they start words or sit together:

```rust
impl File {
    fn found(files: &List<File>, filter: &Text) -> Order<File> {
        Order::found(files, filter.as_str(), |f| &f.name)
    }
}
```

Case is ignored unless the filter has a capital letter. With nothing typed, every item shows in the list's order.

## Mouse

A click on an item chooses it. The wheel over the items scrolls them.

## Keys

The field takes the typing. Up and Down move the highlight, Page Up and Page Down by eight, and Return chooses the highlighted item. Escape empties the field.

## Narrow windows

The field and the items take the full width; each item is a row, laid out like any other.

## Screen reader

The items are a list box whose options say which one is highlighted. When nothing matches, the picker says so.
