# Heading

Text that heads what follows it, in the strong weight. There is one size of text on the grid, so a heading stands out by weight and by the room around it, not by being bigger.

## Use it for

The title of a section of a page. The page's own title is the page's, not a heading.

## Code

```rust
ui.heading("Recent changes");
```

A title with its icon and typed values is a row of them:

```rust
ui.row(|ui| {
    ui.picture(&icons::FOLDER);
    ui.heading(&project.name);
    ui.version(&project.version);
});
```

## Screen reader

A heading, read as it is written, so headings can be jumped between.
