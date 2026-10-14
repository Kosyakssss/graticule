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
- **Sync** between your devices is sending commands, so the facts match.
- **Speed**: when one fact changes, only the part of the screen that shows it is redrawn.

## The tree

Between the view and the pixels sits a tree: a list of every box on screen, what kind of thing it is, where it is on the grid, and where in the code it came from. It's what the web calls the DOM, but rebuilt from the facts each time and much simpler, because the grid removes most of the measuring.

Everything reads that one tree: drawing, clicking, screen readers, Specimen, Scope, the taste checks, printing.

## Commands are everywhere at once

Every command has a name and a label. From that single list Graticule makes the action bar, the help page, the palette, the menus, the right-click menus and the keys. Keys live in a separate file you can edit while the app runs. Press the first key of a sequence and the bar shows what can come next, like Helix.

The same list lets scripts and AI tools drive any Graticule app completely, which is why things like voice control don't need to be built in.

## The units

Graticule is made of named parts, each with a model code. Every one is two things: a piece that apps build in, and something of its own you can open or run. So every unit also shows Graticule working.

- **DK-01 Kernel.** Graticule itself: the grid, drawing, keys and mouse, the window. On its own: a new empty app.
- **LD-01 Ledger.** The app's memory. Every command is written down in order, and from that one record come saving, undo, rewind, crash replays and syncing between your devices. On its own: a command that shows any app's history.
- **GA-01 Galley.** Text: huge files, many cursors, spelling, good typesetting, Helix-style keys. On its own: a text editor. Named for the tray typesetters set lines of type in.
- **PL-01 Plotter.** The canvas, modelled on tldraw: shapes, arrows that stay attached, snapping, endless boards, maps. On its own: a whiteboard. Named for the pen plotters that draw lines on paper.
- **SP-01 Specimen.** The catalog. Every component, live; every screen of every app in every state; the style editor; and the Changes page, where you see what looks different since you last approved it and say yes or no. This is where you review work without reading code.
- **SC-01 Scope.** The microscope over a running app: why it redrew, how long it took, where the code for any box is, and a rewind slider. It costs the app nothing measurable.
- **AS-01 Assay.** The inspector of quality. It checks any app against the rules every Graticule app must follow: never crashes, undo always works, everything on the grid, readable contrast, fast enough. On its own: a report for any app. Named for the test of what a metal is made of.
- **EN-01 Engraver.** Turns drawings (SVG files) into icons inside the app.
- **BD-01 Bindery.** Makes a real Mac app out of the code, installs and updates it, and reloads the app the moment code is saved while someone is working on it.

## The demo apps

Galley's editor and Plotter's whiteboard are already apps. Beside them, six small apps each push Graticule somewhere hard: Twin (files), Ticker (fast live data), Atlas (maps), Patchbay (wires and boxes, built on Plotter), Lightbox (photos and scans) and Four-track (recording music). They live next to Graticule, so any change to it shows in all of them at once.

## How speed is measured

A screen refreshes 60 to 1000 times a second; at 1000, there is one millisecond per picture. Graticule's goal is to finish its work for an ordinary interaction in a tenth of that, and to do nothing at all when nothing is happening, which is what saves battery. Times are measured automatically and judged by the slowest frames, not the average, because the slow ones are the ones you notice.

## How you review

- **Pictures**: Specimen's Changes page, before and after, with the change described in words.
- **Numbers**: the budgets, which say plainly when something got slower.
- **Sentences**: this guide, and `docs/api.md`, the code we want app makers to write. If you can follow it, it's right.
