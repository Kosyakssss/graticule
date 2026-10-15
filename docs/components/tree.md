# Tree

A list whose rows sit inside one another: folders and what they hold, sections and their pages. Each branch opens and closes; like a list, only the rows that show are built, and which rows show is worked out in the background.

## Use it for

Things that hold other things, where the user goes down into some and leaves the rest closed: a project's files, an outline, a hierarchy of places.

## Code

```rust
#[session]
open: Vec<Key<Part>>,
#[derived(Part::shown, from = [parts, open])]
shown: Derived<Order<Part>>,

ui.tree(self.shown.of(&self.parts), Outline::chosen, Outline::open, |ui, part| {
    ui.text(&part.name);
});
```

The items are one flat list, each naming the item it is inside, so moving one is changing a field. `Order::tree` lays them out as rows, items without one at the top, each branch's items under it while its key is in `open`:

```rust
impl Part {
    fn shown(parts: &List<Part>, open: &[Key<Part>]) -> Order<Part> {
        Order::tree(parts, open, |part| part.inside)
    }
}
```

## Mouse

A click on a row's twig opens or closes its branch; a click elsewhere on the row chooses it, as in a list.

## Keys

The keys of a list, and the platform's keys for trees: Right opens a closed branch and goes into an open one, Left closes an open branch and goes up from a row inside one. Both are in the palette as "Open the branch, or go into it" and "Close the branch, or go up to it".

## Narrow windows

A tree takes the rows the page has to spare, at least three. Rows inside others are indented by two columns for each level.

## Screen reader

Says "tree"; each row is a tree item with its level, and a branch says whether it is open.
