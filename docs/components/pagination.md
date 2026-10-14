# Pagination

Links to the pages of something that comes in pages: the first, the last, the ones beside the current one, and the previous and next.

## Use it for

Only where pages are the data's own shape: the pages of a printed document, results a server hands out a page at a time. A list scrolls instead.

## Code

```rust
ui.pages(Catalog::leaf, 12);
```

The fact is a `u32` counting pages from 0; on screen they count from 1. Going to a page is an edit and one undo step. Previous on the first page and Next on the last are shown, faint, and do nothing.

## Keys

Tab reaches each link; Space or Return goes there.

## Narrow windows

The links wrap onto more rows.

## Screen reader

Each page is a link with its number, and the current one says it is selected.
