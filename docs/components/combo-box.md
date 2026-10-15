# Combo box

A one-line field with completions listed under it in an overlay while it has the focus: the ones that contain what is typed, a few at a time.

## Use it for

Text that is usually one of a known set but may be anything: a city, a contact, a tag already used elsewhere. When it must be one of a few options, use a select; when it is free text, a text field.

## Code

```rust
ui.combo_box(Catalog::city, Message::from("City"), &CITIES);
```

The app passes the completions, already in the order it wants; the field keeps those that contain what is typed, ignoring case, and shows the first eight. Typing changes the text as in any field, and taking a completion sets it by one edit, named after the field. The completions are built only while the field or one of them has the focus, so nothing in the facts says whether they show. Following spec §4.4 they hang from the field, under it if they fit, else above, and never cover what is being typed.

## Keys

Down from the field reaches the completions; Up and Down move between them, Home and End go to the first and last, and Up from the first goes back to the field. Return or Space takes one and puts the focus back in the field. Escape on a completion goes back to the field; typing there types into the field.

## Narrow windows

The overlay is as wide as its longest completion, never past the window's edges.

## Screen reader

A text field named by its placeholder; each completion is an option.
