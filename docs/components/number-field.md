# Number field

A whole number in the facts, typed as a number or as a sum and stepped by one with the arrows.

## Use it for

A count or an amount a person knows or works out: copies to print, a width in pixels, a price in cents.

## Code

```rust
ui.labelled("Copies", |ui| ui.number(Catalog::copies));
```

The fact is a whole number, such as a `u32`. While a person types, what they typed is the field's own; nothing in the facts changes. Return, leaving the field, or an arrow works it out (`12*4+3` comes to 51) and sets the number by one edit, one undo step. A sum that isn't a whole number, or a number past what the type holds, is said in the message line and changes nothing.

## Keys

The platform's own text editing. Return works out what was typed; Up and Down step the number by one; Escape puts back what was there before typing, and a second Escape leaves the field.

## Narrow windows

At least ten columns; a long number scrolls inside it.

## Screen reader

A spin button, with the number, and the least and most it can be.
