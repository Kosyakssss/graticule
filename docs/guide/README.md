# Graticule in plain words

This guide explains Graticule for someone who doesn't write code. It grows a little every time something lands that changes what you can see or do.

## What Graticule is

A toolkit for making desktop apps. You describe what your app knows, what can happen in it and how it should look; Graticule handles everything between that description and the pixels: the window, the mouse and keyboard, text, scrolling, drawing on the graphics card, accessibility, undo.

It's built on a grid, like graph paper. Every box, line and button in an app sits exactly on the squares. Inside a box, anything goes: a photo, a chart, a map, a waveform. The grid keeps every app tidy; the inside of the boxes is where each app is itself.

## The scoreboard

Think of a stadium scoreboard.

- **The facts**: home 2, away 1, 67th minute. Just information.
- **The commands**: "goal for home", "minute passes". The only things that can change the facts.
- **The view**: the rule for how the facts look on the board. Big yellow digits, the clock in the corner.

Nobody paints the board by hand. Someone presses "goal for home", the facts change, and the board redraws itself from the new facts. A Graticule app works the same way, and that one idea gives a lot for free:

- **Undo** is going back to older facts.
- **Rewind** in Scope is scrolling through the facts over time.
- **Specimen** can show any screen in any situation, because a situation is just a set of facts.
- **Sync** between your devices sends what each command actually changed ("added this item here"), not the command itself ("clear the finished ones"), because the same command can mean different things on two devices. So the lists on your laptop and your phone always end up the same, and nothing gets lost when both change at once.
- **Speed**: when one fact changes, only the part of the screen that shows it is redrawn.

## The tree

Between the view and the pixels sits a tree: a list of every box on screen, what kind of thing it is, where it is on the grid, and where in the code it came from. It's what the web calls the DOM, but rebuilt from the facts each time and much simpler, because the grid removes most of the measuring.

Everything reads that one tree: drawing, clicking, screen readers, Specimen, Scope, the taste checks, printing.

## From a click to the screen

Say you tick an item in a to-do list.

1. Graticule looks up which box is under the mouse in the last tree. Every square of the grid already knows its box, so this is instant.
2. The box knows its command: "mark ‘buy milk’ done". Graticule checks the command is still allowed, then runs it, and the facts change.
3. Ledger writes down exactly what changed (this item, done is now yes), which is what undo, saving and sync use later.
4. The view builds a fresh tree from the new facts. In a list of ten thousand items only the rows on screen are built.
5. Graticule compares the new tree with the last one, finds that one row changed, and redraws only that row.

Typing works the same way: each keystroke is a small command that changes the field's text, so it can be undone like anything else. When nothing happens, none of this runs.

A list of fifty million items behaves the same. Lists keep running counts (how many items are done), so questions like "is anything finished?" are instant. Work that really does have to touch every item, like clearing all the finished ones or exporting the whole list, runs in the background with its progress in the status line, and the app keeps responding while it runs.

## Commands are everywhere at once

Every command has a name and a label. From that single list Graticule makes the action bar, the help page, the palette, the menus, the right-click menus and the keys. Keys live in a separate file you can edit while the app runs. Press the first key of a sequence and the bar shows what can come next, like Helix.

The same list lets scripts and AI tools drive any Graticule app completely, which is why things like voice control don't need to be built in.

## The units

Graticule is made of named parts, each with a model code. Every one is two things: a piece that apps build in, and something of its own you can open or run. So every unit also shows Graticule working.

- **DK-01 Kernel.** Graticule itself: the grid, drawing, keys and mouse, the window. On its own: a new empty app.
- **LD-01 Ledger.** The app's memory. Every command is written down in order, along with exactly what it changed; from those come saving, undo, rewind, crash replays and syncing between your devices. On its own: a command that shows any app's history.
- **GA-01 Galley.** Text: huge files, many cursors, spelling, good typesetting, Helix-style keys. On its own: a text editor. Named for the tray typesetters set lines of type in.
- **PL-01 Plotter.** The canvas, modelled on tldraw: shapes, arrows that stay attached, snapping, endless boards, maps. On its own: a whiteboard. Named for the pen plotters that draw lines on paper.
- **SP-01 Specimen.** The catalog. Every component, live; every screen of every app in every state; the style editor; and the Changes page, where you see what looks different since you last approved it and say yes or no. This is where you review work without reading code.
- **SC-01 Scope.** The microscope over a running app: why it redrew, how long it took, where the code for any box is, and a rewind slider. It costs the app nothing measurable.
- **AS-01 Assay.** The inspector of quality. It checks any app against the rules every Graticule app must follow: never crashes, undo always works, everything on the grid, readable contrast, fast enough. On its own: a report for any app. Named for the test of what a metal is made of.
- **EN-01 Engraver.** Turns drawings (SVG files) into icons inside the app.
- **BD-01 Bindery.** Makes a real Mac app out of the code, installs and updates it, and reloads the app the moment code is saved while someone is working on it.

