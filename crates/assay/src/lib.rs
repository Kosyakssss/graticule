//! AS-01, Assay: the checks every Graticule app is held to. The universal laws drive an app through generated command sequences; the taste laws look at every named state; the budgets time every frame and every `available`. [`report`] runs them all and prints what it found, like Clippy for Graticule apps; [`assay`] returns the same as data, for an app's own laws.

mod screen;
mod taste;
mod universal;

#[cfg(test)]
mod laws;

use std::fmt::Write as _;

pub use universal::Step;

/// The longest a frame may take, in microseconds: a 1000 Hz display's refresh.
pub const FRAME_BUDGET: u32 = 1000;

/// The longest one call of `available` may take, in nanoseconds. The kernel asks it for every command on screen when the view is built, so it has to be a small part of a frame.
pub const AVAILABLE_BUDGET: u32 = 10_000;

/// The ceiling on a program's size on disk, in bytes, proposed in spec §12.3 and not judged until the owner sets it.
pub const SIZE_PROPOSED: u64 = 10 * 1024 * 1024;

/// Which law a finding breaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Law {
    NeverFails,
    UndoReturns,
    AvailableMeansItRuns,
    NothingInternalReachable,
    EveryCommandReachable,
    SavedFactsLoad,
    OnTheGrid,
    Reachable,
    Named,
    Contrast,
    ColorBlind,
    FrameBudget,
    AvailableBudget,
    NoAllocationPerFrame,
}

impl Law {
    pub fn name(self) -> &'static str {
        match self {
            Law::NeverFails => "it never crashes or fails",
            Law::UndoReturns => "undo returns",
            Law::AvailableMeansItRuns => "a command available says yes to does something",
            Law::NothingInternalReachable => "no internal command is reachable",
            Law::EveryCommandReachable => "every command is reachable",
            Law::SavedFactsLoad => "saved facts load",
            Law::OnTheGrid => "every box lies on the grid, inside its parent and the window",
            Law::Reachable => "every control can be reached by the pointer in at least one cell",
            Law::Named => "no two items in one list read the same",
            Law::Contrast => "text inks reach their contrast",
            Law::ColorBlind => "text inks reach their contrast with each kind of color blindness",
            Law::FrameBudget => "every frame stays within budget",
            Law::AvailableBudget => "available stays within its budget",
            Law::NoAllocationPerFrame => "a steady frame allocates nothing",
        }
    }
}

/// One thing Assay found: the law, what happened, and how to get there from a named state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub law: Law,
    pub said: String,
    pub from: String,
    pub steps: Vec<String>,
}

/// How hard to look: how many command sequences, how long each, and the seed they are generated from. The same plan finds the same things.
#[derive(Clone, Copy, Debug)]
pub struct Plan {
    pub sequences: usize,
    pub steps: usize,
    pub seed: u64,
}

impl Default for Plan {
    fn default() -> Self {
        Plan { sequences: 96, steps: 40, seed: 0x6772_6174_6963_756c }
    }
}

/// What a run of Assay found and measured.
#[derive(Debug, Default)]
pub struct Assayed {
    pub findings: Vec<Finding>,
    pub frames: Measured,
    pub available: Measured,
    pub states: usize,
    pub sequences: usize,
    pub steps: usize,
    pub allocations: Option<u64>,
    /// How many bytes the running program takes on disk, when it could be read.
    pub size: Option<u64>,
}

/// Times measured, in microseconds for frames and nanoseconds for `available`: how many, the slowest 1%, the slowest and where it happened.
#[derive(Clone, Debug, Default)]
pub struct Measured {
    pub count: usize,
    pub slowest_percent: u32,
    pub slowest: u32,
    pub at: String,
}

impl Measured {
    fn of(mut times: Vec<(u32, String)>) -> Measured {
        if times.is_empty() {
            return Measured::default();
        }
        times.sort_by_key(|(t, _)| *t);
        let count = times.len();
        let percent = times[(count * 99 / 100).min(count - 1)].0;
        let (slowest, at) = times.pop().unwrap_or_default();
        Measured { count, slowest_percent: percent, slowest, at }
    }
}

/// Whether times count against the budgets: only in an optimized build.
pub fn judged() -> bool {
    !cfg!(debug_assertions)
}

