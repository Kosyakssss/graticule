//! Kani proofs (spec §11) over the kernel's grid arithmetic. The source is the kernel's own file, included by path, so the proofs build without the kernel's GPU, window and font dependencies, which Kani can't compile. Run with `cargo kani -p proofs` where Kani is installed (`docs/internals/kani.md`).

#[path = "../../kernel/src/grid.rs"]
pub mod grid;

#[cfg(kani)]
mod area;