## The demo apps

Galley's editor and Plotter's whiteboard are already apps. Beside them, six small apps each push Graticule somewhere hard: Twin (files), Ticker (fast live data), Atlas (maps), Patchbay (wires and boxes, built on Plotter), Lightbox (photos and scans) and Four-track (recording music). They live next to Graticule, so any change to it shows in all of them at once.

## The order things are built in

The build plan, at the end of the spec, is one list from top to bottom, and anyone (a person or an AI) asked to build Graticule works down it item by item.

1. **A first slice through everything**: a tiny to-do app that runs in a real window with undo, saving and the palette. It isn't a demo app; it's the smallest thing that touches every layer, so design mistakes show up while they're cheap.
2. **The tools**: Specimen properly, Scope and Assay small, the development loop, icons.
3. **The components**: every one from HeroUI and Blueprint, built in batches, each with its page in Specimen. The owner then removes the ones Graticule doesn't keep.
4. **The units' engines**: Ledger in full with sync, Galley, Plotter.
5. **The demo apps**, each one using every component it touches.
6. **Linux, then Windows.**

## What runs today

`cargo run -p todo` opens the to-do app. It shows its page: the field, the list and the buttons, with the action bar along the bottom listing every command, and a status line that says what the thing under the pointer does and, when it can't, why. A button that can't run right now is drawn faint instead of hidden. Ticking an item, removing one, clearing the finished ones and scrolling a long list work with the mouse.

The keyboard works too. Tab moves between the field, the buttons and the list; the arrows move through the list, Shift with the arrows chooses several items, and Escape lets go. Typing into the field works the way it does everywhere else on the computer, copy and paste included, and Return adds the item. The to-do app's own keys are in `apps/todo/keymap.txt`, a plain file anyone can read and change: Delete or `d d` removes the chosen items, Space ticks one, and Ctrl-Shift-P (Cmd-Shift-P on a Mac) opens the palette. Those are the keys the app ships with; anyone using it can change them without rebuilding it. "Edit the keys" in the palette opens the user's own copy of the file in the system's text editor, and every change applies the moment the file is saved. A line the app can't read is named in the message line and the keys that worked stay in use; deleting the file brings back the keys the app shipped with. A number typed first applies a key to that many rows: `3 d d` removes the focused item and the two below it, as one step that one undo takes back, and `3` then the down arrow moves three rows. The bar at the bottom shows the number while it waits for the key, and Escape lets go of it. A keymap can also have modes, as text editors like Vim do: in normal mode letters run commands, in insert mode they type, and in select mode moving extends the choice. The bar at the bottom always shows the mode, and a pair like `d j` removes the focused item and the one below it. None of the demo apps uses modes yet; the keymap file decides. After the first key of a sequence, the bar at the bottom shows what can come next. The palette lists every command with its keys; typing narrows it, Return runs the highlighted one, and one that can't run says why. A command that needs something typed, like "Rename…", asks for it right there: the palette names what it wants (some text, a whole number, yes or no), and if what was typed doesn't fit, it says so and asks again. "Show the help" in the palette lists everything the app can do, with the keys for it and, for anything that can't be done right now, why; the arrows and Page Up and Down move through it and Escape closes it. "What's this?" turns the next click into a question: instead of pressing the button, it says what the button does, its keys and whether it can run now. "Show what was done" lists everything done in the window, newest first, with the time: each change, each undo and redo, each switch of the look. An app's settings come straight from its data: each one gets the right control by its kind (a switch, a set of options, a number with minus and plus, a field), each change can be undone on its own, and "Show only the changed settings" hides everything still as it was at first. With several items chosen, "Remove 4 items" removes them all at once, skipping and naming any it can't. Ctrl-Z (Cmd-Z on a Mac) undoes and Ctrl-Shift-Z redoes: a typed word, an added item, a removal of four items at once, each as one step, and undoing puts the chosen items and the place in the field back as they were.

The app keeps what you did. Every change is saved as it happens, so closing the window loses nothing and a crash loses at most the change being written at that moment; the next launch opens with the same list, the same item chosen and the list scrolled where it was. Each app keeps its things in its own folder in the computer's place for app data (on Windows, `AppData\Roaming\Graticule\todo`). If something there can't be read, it is moved aside rather than deleted, and the app says so.

Clearing the finished items of a very long list doesn't freeze the window. The search happens in the background, the items go a few hundred at a time between frames, and the line above the bar counts them down ("“Clear finished”: 10000 of 33333"). Undo while it runs stops it and puts everything back; once it's done, it is one step like any other.

