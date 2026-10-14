# Bindery's development loop

`crates/bindery` is a command with two verbs so far; installing and updating apps and fetching the large fonts are build-plan item 25. It uses only the standard library and `cargo`.

## `bindery new`

`bindery new FOLDER` writes an app named after the folder: `Cargo.toml` and `src/main.rs` with facts, two commands (one of them with a reason it can be unavailable), one screen and one named state. Specimen is wired in through `graticule::run`, so `--specimen`, `--render`, `--changes`, `--approve` and `--assay` work at once. It binds no keys: a new app's keymap is the maker's to write.

Inside a workspace whose `Cargo.toml` names `graticule` among its dependencies (this repository's `apps/`), the app inherits the workspace's version, edition, license, lints and Graticule; anywhere else it stands alone, depends on Graticule's public repository by git, and forbids `unsafe` itself.

## `bindery dev`

`bindery dev [FOLDER] [-p PACKAGE] [--bin NAME] [-- APP ARGUMENTS]` builds the app with `cargo build --message-format json-render-diagnostics`, takes the program's path from the build's own report, and runs it. Every quarter of a second it fingerprints the workspace (paths, sizes and modification times of every file outside the target folder and dot folders); when that changes it builds again. A build that fails prints its errors and never reaches the app. A build that changes the program replaces the running app:

1. Bindery writes `reload` to the app's standard input. The kernel's shell listens for it whenever `GRATICULE_RELOAD` names a handoff file (`kernel/src/reload.rs`).
2. The shell writes the handoff, Ledger's binary encoding of the window's outer position and inner size and whatever the face keeps (`Face::keep`), and leaves the event loop the normal way.
3. Bindery waits for the app to exit (stopping it after five seconds if it doesn't), and starts the new build with the same arguments and the same handoff file.
4. The new build reads the handoff, deletes it, opens its window at the same frame, and gives the face its part (`Face::restore`) right after making it.

What a face keeps is what the new build can't get back on its own. An app that saves gets its facts back from its own store, so the runner keeps only where the user was (focus, caret, every list's scroll anchor); one that doesn't save (`--specimen`, Scope, an app run with `Runner::new`) also keeps its facts, as a `Value`. When the facts no longer fit the new build (a field was added), the runner starts from what the app starts with and says so in the message line. Specimen keeps its catalog, which named state is showing, and each state's runner; Scope keeps its runner and reattaches to the app it was watching.

### Why Bindery restarts the app rather than the app replacing itself

Feedreader's Bindery had the app `exec` its own new image, so the process id stayed the same and Bindery's child never ended. Windows has no `exec`, and it doesn't let a build overwrite a program that is running, so the same design can't work there. Bindery therefore runs a copy (`target/<profile>/bindery/<name>`, keeping the program's name so the data folder, the proofs folder and Scope still find it), lets cargo replace the original freely, and starts the new copy itself once the old one has exited. The same steps on every system, and a reload costs one launch: the window disappears for about as long as the app takes to start.

The app ending on its own ends the session when it exits cleanly (the window was closed), and is reported and started again after the next change when it crashed.

Patching code into the running process (Subsecond) was turned down in Feedreader and still is: it needs its own build driver and can't follow a change to a type's layout, and Graticule's state is already small enough to carry across a restart.

### Laws

A reload keeps the state: `a_reload_keeps_the_state` in the kernel's laws restores what a runner kept after generated command sequences into a fresh runner and compares facts and whereabouts. A failed build never reaching the app and the window keeping its frame are checked by hand: the first with a stand-in program that hands its state over (it did, three builds in a row, and a failed build in between left the running one alone), the second needs one pass in a real window.
