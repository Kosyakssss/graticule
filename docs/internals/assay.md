# Assay, small

Assay (`crates/assay`) holds every app to the same checks. `assay::assay::<App>(plan)` runs them and returns what it found and measured; `assay::report::<App>()` prints it and fails when anything was found. Every app has it for free: `--assay` on any app's command line (through `specimen::offscreen`, so the to-do app, Specimen and Scope alike). The rest of spec §13.7 (every taste law, color blindness, energy and size) is build-plan item 20.

## The universal laws

Assay drives the app through generated command sequences: by default 96 sequences of 40 steps, spread over the app's named states, which are where they start. A step presses a key (arrows, Tab, Enter, Escape, Delete, the undo keys, the palette), types a character, runs a command by the name the palette and keymaps use (every command in the registry and the kernel's own), clicks a control on screen, or fills in a field with a word (plain, accented, Chinese, Hebrew, a huge number). Clicks and fills pick their target by its place among what is on screen then, so a sequence means the same thing every time it is played. The generator is a seeded SplitMix64, not proptest, so the report doesn't depend on a test harness and the same plan always plays the same sequences.

The generator is guided by what it has found so far, deterministically. Steps that run a command by name prefer commands that haven't run yet in any earlier sequence. Every other sequence starts where a command not yet run was first seen available on screen: from the same named state, replaying the steps that led there, then going on at random. A command that only becomes available after another, such as Specimen's "edit-light" after "edit-dark", is reached this way rather than by luck. Without this, a command available from one state only, such as Specimen's steps for a chosen ink, was reached by luck in about five of the 96 sequences, and "every command is reachable" failed on chance rather than on the app.

After each step it checks:

- **It never crashes or fails.** A panic anywhere (`available`, `update`, the view, layout, painting) or a failure the runner reports: a command that panicked and was rolled back, one that returned `Outcome::Nothing` but changed the facts, a container replaced by assignment (below).
- **A command `available` says yes to does something.** When a click lands on a control whose command is available, the command must change the facts, ask for an effect or say something. A button that asks first (a confirm row) is not held to this when it is pressed, since pressing it only asks; its answer is.
- **No internal command is reachable.** No control on screen runs an internal command, and running one by name from each named state changes nothing.
- **Undo returns.** Right after a step that undo can take back, Assay undoes it and compares the facts with just before the step, then redoes it and compares with just after. Checking at once, rather than undoing everything at the end, keeps changes that are deliberately not undoable (a session field like the page Specimen shows) from looking like failures.

At the end of a sequence, the facts as saved must load back as they were. Across all sequences, every command in the registry must have run at least once: a command that never does is unreachable in practice, whatever the palette lists.

When a sequence breaks a law, Assay shrinks it: it drops one step at a time while the same law still breaks, repeating until no step can go, and reports the shortest sequence in words ("click the button “Clear finished”"). Findings that differ only in numbers (the same panic on another item) count once, and each law reports at most three.

## The taste laws, small

Each named state is laid out at the normal size (1920 by 1280 at scale 2) and at its smallest window. Every box must lie inside its parent and inside the window; areas are whole cells by type, so this is what "on the grid" can still get wrong. Each ink that text is drawn in must reach its contrast on the background (`Ink::needs`) in both appearances.

## The budgets

- **Frames.** Every step is followed by a frame, timed from the start of the step to the end of painting (input, view, layout, paint; not the GPU, which offscreen has none of). The slowest 1% of all frames is judged against 1 ms, and the slowest frame is named with its stages so the cause is visible. Bulk changes landed by `finish` between steps are not counted: in a window they land a slice at a time between frames.
- **`available`.** Every command on screen is asked after each step, each call timed; the slowest 1% is judged against 10 µs (proposed, spec §12.2). The kernel asks once per command on screen whenever the view is rebuilt, so this keeps a hundred commands to a tenth of a frame.
- **Allocation.** Each named state is painted until steady and then once more, counting allocations, which must be zero. Counting replaces the program's allocator, which every app would then pay for on every allocation, so it is a feature: build with `--features graticule/count-allocations` (or `assay/count-allocations`) to count. Without it the report says it didn't count.
- **Idle.** An app has no way to ask for frames: it has no timers, jobs wake the window only when they finish, and the shell's one timer wakes it only when a date written as how long ago would read differently. The shell's own law (`idle_is_zero`) therefore covers every app, and the report says so rather than measuring it again.

Times are judged only in an optimized build; a debug build shows them and says they don't count.

## Assay's own law

A check that passes on an app keeps passing when nothing it reads has changed: the same plan over the same app finds the same things, step for step (`a_check_that_reads_nothing_new_finds_the_same_things`, over random seeds, budgets aside since they read the clock).

## What it found first

Run over the to-do app, Specimen and Scope, Assay found four things that are fixed in the same change:

- The kernel rebuilt the bar by gathering every chosen key in list order, so with ten thousand items chosen every frame cost over a millisecond. The bar, which-key and the palette now take the count and one representative; every subject is gathered only when a command actually runs (`Runner::glance`).
- Replacing a whole `List`, `Text` or `Selection` by assignment (`self.blocks = blocks(component)`) recorded nothing, so undo left the new contents in place and later operations named the wrong path. See `kernel.md` §4.4.
- Removing records pruned their keys from every selection, whatever list it chose from, so equal keys in two lists (fixed keys in Specimen's pages and sections) cleared the wrong selection. Pruning now carries the removed record's type.
- Specimen's catalog and Scope replaced lists by assignment; they use `List::replace` now.

What remains slow is known: removing thousands of chosen items in one command and undoing a big change both land at once on the main thread (about 10 ms with thousands of items, in release on the Windows machine); the slowest 1% stays inside the budget. Both belong to build-plan item 13 (`kernel.md` §20).