Export… asks where to save the list, the way every app on the computer does, and then writes it in the background as a Markdown checklist. The line above the bar says "Saving “To do.md”" while it works and the message line says where it went. Calling off the save panel does nothing at all.

`cargo run -p todo -- --specimen` opens Specimen over the to-do app instead: the same window, but showing the app's named states (empty, a few things, a very long list, long titles), each one live on its own copy, so you can click and type without touching your real list. The bar has a "Show …" command for each state. Export shows what it would have done instead of opening the save panel. Specimen's other places are in the bar too: Components, a live page of every piece Graticule draws with (the text, buttons, links, boxes, fields and lists). Each piece has its own page too: what it is for, how app code asks for it, its keys, what a screen reader says, the piece itself to try, and the piece with awkward content (nothing, very long text, right-to-left, Chinese, emoji, a huge number). The pages are written in `docs/components/`, so a change to the words shows in Specimen the next time it is built. Style shows every ink of the theme, light and dark, with its numbers and how well text in it reads on the background, and the sizes the grid and the type are built from; you can look but not yet change them. Changes lists every screen that looks different from the last time you approved it, at its normal and smallest size, and says what changed in words ("the button “Add” now says “Add it to the list”", "the window is now 41 by 17 cells, was 33 by 17"); Approve keeps the new look as the one to compare against. `cargo run -p todo -- --changes` says the same without a window, and `--approve` approves everything as it is. Pictures of before and after come later. `--render <folder>` also writes a page, `index.html`, with every state, light and dark, normal and smallest, and what changed under each; add `--references <folder>` to pin your own pictures (a mockup, a screenshot of another app) beside the state they are named after, such as `Empty, the mockup.png`. `cargo run -p specimen` opens Specimen on its own, on Components, and `cargo run -p specimen -- --render <folder>` draws all of its pages into pictures.

`cargo run -p todo -- --render <folder>` draws every named state of the app, light and dark, at its normal size and at the smallest window it allows, into pictures in that folder. The smallest window is worked out from the layout itself, so nothing is ever cut off by making the window small.

`cargo run -p scope` opens Scope. It lists the Graticule apps running on this computer; attach to one and Scope shows, live, how long each of its pictures took and what it spent the time on, whether the slowest one in a hundred stays inside the millisecond, how long the app took to start, and why each picture was drawn at all (a key, the pointer moving, a background job finishing). An app can only be reached from the same computer, and sends nothing until Scope asks.

`cargo run --release -p todo -- --assay` runs Assay over the to-do app, and the same works for every app. Assay plays the app like a careless, tireless tester: from each named state it presses keys, types, clicks whatever is on screen and runs commands by name, about four thousand steps in all, always the same ones so a result can be repeated. It checks that nothing crashes, that undo puts things back and redo puts them forward again, that a button which looks usable does something, that the app's private commands can't be reached, that what is saved opens again, and that every command can be reached at all. It looks at every state at its normal and smallest size for anything sticking out of its box, and checks that every text color is readable on the background. It times every picture and judges the slowest one in a hundred against the millisecond. When something fails, it cuts the steps down to the fewest that still show it and prints them in words ("click the button “Clear finished”"). Its first run found four real bugs in Graticule itself, now fixed: one made a long list with everything chosen slow, and two made undo wrong in Specimen.

How everything looks can be changed while an app runs, from the palette: "Use the dark appearance", "Use the light appearance" and "Follow the system's appearance", "Use the next theme" (the colors: Graticule's own, or Contrast, pure black and white) and "Use the next style" (how things are drawn: Graticule's own, or Flat, with thinner lines, no shadows and ticks in ticked boxes). The choice holds for every Graticule app and is remembered. Themes and styles are small text files; your own go in the looks folder (on Windows, `AppData\Roaming\Graticule\looks`). Every running app changes the moment one of them is saved. Specimen's Style place changes them by command: choose an ink, then make it lighter or darker, give it more or less color or turn its hue, in the light or the dark appearance, and deepen the shadow or change the lines and the check mark; everything in Specimen follows at once, each step can be undone, a step that would make text hard to read can't be taken and says why, and each ink says on how many of the app's screens it is drawn. "Save the theme and the style" writes them for every app. A theme whose text would be hard to read on its background is refused, with the color and how far it falls short.

Text on a page can be chosen with the mouse, across paragraphs and values, and copied with the usual copy keys or "Copy the chosen text" in the palette; Escape lets it go. Typing Chinese, Japanese or Korean through the system's input method shows the characters being composed, underlined, in the field, with the system's candidate list beside them. On a Mac, a right click on a row opens a menu of that row's commands, the same ones the bar shows, and a right click elsewhere offers the page's own; this hasn't been tried on a Mac yet. On Windows and Linux Graticule draws the same menu itself, a box with a hard shadow hanging from where you clicked: the arrow keys move through it, Return runs an entry, and Escape or a click elsewhere closes it.

