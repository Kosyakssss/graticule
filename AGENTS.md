# Graticule

Graticule is a framework for native desktop apps in Rust, built on a grid. An app describes its facts, the commands that change them and how they look; Graticule does the rest, from the window to the pixels, faster than any display can show. It should carry anything from a notes app to a sequencer, a trading terminal or an image viewer, with app code a person can read top to bottom.

[`docs/spec.md`](docs/spec.md) is the source of truth for what to build, and its §17 is the build plan with status; this file is how to work on it. [`docs/guide/`](docs/guide/README.md) explains it all in plain words.

## What we never compromise on

### 1. The grid is law; inside a box is freedom

Every box, rule, column and component edge sits on whole cells, and the code that places them can only speak in cells. Inside a box, a canvas or a picture, anything goes. The frame belongs to Graticule; the content belongs to the app.

### 2. Faster than any display can show, silent when idle

No frame takes longer than a 1000 Hz display's refresh, however heavy the app. The main thread never does work that grows with the data. Idle costs nothing: no timers, no wakeups, no frames.

### 3. App code a person can write

If someone who is learning Rust can't follow an app's code, the API is wrong. One obvious way to do each thing. The hard parts belong to the kernel.

### 4. Facts, commands, a view

An app is its facts, the commands that change them and a view drawn from them. The view never writes back. One tree sits between the view and the pixels, and every tool reads it.

### 5. Everything visible, inner workings included

Every action available right now is on screen, generated from the registry. Why something is unavailable is said, not hidden. Specimen and Scope are part of the framework, not extras.

### 6. Proven by law

Types make whole classes of mistakes impossible to write, laws check behavior over generated inputs, and the few cores where a mistake would be worst get proofs. Every app built on Graticule is tested against the universal laws for free.

### 7. Native, open, private

Public repository, no accounts, no cloud service run by us, no app stores. Nothing leaves the user's devices unless the app sends it on purpose.

## A note on simplicity

Prefer the smallest model that makes the correct behavior unsurprising. Do not keep complexity because it exists, and do not add machinery because it looks architectural. If something falls out of the architecture (the registry, the tree, the facts), do not build it; make sure it stays possible. When the spec and the simplest design disagree, say so instead of quietly following either.

## Glossary

- **owner** means the person who decides what Graticule is. Product and taste decisions come from the spec or from them. **maintainers** are everyone who works on the code, people and agents alike. The project names roles, never people.
- **kernel** (model DK-01) means `crates/kernel`: the tree, layout, painting, input, the shell around the operating system. The model code changes with each breaking version.
- **facts** means an app's state: plain data, enums, no flags that contradict each other. Also called the model.
- **command** means an entry in the registry: name, label, arguments, availability and the change it makes to the facts. Commands are the only way facts change. **registry** means all of an app's commands. **keymap** means the data file that binds keys, sequences and modes to commands.
- **view** means the function that describes the screen from the facts. **tree** means what the view produces: components in cells, with stable ids, which the kernel lays out, paints and hit-tests.
- **effect** means work the app asks for as data: a request, a file write, a URL to open. **job** means background work, always visible while it runs. **live value** means a value written by another thread and read once per frame.
- **component** means a reusable piece of the tree: Graticule's or the app's own. **canvas** means a box with its own coordinate space for shapes drawn at runtime. **picture** means vector art compiled ahead of time.
- **style** means how things are drawn (lines, shadows, glyphs, density), never where. **theme** means colors. **ink** means a semantic color: ink, muted, faint, rule, link, danger, warn, ok and the surfaces.
- **unit** means a named part of Graticule with a model code: a library that apps build in and a face of its own that you can run (spec §13). The units are the **Kernel** (DK-01), **Ledger** (LD-01, the command log: saving, undo, history, replays, sync), **Galley** (GA-01, the text engine), **Plotter** (PL-01, the canvas engine), **Specimen** (SP-01), **Scope** (SC-01), **Assay** (AS-01, the laws, taste checks and budgets), **Engraver** (EN-01) and **Bindery** (BD-01). A new unit is warranted only when it is ours, has its own model and has its own laws.
- **service** means a battery that ships as its own crate and mostly wraps others' libraries (search, media, networking, secrets); apps plug in what they need.
- **the apps** means the demo apps in `apps/` (spec §14).

## Working together

Many people and many agent threads work on this, and none of them remember each other. The repository is the memory; a thread is a workbench.

