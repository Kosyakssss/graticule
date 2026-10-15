# Side panel

A pane of details beside the page that comes and goes, docked rather than laid over: the page gets narrower while it shows, and nothing is covered.

## Use it for

What belongs to the thing in front of you but isn't needed all the time: an item's details, its history, the settings of the selected layer.

## Code

```rust
ui.switch("Show the details", Catalog::details);
ui.columns(|ui| {
    ui.column(Fill, |ui| {
        ui.text("Groceries");
        ui.muted("Milk, eggs and bread.");
    });
    if self.details {
        ui.column(28, |ui| {
            ui.text("Details");
            ui.muted("Two tags, made on Friday");
        });
    }
});
```

A side panel is not a component of its own: it is a fact for whether it shows, and a column that the view describes only while the fact says so. Whatever shows or hides it, a switch or a command with a key, changes that fact by one edit, so it is undone like anything else and the app opens the way it was left.

## Keys

Whatever shows it has the keys the keymap gives it; Tab reaches what is inside it, after the page.

## Narrow windows

The columns share the window as columns do: the panel keeps its width, and when the two no longer fit side by side the panel goes under the page.

## Screen reader

Its contents follow the page's, in the order they are drawn.
