# Kani

Spec §11 asks for Kani proofs of the few cores where a mistake would be worst: grid arithmetic, field edit operations and Ledger's merging. What exists, why it lives where it does, and what Kani couldn't do.

## Where they run

Kani runs only on Linux and macOS, so on Windows it runs inside WSL: `cargo kani -p proofs`, with `CARGO_TARGET_DIR` on the Linux side because building on `/mnt/c` is slow. Kani compiles every dependency of the crate it checks with its own compiler, and the kernel's dependencies (wgpu, winit, fontconfig's build script) don't get through it. So the proofs live in their own crate, `crates/proofs`, which includes the kernel's `grid.rs` by path instead of depending on the kernel: the proofs check the same source the kernel builds, with nothing else attached. A file included this way must not use anything outside itself; `grid.rs` uses nothing. On a normal build the crate is just the grid module, so it costs nothing and the harnesses (behind `cfg(kani)`) are never compiled.

Run one harness at a time on a machine with less than 32 GB (`--harness <name>`), and cap memory (`ulimit -v`): CBMC's solver can take all the memory WSL has, and a WSL that runs out of memory stops answering until `wsl --shutdown`.

## Grid arithmetic

`Area` is the arithmetic every box's place goes through. The proofs, over every area within a million cells of the origin and up to a million cells wide or tall:

- Taking rows or columns from any side, by any amount (negative and too large included), splits the area into two parts that lie inside it, don't overlap and together cover it exactly.
- `columns` with three weights and a gap gives columns inside the area, in order, each starting a gap after the last, that use all of the room between the gaps. This one is bounded more tightly (areas up to 512 columns, weights up to 16), because a multiplication followed by a division of two unknowns is hard for the solver: at 4096 columns and weights up to 64 it ran for over ten minutes without an answer. 512 columns is wider than any window.
- A cell inside an area that lies inside another is inside that other.

Kani also checks every arithmetic step for overflow, so these hold without wrapping anywhere in the bounds. With Kani 0.68 and CBMC 6.11 they take 0.2 s, 1.4 s and about 70 s.

## Field edits

The text splice (`Text::apply` with `Op::Splice`, and its inverse) was tried and doesn't finish: anything that goes through `String`'s allocation and `replace_range` runs CBMC's solver out of memory, even with texts of three characters picked from a fixed table and the `cadical` solver. Field edits stay checked by the laws over generated edits (field edits undo to the start and redo to the end, in `crates/kernel/src/laws.rs`), which run the real code at any length. A proof would need the splice rewritten over a fixed-size buffer, which would prove the rewrite rather than the code.

## Merging

Waits on how sync merges (spec §15, question 11). If the answer is a total order of stamped entries, the property worth proving is that two devices with the same entries in any arrival order hold the same facts; that is a law over generated sequences first.
