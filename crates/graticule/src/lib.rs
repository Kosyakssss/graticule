//! The one crate a Graticule app imports: `use graticule::prelude::*;` and `graticule::run::<App>(t!("Name"))`.

pub use kernel;
pub use ledger;

/// Everything an app's code uses, in one import.
pub mod prelude {
    pub use graticule_macros::{Commands, Facts, t};
    pub use kernel::Available::{self, No, Yes};
    pub use kernel::{App, Cx, Failure, Fill, Ink, Message, Moment, Path, States, Ui, cols, no_if};
    pub use ledger::{Edit, Key, List, Outcome, Selection, Text};
}

/// Opens the app's window with the facts it kept last time (fresh ones on the first launch) and runs it until the window closes, saving every change. With `--specimen`, it opens Specimen over the app instead: every named state, live on a copy, with nothing saved. With `--render <folder>`, it renders every named state in both appearances into that folder as pictures, and prints how long each took to paint. With `--changes`, it prints what renders differently from the proofs last approved, in words; `--approve` approves them all as they are.
pub fn run<A: kernel::App + Default>(title: kernel::Message) {
    let mut args = std::env::args().skip(1);
    let ran = match args.next().as_deref() {
        Some("--render") => specimen::render::<A>(&std::path::PathBuf::from(args.next().unwrap_or_else(|| ".".into()))),
        Some("--changes") => specimen::changes::<A>(),
        Some("--approve") => specimen::approve::<A>(),
        Some("--specimen") => {
            let config = kernel::shell::Config::new(format!("{} — Specimen", title.as_str()));
            kernel::shell::run(config, |waker| specimen::Specimen::over(A::default()).wake_with(waker))
        }
        _ => {
            let config = kernel::shell::Config::new(title.as_str());
            kernel::shell::run(config, |waker| kernel::runner::Runner::open(&kernel::runner::folder(), A::default).wake_with(waker))
        }
    };
    if let Err(e) = ran {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
