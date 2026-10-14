# Recipes

The recurring jobs, written down so the result does not depend on who does them. Each ends with the "Hit every surface" list in `AGENTS.md`; a recipe says where that list bites for its job.

## A new component

1. **Find it in the inventory.** `docs/components/README.md` says which wave it belongs to and whether it is grounded or anchored. Read what React Aria says it does: its states, its keys, its focus and what a screen reader hears. Take the behavior, never the look.
2. **Decide whether it is a kind or a composition.** If it can be made from what exists (a row of a picture, a heading and a value), it gets no code: its page shows the composition, and the inventory says so. A new kind is warranted when it draws, lays out or acts differently from every kind there is.
3. **The kind.** Add it to `Kind` in `crates/kernel/src/tree.rs`, with its role name for proofs, and say whether it acts or contains others. Keep its state in the kind's own fields, as enums, never as flags that can contradict each other.
4. **The call.** Add the method to `Ui` in `crates/kernel/src/ui.rs`, with a `///` comment that says what it is and shows the call. One obvious way: no second method that does the same. A component that acts takes a command (`impl Bind`); one that changes a fact takes a `FactPath` and sets it with an `Edit`, as `form` does, so the change is a command, an undo step and in the log.
5. **Layout.** In `crates/kernel/src/layout.rs`, give its fewest and wished-for columns (`size`) and its rows at a width (`rows`); a container also places its children (`place`). Everything is whole cells.
6. **Painting.** In `crates/kernel/src/components.rs`, draw it into the under layer (fills that its children's text lies on) or the over layer (everything else). Inside its box anything goes; the box sits on the grid. Use the theme's inks and the style's lines and shadow, never a fixed color or width.
7. **The rest of the kernel.** Its accessibility role and state in `access.rs`; its state in words in `proof.rs`; if it acts, how the runner fires it, focuses it and what keys it answers in `runner.rs`; if it has a command, how `resolve` labels it.
8. **Its page.** Write `docs/components/<name>.md`: what it is in a sentence or two, "Use it for", "Code" (the first code block is shown live), "Keys" if it has any, "Narrow windows" if it changes shape, and "Screen reader". Plain words, no jargon.
9. **Specimen.** Add it to `Component` in `crates/specimen/src/catalog.rs`: to `ALL`, its name, its page and its live example; if it shows text the app gives it, to `awkward`, with how it shows each awkward sample.
10. **Look at it.** `cargo run -p specimen -- --render <folder outside the repository>` and open the pictures: both appearances, the smallest and the largest size. Then `--changes`: only the new sheets and what you meant to change may differ. Approve with `--approve` once they are right.
11. **Check.** `cargo clippy --workspace --all-targets`, `cargo test --workspace`, and `--assay` for Specimen and the to-do app.
12. **Record it.** Move its row to "Done" in the inventory, with a note if it differs from what the row said; mark the part of build-plan item 15 it belongs to; add a sentence to the guide when a wave's part is done.

## Moving a piece in from elsewhere

Feedreader's source, and other code under a permissive license, can be ported; anything else is read, understood and written again.

1. **Read it whole** before taking any of it, and write down in a sentence what it does and what it assumes.
2. **Check the license.** MIT or Apache-2.0 code may be ported with its notice kept, in the file and in `docs/internals/` where the decision is recorded. Anything else is only learned from.
3. **Fit it to Graticule, not the other way.** Rename to the glossary's words; turn flags into enums; drop the comments that aren't `///` or `//!`; replace `unsafe` with a safe crate or leave the piece out.
4. **Say what changed and why** in `docs/internals/`, where it crosses components or would surprise the next reader: what was kept, what was rewritten, and what the original did that ours doesn't.
5. **Prove it the same way as code written here**: the laws of the unit it joins, the budgets, and Specimen's Changes page.

## Reviewing a change

Code is reviewed by a maintainer, or by an agent on a different model from the one that wrote it.

1. **Read the build-plan item and its finish line** first, then the change against them: does it do what the item says, all of it, and nothing it doesn't?
2. **Walk "Hit every surface"** in `AGENTS.md` and name each surface the change touches: apps and faces, sizes and looks, entry points, reverse states, accessibility, budgets, contracts, docs.
3. **Read for the rules**: no `unsafe`, no comments but `///` and `//!`, no invented keys, no flags that contradict, nothing that grows with the data on the main thread, nothing that wakes when idle.
4. **Check the pictures and numbers**, not only the code: Specimen's Changes page, `--assay`, and what moved in the budgets.
5. **Write findings as failures**: what input or state goes wrong and what happens, ranked worst first. A matter of taste goes to the owner as a question, not as a finding.