- A decision is not real until it is written down: what and why in the spec, reasons that cross components in `docs/internals/`, progress in the build plan, and a few plain sentences in `docs/guide/` when it changes what the owner sees.
- Work comes from the build plan in small items, each with a visible finish line, marked as taken while someone is on it. Asked to build Graticule, or simply to carry on, work through the plan from the top as its first paragraph says, item after item, until it is done. Two workers never take the same item. Parallel threads work in their own checkout.
- Product and taste questions go to the owner. A thread that hits one adds it to the spec's open questions and asks; it does not decide.
- The owner reviews pictures, numbers and sentences: Specimen's Changes page, the budgets, the guide. Code is reviewed by maintainers, and by a second agent on a different model before it lands.
- Recurring jobs (a new component, moving a piece in from elsewhere, reviewing a change) follow the written recipes in `docs/recipes.md`, so the result does not depend on who did it.

## Hit every surface

The most common defect is a change that works on the path you tried and is missing everywhere else. Before calling work done, check which of these apply and say so:

- **Apps and faces.** Every demo app and every unit's face still builds and its states still render. Specimen's Changes page shows only what you meant to change. A unit's core that needs no screen still builds without the kernel.
- **Sizes and looks.** The smallest and largest window, both appearances, every style that ships.
- **Entry points.** An action belongs in the registry, so it shows in the bar, which-key, help, the palette, menus and context menus, works by mouse and keyboard, and is reachable through automation. Bulk versions work on the selection.
- **Reverse states.** A way in needs a way out and a way to see it. Every user-visible change is undoable.
- **Accessibility.** The screen reader announces it, the keyboard reaches it, and the taste laws pass.
- **Budgets.** Frame time, allocation, idle, binary size. Regressions show up as numbers; say what moved.
- **Contracts.** The public API is the wire format between Graticule and every app. An incompatible change bumps the kernel's model code and fixes every app in the same change.
- **Docs.** Check whether the change makes the spec, the guide, a component page or the README wrong.

## Taste

- Complexity belongs at the edges. Decisions are pure functions over facts; the view is a pure function of the facts.
- State is enums, not flags that can contradict each other.
- Inferred types over annotations. No `unsafe` anywhere; the workspace denies it. When a platform API needs it, use a crate that wraps it safely. The approved exceptions are the native context menu on macOS and the macOS spellchecker (if no safe binding reaches it), explained in `docs/internals/unsafe.md`; any other one needs the owner's sign-off first, with proof that no safe route exists.
- No code comments, with one exception: documentation comments (`///`, `//!`) on public items that app developers use. Names and types carry the meaning everywhere else; reasons that cross components go in `docs/internals/`.
- Do not invent keyboard shortcuts. Keys for app commands live in keymap files; Graticule assigns none, the platform's own conventions (text editing, copy and paste, Tab, Escape) belong to the system, and a demo app's defaults are chosen by maintainers for good use, which the owner can change.
- Learning from other projects' source is fine; copying it is not. Read, understand, write our own. Code under a permissive license (MIT, Apache-2.0) may be ported with its notice kept.
- If a rule here fights the task in front of you, say so plainly and get the owner's sign-off before breaking it.

## Verifying

- Tests are laws (spec §11): properties over generated inputs and command sequences (proptest), never tests that restate the code. New laws and new test files need the owner's approval.
- Use the smallest proof: `cargo build`, `cargo clippy` on the crates you touched, and the laws you touched.
- Look at the result in Specimen, at the smallest and largest sizes. Offscreen renders print paint times.
- Measure. Scope shows every stage of every frame; Tracy (`--features profile`) shows zones and allocations. Budgets are checked on the reference machine in release builds, by the slowest 1% of frames, not the average.
- User-visible changes get one pass in the real window. Ask before using computer use on the owner's screen.

## Repository rules

- The repository is public. Never commit secrets, credentials, personal data, session data or machine-local state. Never hardcode a home path; derive paths at runtime.
- Version control is Git. Never add remotes or rewrite shared history without explicit approval.
- Do not commit research notes or scratch files. The build plan lives in the spec.

## Documentation

- `docs/spec.md` holds the decisions. When a decision changes, rewrite the affected text; do not append a second account or keep the old one. Keep the build plan current.
- `docs/internals/` is for decisions and their reasons, constraints across components and traps that are hard to see from the source. If reading the code answers the question, leave it out.
- `docs/guide/` explains Graticule in plain words, for someone who doesn't write code. Every finished build-plan item that changes what the owner sees adds to it.
- `docs/components/` holds one page per component; Specimen shows it next to the live component.
- `docs/api.md` is the app code we want to be able to write. The kernel's job is to make it real.
