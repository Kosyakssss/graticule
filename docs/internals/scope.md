# Scope, small

Scope has two halves: a recorder inside every app (`kernel::scope`) and a window of its own (`crates/scope`) that attaches to a running app. The small version records what each frame cost and why it was drawn, and shows Time and Why it redrew. The rest of spec §13.6 (rewind, code, facts, why it's this size, commands, taste, effects and jobs, the overlays in the app) is build-plan item 19.

## The recorder

The shell keeps a `Recorder` for the window. Each input is timed; when it asks for a frame, its cause is noted (the first cause since the last frame wins), and the time spent on input is added to the next frame's `update`. Each frame records a `Stamp`: its number, when it was drawn, why, and the time in microseconds spent on input, building the view, laying it out, painting, handing quads to the GPU, after the frame (effects, the input method, accessibility) and in all. The runner measures view, layout and paint itself and gives them to the shell through `Face::stages`; a face that doesn't measure them has its whole paint counted as painting.

A stamp goes into a fixed ring of 1024, preallocated, under an uncontended lock. While Scope is attached it is also offered to a bounded channel with `try_send`, which neither blocks nor allocates; if Scope falls behind, stamps are dropped rather than slowing the app. Measured in release on the Windows machine: 39 ns a frame, most of it the one clock read (28 ns) for the frame's time; the shell's six clock reads around the stages cost about as much again.

## How Scope finds an app

At launch the recorder listens on a port on 127.0.0.1, chosen by the system, and writes the app's title and the port to `Graticule/Scope/<process id>` in the user's data folder. The file is removed when the app closes normally; Scope removes any whose port no longer answers. Nothing listens beyond this machine, and nothing is sent until Scope connects. A thread waits for the connection, blocked, so idle costs nothing. It serves one Scope at a time.

## The wire

Text, one record a line, tab-separated: `launch <stage> <ms>` for each launch stage, then `frame` followed by the stamp's fields in order (`Stamp::line`, `Stamp::read`). On connecting, Scope gets the launch stages and the whole ring, oldest first; after that, each new frame as it is drawn. The connection's end is noticed at the next frame the app tries to send.

## Scope's window

Scope is an ordinary Graticule app (`Scope`) with a thin face around its runner that owns the connection, as Specimen's face owns its runners. A thread reads the lines and wakes the window, at most once until the window has caught up, so an app drawing a thousand frames a second doesn't wake Scope a thousand times. On waking, the frames arrive as one internal command; Scope keeps the last 600, newest first. Its views are Running apps, Time (the slowest 1% against the 1 ms budget, launch stage by stage, and every frame's stages with a bar in block characters, a tenth of the budget per cell) and Why it redrew (the causes tallied, then frame by frame).

## Not yet

The laws of §13.6 (with Scope off the frame time is the same; with Scope on the app stays within budget; Scope shows and subtracts its own cost) compare frame times with and without Scope, which only means something on the reference machine in release builds; they wait for it. Assay's budgets (`assay.md`) already judge the frames of every app. History and source locations, recorded only in development builds, come with item 19.
