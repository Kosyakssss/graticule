# Bindery

`crates/bindery` is a command: `new` and `dev` make apps and run the development loop, `install` and `update` put them on the computer and keep them current, `fonts` fetches the fonts too large for the repository, and `messages` gathers the words to translate. It uses the standard library, `dirs` for the system's folders, and the system's `cargo`, `git` and `curl`.

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

## `bindery fonts`

The Japanese, Korean and Chinese Plex fonts are pinned in the kernel's `assets/fonts/fetched.txt`, a URL at a fixed commit and a SHA-256 per file, which Bindery reads at compile time, so the kernel and Bindery can't disagree about which files are meant. Each is downloaded with `curl`, checked and then renamed into the shared `Graticule/Fonts` folder, so a half-written or changed file is never where an app looks. SHA-256 is Bindery's own, fifty lines checked against `sha256sum`, so fetching fonts adds no dependency; `--from` takes the files from a folder, for a machine that can't reach the internet.

## `bindery install` and `update`

`install` builds the app for release and puts it where this user's programs go: on macOS an `.app` in `~/Applications`, signed ad hoc with `codesign` so Gatekeeper runs it, with its command linked into `~/.local/bin`; on Linux the program in `~/.local/bin` and a desktop entry; on Windows the program in `Programs/Graticule` under local app data. A program that is running can't be replaced on Windows but can be renamed, so the old one is moved aside to `NAME.old.exe` and removed at the next install. The macOS and Linux paths are written but not yet tried on those systems.

Bindery remembers each install in `Graticule/bindery/installed.txt`: the app, its source folder, the package and program it was built with, the commit it was built from and where it went. `update` brings each source up to date with `git pull --ff-only` when it follows an upstream (never merging: a source with work of its own is left for its owner), and builds and installs again any whose commit moved; `--offline` skips the pull, for a source updated by hand. Updating is what the user asks for, never a check on its own: when Bindery may look for new versions without being asked is open question 14, since that reaches the network.

## What's new

An update writes what changed, the subject line of each commit since the last install, into `news.txt` in the app's own data folder (`Graticule/NAME`, the folder the kernel keeps the app's facts in), newest first, keeping the last ten updates, and leaves a `news.unread` marker beside it. The kernel reads both when the app opens its folder: the built-in "Show what's new" opens a page of them, like the log, and is unavailable with a reason when there is nothing; the marker makes the message line say once that the app was updated, then goes. Nothing pops up. The words are the commits' own, so they are as plain as the commits are written.
