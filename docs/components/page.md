# Page

The app's area inside the window: its title at the top, then its components from top to bottom, one blank row between them. The kernel draws the window's own lines (message line, status line, action bar) below it.

## Use it for

The outermost component of every view. An app has one page per window.

## Code

```rust
ui.page(t!("To do"), |ui| {
    ui.text("Everything the app shows goes inside.");
});
```

## Narrow windows

The page keeps two columns of margin on each side. A window can't be made smaller than the page's narrow form, so nothing on it is ever cut off.

## Screen reader

Says "page" and the title.