Values look the same in every app, so you can tell what something is before reading it: a count with its thousands grouped (1,284), a date, a version in a dotted box, a state such as OFFLINE filled with its color, a product code such as DK-01 in a heavy box, a piece of code on a gray ground, a mode underlined, and an error in red. The boxed ones only say what something is; clicking them does nothing. Specimen shows them all on its Values page. Every date in every app is written one way, chosen in the palette: "Write dates as the day and time" (2026-06-01 10:00) or "Write dates as how long ago" (5 min ago, yesterday, 3 weeks ago). The choice holds for every app, beside the look. Dates written as how long ago keep themselves current: the window wakes once, at the moment one of them would read differently, and otherwise stays asleep.

Apps can show pictures: icons and small drawings, made as SVG files in any drawing program and turned into part of the app when it is built, so they cost nothing to load. Graticule comes with 42 icons for everyday things (add, close, search, folder, undo, warning and so on), which Specimen shows on its Picture page. A picture is drawn in the theme's inks, so it turns light on dark with everything else, and it always has a name that screen readers say. `cargo run -p engraver -- FOLDER` says what it would make of a folder of drawings, and refuses, with the reason, a drawing without a name or in a color that isn't one of the theme's.

`cargo run -p bindery -- new apps/hello` starts a new app: a window with a counter, two commands and one named state, ready to change, which already passes Assay and opens in Specimen. `cargo run -p bindery -- dev apps/hello` runs it and watches the code: every time a file is saved it builds the app again, and if the build works, the window closes and reopens with the new version in the same place, at the same size, with the same things in it. If the build fails, the errors show in the terminal and the app you have keeps running.

Some of what an app shows changes on its own: prices from a market, the level of a microphone, a download's progress. Such a value is written by its own source as often as it likes, and the window wakes to show it at most once per picture, never more. Other values are worked out from the app's facts, a chart from thousands of prices or a count over a long list. They are worked out in the background, only when what they come from has changed, so the window never waits for them; until the new one is ready the app shows the last one, and the status line says "Working out …" with its name.
An app can also run work of its own in the background, such as a feed that keeps prices current or a search through many files. Every such job shows by its name in the status line while it runs, and if one stops without finishing, the message line says so.
Doing something to thousands of chosen items at once, or undoing a big change, happens a little at a time, between pictures, with how far it has got in the status line, so the window never stops answering; it is still one step to undo.
After an undo, or when something finishes in the background and changes the list, the rows that changed carry a small blue bar at their right edge until you next press a key or click, so you can see what moved without watching for it.

The first wave of further components is in Specimen, each on its own page: headings, separators, the keys of a command drawn from the keymap, progress and level bars, placeholders for content still on its way, tags, a person's initials, boxed messages marked with an icon in their color, and cards that box a group under a title. Then the controls: buttons that stay pressed in while something is on, buttons joined into one, buttons with a picture instead of words, switches for settings, radio buttons, groups of controls under a heading, and labels above fields with a line of help below that a screen reader reads too. Last, the ways of moving around: tabs over the pages of a panel, the places of an app along the top, page numbers, sections that open in place, a trail of where you are that shortens in the middle when it is long, and a row of actions that puts what doesn't fit into a "More…" menu. Every list now marks its top or bottom edge with a rule when there is more beyond it. Moving between tabs, places and pages can be undone like any change.

Fields now come in four shapes: one line, a search field with a magnifying glass that Escape empties, a text area of several lines that wraps and grows as you type, and text that reads as text until you click it and then becomes a field, for renaming things where they are.

Numbers have fields of their own. You can type a sum such as 12*4+3 and the field works it out when you press Return or move on; the arrows step it by one, and Escape takes back what you typed. A slider sets a number by dragging a handle along a track, with the number written after it. A code field shows one box per character, for the six digits a sign-in sends.

Apps can take files dragged in from elsewhere on the computer: while they hover over the window, the status line says what dropping them will do ("Import a file"), or why it can't. A list can let you drag its rows into your own order: a line shows where the row will land, the status line says the move, Escape puts it back, and the move undoes in one step. Dragging a file out of a Graticule app onto the desktop isn't possible yet, because the windowing library underneath can't start that kind of drag.

## How speed is measured

A screen refreshes 60 to 1000 times a second; at 1000, there is one millisecond per picture. Graticule's goal is to finish its work for an ordinary interaction in a tenth of that, and to do nothing at all when nothing is happening, which is what saves battery. Times are measured automatically and judged by the slowest frames, not the average, because the slow ones are the ones you notice.

## How you review

- **Pictures**: Specimen's Changes page, before and after, with the change described in words.
- **Numbers**: the budgets, which say plainly when something got slower.
- **Sentences**: this guide, and `docs/api.md`, the code we want app makers to write. If you can follow it, it's right.
