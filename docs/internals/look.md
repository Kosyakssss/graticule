# Looks: themes and styles

`kernel::look` reads theme and style files, keeps which of them the user chose, and gives every frame its `Look`: the theme's colors in the appearance shown, and the style.

## The files

A **theme** (`.theme`) is a name and every ink in `[light]` and `[dark]`, each as `oklch(lightness chroma hue)`. All fourteen inks must be given in both appearances; a missing, doubled or unknown ink is refused with its line. A theme whose text inks don't reach their contrast on the background (4.5:1, faint 3:1, by `Ink::needs`) is refused too, with the ink and both numbers, so a theme that loads always passes the contrast law. The kernel's laws hold this over generated themes: a theme is refused exactly when an ink misses, and one that is accepted prints a file that reads back as the same theme.

A **style** (`.style`) is a name and three values: `lines` (thin, regular or heavy: the edges of boxes and rules), `shadow` (the hard shadow's depth as a share of a row, 0 for none) and `check` (box or tick). They change how things are drawn, never where, so a style can't move anything off the grid; a value that would (corners that cut into cells, density) is not a style value. More values arrive with the components that use them, such as the glyphs of tabs.

Graticule ships two themes, Graticule and Contrast, and two styles, Graticule and Flat (`crates/kernel/looks/`), compiled in. The first of each is the default, and `ink::LIGHT` and `ink::DARK` are the default theme's appearances, for the many places (offscreen pages, proofs, Assay) that draw in it.

## Choosing

The user's own files live in the looks folder, shared by every Graticule app (`<data>/Graticule/looks`, beside the apps' own folders). A file whose name matches a built-in theme or style replaces it. The choice of theme, style and appearance (the system's, light or dark), and how dates are written (`dates = absolute` or `relative`), is kept there in `look.txt`, so switching in one app switches all of them the next time they start.

The shell owns the `Looks`. It reads them once before the first frame and gives the face `Face::offered`. The built-in commands that switch the look are `light-appearance`, `dark-appearance`, `system-appearance`, `next-theme` and `next-style`, and beside them `absolute-dates` and `relative-dates`, which switch how every date is written (`docs/internals/shell.md`, "Dates that move"). They are in the registry like any other command: in the palette, bindable in a keymap, unavailable with a reason when they would do nothing. They don't change facts, so they are never undo steps; undoing a switch is the opposite switch. Running one leaves the switch in the runner. After each input the shell takes it (`Face::switch`), applies it, writes `look.txt` and draws again. A runner whose theme or style differs from the last frame's redraws everything it keeps.

## The editor

Specimen's Style place edits Graticule's own theme and style. The inks (in thousandths of lightness and chroma and tenths of a degree of hue, so every step lands on a number the file writes exactly) and the three style values are Specimen's facts, so every step is a command and undoes like any other. The steps for the chosen ink are lighter, darker, more color, less color, and the hue forward or back. They go to the appearance being changed (light or dark, switched by command). A step that would leave any text ink below its contrast is unavailable, and the reason gives the ratio, so the editor can't make a theme the file would refuse.

While anything differs from the built-in look, Specimen draws in the edited one (`Face::look`), so every component page and every screen follows each step. Saving writes `graticule.theme` and `graticule.style` to the looks folder. They take the built-in ones' names and so replace them for this user, and every running Graticule app redraws in them through the watcher.

The blast radius (spec §13.5) is counted once, on a thread, the first time Style opens. Every named state is painted in a coded theme in which each ink is a color no other ink has, and the colors of the quads say which inks each screen uses. The column "Drawn" then reads "on 12 of 18 screens". The count doesn't depend on the inks' values, so editing never makes it stale.

`Face::look` lets a face draw in a look other than the chosen one; Specimen's style editor uses it to show a theme or style as it is being edited.

## Editing live

The shell watches the looks folder with the system's file notifications (`notify`), so watching costs nothing while nothing changes, as the idle law asks. When a `.theme`, `.style` or `look.txt` file changes, the watcher's own thread reads the folder afresh and sends the new `Looks` to the event loop. The shell swaps it in and draws again only if it differs from the old one, so writing `look.txt` after a switch, which the watcher also sees, costs one comparison. Every Graticule app that is running sees a saved file at once.

A file in the looks folder that can't be read doesn't stop the app. The message line says it once, not again on every later change. It is left out, and the message line says which file and why.
