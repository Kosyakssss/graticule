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

The keyboard works too. Tab moves between the field, the buttons and the list; the arrows move through the list, Shift with the arrows chooses several items, and Escape lets go. Typing into the field works the way it does everywhere else on the computer, copy and paste included, and Return adds the item. The to-do app's own keys are in `apps/todo/keymap.txt`, a plain file anyone can read and change: Delete or `d d` removes the chosen items, Space ticks one, and Ctrl-Shift-P (Cmd-Shift-P on a Mac) opens the palette. After the first key of a sequence, the bar at the bottom shows what can come next. The palette lists every command with its keys; typing narrows it, Return runs the highlighted one, and one that can't run says why. With several items chosen, "Remove 4 items" removes them all at once, skipping and naming any it can't. Ctrl-Z (Cmd-Z on a Mac) undoes and Ctrl-Shift-Z redoes: a typed word, an added item, a removal of four items at once, each as one step, and undoing puts the chosen items and the place in the field back as they were.

The app keeps what you did. Every change is saved as it happens, so closing the window loses nothing and a crash loses at most the change being written at that moment; the next launch opens with the same list, the same item chosen and the list scrolled where it was. Each app keeps its things in its own folder in the computer's place for app data (on Windows, `AppData\Roaming\Graticule\todo`). If something there can't be read, it is moved aside rather than deleted, and the app says so.

Clearing the finished items of a very long list doesn't freeze the window. The search happens in the background, the items go a few hundred at a time between frames, and the line above the bar counts them down ("“Clear finished”: 10000 of 33333"). Undo while it runs stops it and puts everything back; once it's done, it is one step like any other.

Export… asks where to save the list, the way every app on the computer does, and then writes it in the background as a Markdown checklist. The line above the bar says "Saving “To do.md”" while it works and the message line says where it went. Calling off the save panel does nothing at all.

`cargo run -p todo -- --specimen` opens Specimen over the to-do app instead: the same window, but showing the app's named states (empty, a few things, a very long list, long titles), each one live on its own copy, so you can click and type without touching your real list. The bar has a "Show …" command for each state. Export shows what it would have done instead of opening the save panel. Specimen's other places are in the bar too: Components, a live page of every piece Graticule draws with (the text, buttons, links, boxes, fields and lists). Each piece has its own page too: what it is for, how app code asks for it, its keys, what a screen reader says, the piece itself to try, and the piece with awkward content (nothing, very long text, right-to-left, Chinese, emoji, a huge number). The pages are written in `docs/components/`, so a change to the words shows in Specimen the next time it is built. Style and Changes come next. `cargo run -p specimen` opens Specimen on its own, on Components.

`cargo run -p todo -- --render <folder>` draws every named state of the app, light and dark, at its normal size and at the smallest window it allows, into pictures in that folder. The smallest window is worked out from the layout itself, so nothing is ever cut off by making the window small.

## How speed is measured

A screen refreshes 60 to 1000 times a second; at 1000, there is one millisecond per picture. Graticule's goal is to finish its work for an ordinary interaction in a tenth of that, and to do nothing at all when nothing is happening, which is what saves battery. Times are measured automatically and judged by the slowest frames, not the average, because the slow ones are the ones you notice.

## How you review

- **Pictures**: Specimen's Changes page, before and after, with the change described in words.
- **Numbers**: the budgets, which say plainly when something got slower.
- **Sentences**: this guide, and `docs/api.md`, the code we want app makers to write. If you can follow it, it's right.
