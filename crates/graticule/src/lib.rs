//! The one crate a Graticule app imports: `use graticule::prelude::*;` and `graticule::run::<App>(t!("Name"))`.

pub use kernel;
/// The types Engraver's output names, so a build script can compile an app's own pictures with the prefix `graticule`.
pub use kernel::{Ink, Layer, Picture};
pub use ledger;

mod face;

/// Everything an app's code uses, in one import.
pub mod prelude {
    pub use graticule_macros::{Choice, Commands, Facts, Form, t};
    pub use kernel::Available::{self, No, Yes};
    pub use kernel::language::Translations;
    pub use kernel::{App, Cx, Derived, Failure, Feed, Fill, Ink, Jobs, Live, Message, Moment, Path, Picture, States, Ui, cols, icons, no_if};
    pub use ledger::{Edit, Key, List, Outcome, Selection, Text};
}

/// Opens the app's window with the facts it kept last time (fresh ones on the first launch) and runs it until the window closes, saving every change. With `--specimen`, it opens Specimen over the app instead: every named state, live on a copy, with nothing saved. With `--render <folder> [--references <folder>]`, it renders every named state in both appearances into that folder as pictures, with a page for review (`index.html`) that pins any reference images named after a state beside it, and prints how long each took to paint. With `--changes`, it prints what renders differently from the proofs last approved, in words; `--approve` approves them all as they are. With `--assay`, it runs Assay over the app and prints what it found. With `--ledger`, it shows the facts the app keeps: their history, the facts after any change, whether the log replays to them, how they differ from another copy, or the log as text.
pub fn run<A: kernel::App + Default>(title: kernel::Message) {
    let ran = specimen::offscreen::<A>(std::env::args().skip(1)).unwrap_or_else(|| match std::env::args().nth(1).as_deref() {
        Some("--ledger") => face::ledger::<A>(&std::env::args().skip(2).collect::<Vec<_>>()),
        Some("--specimen") => {
            let config = kernel::shell::Config::new(format!("{} — Specimen", title.as_str()));
            kernel::shell::run(config, |waker| specimen::Specimen::over(A::default()).wake_with(waker))
        }
        _ => {
            let args: Vec<String> = std::env::args().collect();
            let asked = args.iter().position(|a| a == "--language").and_then(|i| args.get(i + 1));
            let (_, troubles) = match asked {
                Some(tag) => kernel::language::choose(A::translations(), tag),
                None => kernel::language::choose_system(A::translations()),
            };
            for trouble in troubles {
                eprintln!("{trouble}");
            }
            let config = kernel::shell::Config::new(title.as_str());
            kernel::shell::run(config, |waker| kernel::runner::Runner::open(&kernel::runner::folder(), A::default).wake_with(waker))
        }
    });
    if let Err(e) = ran {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