/// Runs every check over an app with this plan and returns what it found and measured.
pub fn assay<A: kernel::App>(plan: Plan) -> Assayed {
    let starts: Vec<(String, A)> = A::states().list.into_iter().map(|(name, facts)| (format!("“{name}”"), facts)).collect();
    if starts.is_empty() {
        let said = "the app names no states in `App::states`, so there was nothing to start from".to_string();
        return Assayed { findings: vec![Finding { law: Law::NeverFails, said, from: String::new(), steps: Vec::new() }], ..Assayed::default() };
    }
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let mut universal = universal::run(&starts, plan);
    std::panic::set_hook(hook);
    let mut findings = std::mem::take(&mut universal.findings);
    let taste = taste::run(&starts);
    findings.extend(taste.findings);
    let frames = Measured::of(universal.frames);
    if judged() && frames.slowest_percent > FRAME_BUDGET {
        findings.push(Finding {
            law: Law::FrameBudget,
            said: format!("the slowest 1% of frames took {} µs, over the budget of {FRAME_BUDGET} µs; the slowest, {} µs, was {}", frames.slowest_percent, frames.slowest, frames.at),
            from: String::new(),
            steps: Vec::new(),
        });
    }
    let available = Measured::of(universal.available);
    if judged() && available.slowest_percent > AVAILABLE_BUDGET {
        findings.push(Finding {
            law: Law::AvailableBudget,
            said: format!("the slowest 1% of calls took {} ns, over the budget of {AVAILABLE_BUDGET} ns; the slowest, {} ns, was {}", available.slowest_percent, available.slowest, available.at),
            from: String::new(),
            steps: Vec::new(),
        });
    }
    findings.extend(taste.allocations.iter().filter(|(_, count)| *count > 0).map(|(from, count)| Finding {
        law: Law::NoAllocationPerFrame,
        said: format!("a frame with nothing changed allocated {count} times"),
        from: from.clone(),
        steps: Vec::new(),
    }));
    findings.sort_by(|a, b| a.law.cmp(&b.law).then_with(|| a.from.cmp(&b.from)));
    let allocations = cfg!(feature = "count-allocations").then(|| taste.allocations.iter().map(|(_, n)| n).sum());
    let size = std::env::current_exe().and_then(std::fs::metadata).ok().map(|m| m.len());
    Assayed { findings, frames, available, states: starts.len(), sequences: plan.sequences, steps: plan.steps, allocations, size }
}

/// Runs every check over an app and prints a report. Fails when anything was found, so a check can stop on it.
pub fn report<A: kernel::App>() -> Result<(), String> {
    let assayed = assay::<A>(Plan::default());
    print!("{}", written(&assayed));
    match assayed.findings.len() {
        0 => Ok(()),
        1 => Err("Assay found one thing".into()),
        n => Err(format!("Assay found {n} things")),
    }
}

/// The report, as `report` prints it.
pub fn written(a: &Assayed) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Assay drove the app through {} command sequences of {} steps, from {} starting states, and looked at each state at its normal and smallest size.",
        a.sequences, a.steps, a.states
    );
    for f in &a.findings {
        let _ = writeln!(out, "\nfound: {}", f.law.name());
        let _ = writeln!(out, "  {}", f.said);
        if !f.from.is_empty() {
            let _ = writeln!(out, "  from {}", f.from);
        }
        for (n, step) in f.steps.iter().enumerate() {
            let _ = writeln!(out, "  {:>3}. {step}", n + 1);
        }
    }
    let _ = writeln!(out);
    let judging = if judged() { "" } else { " (a debug build: times are shown but not judged; build with --release)" };
    let _ =
        writeln!(out, "Frames{judging}: {} drawn, the slowest 1% in {} µs against {FRAME_BUDGET}, the slowest in {} µs, {}.", a.frames.count, a.frames.slowest_percent, a.frames.slowest, a.frames.at);
    let _ = writeln!(
        out,
        "Available: {} calls, the slowest 1% in {} ns against {AVAILABLE_BUDGET}, the slowest in {} ns, {}.",
        a.available.count, a.available.slowest_percent, a.available.slowest, a.available.at
    );
    let _ = match a.allocations {
        Some(n) => writeln!(out, "Allocation: {n} in steady frames of every state."),
        None => writeln!(out, "Allocation: not counted in this build; build with the feature count-allocations to count it."),
    };
    let _ = writeln!(out, "Idle: an app can't ask for frames itself, so the shell's law that idle is zero holds for it.");
    if let Some(size) = a.size {
        let mb = |b: u64| b as f64 / 1_048_576.0;
        let fonts = kernel::fonts::bundled() as u64;
        let build = if judged() { "" } else { " in this debug build" };
        let _ = writeln!(
            out,
            "Size: the program takes {:.1} MB on disk{build}, {:.1} MB of it the bundled fonts, against a ceiling of {:.0} MB that is proposed and not yet judged.",
            mb(size),
            mb(fonts),
            mb(SIZE_PROPOSED)
        );
    }
    let _ = writeln!(out, "Energy: not measured; it needs the reference machine.");
    let _ = match a.findings.len() {
        0 => writeln!(out, "Every law holds."),
        1 => writeln!(out, "One finding."),
        n => writeln!(out, "{n} findings."),
    };
    out
}
