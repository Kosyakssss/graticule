use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Instant;

use kernel::accesskit::Action;
use kernel::keys::{Chord, Key as Pressed};
use kernel::runner::Runner;
use kernel::shell::Face;
use kernel::tree::{Kind, NONE};
use kernel::{App, Commands};

use crate::screen::Screen;
use crate::{Finding, Law, Plan};

const KEYS: &[&str] = &[
    "down",
    "up",
    "shift+down",
    "shift+up",
    "home",
    "end",
    "pageup",
    "pagedown",
    "space",
    "enter",
    "tab",
    "shift+tab",
    "escape",
    "delete",
    "backspace",
    "left",
    "right",
    "primary+a",
    "primary+z",
    "primary+shift+z",
    "primary+shift+p",
];
const BUILTINS: &[&str] = &["palette", "undo", "redo", "next-control", "previous-control", "up", "down", "extend-up", "extend-down", "first", "last", "select-all", "cancel"];
const TYPED: &[char] = &['a', 'b', 'c', 'd', 'e', ' ', 'é'];
const WORDS: &[&str] = &["milk", "Call mum", "été", "長い", "12345678901234567890", "שלום"];
const MOST_PER_LAW: usize = 3;

/// One generated step: a key pressed, a character typed, a command run by the name the palette and keymaps use (with the words typed when it asks for something, then Return), or a control on screen clicked, picked by its place among the controls then.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Press(&'static str),
    Type(char),
    Run(&'static str, Option<&'static str>),
    Click(u32),
    Fill(u32),
}

pub struct Universal {
    pub findings: Vec<Finding>,
    pub frames: Vec<(u32, String)>,
    pub available: Vec<(u32, String)>,
}

struct Dice(u64);

impl Dice {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

#[derive(Default)]
struct Measure {
    frames: Vec<(u32, String)>,
    available: Vec<(u32, String)>,
}

#[derive(Default)]
struct Played {
    broke: Option<(Law, String)>,
    done: Vec<String>,
    ran: Vec<&'static str>,
    seen: Vec<(&'static str, usize, Option<u32>)>,
}

pub fn run<A: App>(starts: &[(String, A)], plan: Plan) -> Universal {
    let entries = A::Cmd::entries();
    let names: Vec<&'static str> = entries.iter().filter(|e| !e.internal).map(|e| e.name).chain(BUILTINS.iter().copied()).collect();
    let mut dice = Dice(plan.seed);
    let mut screen = Screen::new();
    let mut measure = Measure::default();
    let mut findings: Vec<Finding> = Vec::new();
    let mut ran = BTreeSet::new();
    let mut seen: Vec<(&'static str, usize, Vec<Step>)> = Vec::new();
    for (name, facts) in starts {
        if let Some(said) = internal_reached(facts) {
            findings.push(Finding { law: Law::NothingInternalReachable, said, from: name.clone(), steps: Vec::new() });
        }
    }
    for sequence in 0..plan.sequences {
        let guided = (sequence % 2 == 1).then(|| seen.iter().find(|(name, ..)| !ran.contains(name))).flatten();
        let (start, lead) = guided.map_or((sequence % starts.len(), Vec::new()), |(_, start, lead)| (*start, lead.clone()));
        let (from, facts) = &starts[start];
        let unrun: Vec<&'static str> = names.iter().copied().filter(|n| !ran.contains(n)).collect();
        let fresh = plan.steps.saturating_sub(lead.len());
        let steps: Vec<Step> = lead
            .into_iter()
            .chain((0..fresh).map(|_| match dice.below(11) {
                0..=3 => Step::Press(KEYS[dice.below(KEYS.len())]),
                4 | 5 => Step::Type(TYPED[dice.below(TYPED.len())]),
                6 | 7 => {
                    let name = if unrun.is_empty() { names[dice.below(names.len())] } else { unrun[dice.below(unrun.len())] };
                    let asks = A::Cmd::entries().iter().any(|e| e.name == name && !e.args.is_empty());
                    Step::Run(name, asks.then(|| WORDS[dice.below(WORDS.len())]))
                }
                8 => Step::Fill(dice.next() as u32),
                _ => Step::Click(dice.next() as u32),
            }))
            .collect();
        let played = play(facts, &steps, &mut screen, Some((&mut measure, from)));
        ran.extend(played.ran);
        for (name, at, pick) in played.seen {
            if !seen.iter().any(|(n, ..)| *n == name) {
                seen.push((name, start, steps[..at].iter().copied().chain(pick.map(Step::Click)).collect()));
            }
        }
        let Some((law, said)) = played.broke else { continue };
        if findings.iter().filter(|f| f.law == law).count() >= MOST_PER_LAW || findings.iter().any(|f| f.law == law && alike(law, &f.said, &said)) {
            continue;
        }
        let mut steps = steps;
        steps.truncate(played.done.len());
        let shortest = shrink(facts, steps, law, &mut screen);
        let replayed = play(facts, &shortest, &mut screen, None);
        let (law, said) = replayed.broke.unwrap_or((law, said));
        findings.push(Finding { law, said, from: from.clone(), steps: replayed.done });
    }
    for entry in entries.iter().filter(|e| !e.internal && !ran.contains(e.name)) {
        findings.push(Finding {
            law: Law::EveryCommandReachable,
            said: format!("“{}” never ran in {} sequences: no key, name or control reached it while it was available", entry.name, plan.sequences),
            from: String::new(),
            steps: Vec::new(),
        });
    }
    Universal { findings, frames: measure.frames, available: measure.available }
}

fn internal_reached<A: App>(facts: &A) -> Option<String> {
    let mut runner = Runner::new(facts.clone());
    runner.build();
    let settled = runner.facts.clone();
    for entry in A::Cmd::entries().iter().filter(|e| e.internal) {
        let before = runner.history.len();
        runner.run_named(entry.name);
        if runner.history.len() != before || !runner.facts.same(&settled) {
            return Some(format!("running “{}” by name changed the facts, though it is internal", entry.name));
        }
    }
    None
}

fn shrink<A: App>(facts: &A, mut steps: Vec<Step>, law: Law, screen: &mut Screen) -> Vec<Step> {
    loop {
        let had = steps.len();
        let mut i = steps.len();
        while i > 0 {
            i -= 1;
            let mut fewer = steps.clone();
            fewer.remove(i);
            let played = play(facts, &fewer, screen, None);
            if played.broke.is_some_and(|(l, _)| l == law) {
                fewer.truncate(played.done.len());
                steps = fewer;
                i = i.min(steps.len());
            }
        }
        if steps.len() == had {
            return steps;
        }
    }
}

fn alike(law: Law, a: &str, b: &str) -> bool {
    let gist = |s: &str| {
        let s = if law == Law::NeverFails { s.rsplit(": ").next().unwrap_or(s) } else { s };
        s.chars().map(|c| if c.is_ascii_digit() { '#' } else { c }).collect::<String>()
    };
    gist(a) == gist(b)
}

fn panicked(panic: Box<dyn std::any::Any + Send>) -> String {
    let why = panic.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| panic.downcast_ref::<String>().cloned()).unwrap_or_default();
    format!("it panicked: {why}")
}

fn play<A: App>(start: &A, steps: &[Step], screen: &mut Screen, mut measure: Option<(&mut Measure, &String)>) -> Played {
    let mut played = Played::default();
    screen.resize(crate::screen::WIDTH, crate::screen::HEIGHT);
    let mut runner = Runner::new(start.clone());
    if let Some(failure) = runner.failure() {
        played.broke = Some((Law::NeverFails, failure.to_string()));
        return played;
    }
    if let Err(panic) = catch_unwind(AssertUnwindSafe(|| screen.settle(&mut runner))) {
        played.broke = Some((Law::NeverFails, panicked(panic)));
        return played;
    }
    for (n, &step) in steps.iter().enumerate() {
        let aimed = match step {
            Step::Fill(pick) => match catch_unwind(AssertUnwindSafe(|| aim(&mut runner, pick))) {
                Ok(aimed) => aimed,
                Err(panic) => {
                    played.done.push(describe(step));
                    played.broke = Some((Law::NeverFails, panicked(panic)));
                    return played;
                }
            },
            _ => None,
        };
        let value = runner.facts.value();
        let before = (runner.history.len(), runner.effects().len(), runner.history.depth());
        let started = Instant::now();
        let taken = catch_unwind(AssertUnwindSafe(|| take(&mut runner, step, aimed)));
        let took = started.elapsed().as_micros().min(u32::MAX as u128) as u32;
        let (done, probe) = match taken {
            Ok(taken) => taken,
            Err(panic) => {
                played.done.push(describe(step));
                played.broke = Some((Law::NeverFails, panicked(panic)));
                return played;
            }
        };
        played.done.push(done);
        if let Err(panic) = catch_unwind(AssertUnwindSafe(|| runner.finish())) {
            played.broke = Some((Law::NeverFails, panicked(panic)));
            return played;
        }
        if let Some(failure) = runner.failure() {
            played.broke = Some((Law::NeverFails, failure.to_string()));
            return played;
        }
        if runner.history.depth() == before.2 + 1
            && runner.history.len() > before.0
            && let Some(broke) = returns(&mut runner, &value, &mut played.done)
        {
            played.broke = Some(broke);
            return played;
        }
        if let Some(probe) = probe {
            let did = !runner.facts.same(&probe.facts) || runner.history.len() != probe.history || runner.effects().len() != probe.effects || runner.said().is_some();
            if !did {
                played.broke = Some((Law::AvailableMeansItRuns, format!("“{}” was available but changed nothing, asked for nothing and said nothing", probe.name)));
                return played;
            }
            played.ran.push(probe.name);
        }
        if let Step::Run(name, _) = step
            && ((runner.history.len(), runner.effects().len()) != (before.0, before.1) || runner.facts.value() != value)
            && A::Cmd::entries().iter().any(|e| e.name == name && !e.internal)
        {
            played.ran.push(name);
        }
        let painted = match catch_unwind(AssertUnwindSafe(|| screen.paint(&mut runner))) {
            Ok(micros) => micros,
            Err(panic) => {
                played.broke = Some((Law::NeverFails, panicked(panic)));
                return played;
            }
        };
        for node in &runner.tree.nodes {
            if node.cmd == NONE {
                continue;
            }
            let cmd = &runner.tree.commands[node.cmd as usize];
            if cmd.internal() {
                played.broke = Some((Law::NothingInternalReachable, format!("a {} on screen runs “{}”, which is internal", node.kind.role(), cmd.name())));
                return played;
            }
        }
        if let Some((measure, from)) = measure.as_mut() {
            measure.frames.push((
                took + painted,
                format!("from {from}, after step {}: {}", n + 1, played.done[n])
                    + &format!(" (update {took} µs, view {}, layout {}, paint {})", runner.stages().view, runner.stages().layout, runner.stages().paint),
            ));
            let mut acting = 0u32;
            for node in &runner.tree.nodes {
                let pick = node.kind.acts().then_some(acting);
                acting += u32::from(node.kind.acts());
                if node.cmd == NONE {
                    continue;
                }
                let cmd = &runner.tree.commands[node.cmd as usize];
                let asked = Instant::now();
                let available = std::hint::black_box(runner.facts.available(cmd));
                let ns = asked.elapsed().as_nanos().min(u32::MAX as u128) as u32;
                if available.is_yes() && !played.seen.iter().any(|(name, ..)| *name == cmd.name()) {
                    played.seen.push((cmd.name(), n + 1, pick));
                }
                measure.available.push((ns, format!("for “{}” from {from}", cmd.name())));
            }
        }
    }
    let value = runner.facts.value();
    if A::from_value(&value).map(|f| f.value()).as_ref() != Some(&value) {
        played.broke = Some((Law::SavedFactsLoad, "the facts as saved after these steps don't load back as they were".into()));
        return played;
    }
    played
}

fn returns<A: App>(runner: &mut Runner<A>, before: &ledger::Value, done: &mut Vec<String>) -> Option<(Law, String)> {
    if !runner.facts.undoable().is_yes() {
        return None;
    }
    let after = runner.facts.value();
    let undone = catch_unwind(AssertUnwindSafe(|| {
        runner.run_named("undo");
        runner.finish();
        let undone = runner.facts.value() == *before;
        runner.run_named("redo");
        runner.finish();
        (undone, runner.facts.value() == after)
    }));
    match undone {
        Err(panic) => Some((Law::NeverFails, panicked(panic))),
        Ok((false, _)) => {
            done.push("undo".into());
            Some((Law::UndoReturns, "undo didn't return the facts to where they were just before the step".into()))
        }
        Ok((true, false)) => {
            done.extend(["undo".into(), "redo".into()]);
            Some((Law::UndoReturns, "redo didn't return the facts to where they were just after the step".into()))
        }
        Ok((true, true)) => None,
    }
}

struct Probe<A> {
    facts: A,
    history: usize,
    effects: usize,
    name: &'static str,
}

fn describe(step: Step) -> String {
    match step {
        Step::Press(keys) => format!("press {keys}"),
        Step::Type(c) => format!("type “{c}”"),
        Step::Run(name, None) => format!("run “{name}”"),
        Step::Run(name, Some(words)) => format!("run “{name}”, answering “{words}”"),
        Step::Click(_) => "click a control".into(),
        Step::Fill(_) => "fill in a field".into(),
    }
}

fn aim<A: App>(runner: &mut Runner<A>, pick: u32) -> Option<(String, &'static str)> {
    let fields: Vec<u32> = (0..runner.tree.nodes.len() as u32).filter(|&i| matches!(runner.tree.nodes[i as usize].kind, Kind::Field { .. } | Kind::Editor { .. })).collect();
    let node = runner.tree.nodes[*fields.get(pick as usize % fields.len().max(1))? as usize];
    let word = WORDS[(pick as usize / fields.len()) % WORDS.len()];
    let what = format!("{} “{}”", if matches!(node.kind, Kind::Editor { .. }) { "editor" } else { "text field" }, runner.tree.str(node.name));
    runner.act(node.id.0, Action::Focus);
    Some((what, word))
}

/// Reaches one of a canvas's shapes the way the keyboard does, going to the canvas, moving down to the shape and pressing Return, so a command only a canvas's shapes run is reached.
fn reach<A: App>(runner: &mut Runner<A>, at: u32, pick: usize) -> (String, Option<Probe<A>>) {
    let node = runner.tree.nodes[at as usize];
    let name = runner.tree.str(node.name).to_string();
    let Some(presses) = runner.tree.canvases.iter().find(|b| b.node == at).map(|b| b.presses.clone()).filter(|p| !p.is_empty()) else {
        return (format!("go to the canvas “{name}”, with nothing on it to press"), None);
    };
    let n = pick % presses.len().min(REACHED);
    let probe = Some(runner.tree.commands[presses[n].1 as usize].clone()).filter(|cmd| runner.facts.available(cmd).is_yes()).map(|cmd| Probe {
        facts: runner.facts.clone(),
        history: runner.history.len(),
        effects: runner.effects().len(),
        name: cmd.name(),
    });
    runner.act(node.id.0, Action::Focus);
    for key in std::iter::repeat_n("down", n + 1).chain(["enter"]) {
        if let Ok(chord) = Chord::parse(key) {
            runner.press(chord, None);
        }
    }
    (format!("go to the canvas “{name}”, move down {} and press Return", n + 1), probe)
}

/// How far down a canvas's shapes Assay goes to press one.
const REACHED: usize = 8;

fn take<A: App>(runner: &mut Runner<A>, step: Step, aimed: Option<(String, &'static str)>) -> (String, Option<Probe<A>>) {
    match step {
        Step::Press(keys) => {
            if let Ok(chord) = Chord::parse(keys) {
                runner.press(chord, None);
            }
        }
        Step::Type(c) => {
            runner.press(Chord { key: Pressed::Char(c), mods: Default::default() }, Some(&c.to_string()));
        }
        Step::Run(name, answer) => {
            runner.run_named(name);
            if let Some(words) = answer {
                for c in words.chars() {
                    runner.press(Chord { key: Pressed::Char(c), mods: Default::default() }, Some(&c.to_string()));
                }
                if let Ok(enter) = Chord::parse("enter") {
                    runner.press(enter, None);
                }
            }
        }
        Step::Click(pick) => {
            let acting: Vec<u32> = (0..runner.tree.nodes.len() as u32)
                .filter(|&i| runner.tree.nodes[i as usize].kind.acts() || matches!(runner.tree.nodes[i as usize].kind, Kind::Canvas { pressable: true, .. }))
                .collect();
            if acting.is_empty() {
                return ("click, with nothing on screen to click".into(), None);
            }
            let at = acting[pick as usize % acting.len()];
            let node = runner.tree.nodes[at as usize];
            if let Kind::Canvas { .. } = node.kind {
                return reach(runner, at, pick as usize / acting.len());
            }
            let label = [runner.tree.str(node.name), runner.tree.str(node.text)].into_iter().find(|s| !s.is_empty()).unwrap_or_default().to_string();
            let what = match node.kind {
                Kind::ListRow { index, .. } if label.is_empty() => format!("list item {}", index + 1),
                kind => format!("{} “{label}”", kind.role()),
            };
            if matches!(node.kind, Kind::Field { .. } | Kind::Editor { .. }) {
                runner.act(node.id.0, Action::Focus);
                return (format!("go to the {what}"), None);
            }
            let probe = (node.enabled && node.kind != (Kind::Confirm { open: false }) && node.cmd != NONE && node.builtin.is_none() && node.host == NONE && node.many == 0)
                .then(|| runner.tree.commands[node.cmd as usize].clone())
                .filter(|cmd| runner.facts.available(cmd).is_yes())
                .map(|cmd| Probe { facts: runner.facts.clone(), history: runner.history.len(), effects: runner.effects().len(), name: cmd.name() });
            runner.act(node.id.0, Action::Click);
            return (format!("click the {what}"), probe);
        }
        Step::Fill(_) => {
            let Some((label, word)) = aimed else { return ("fill in a field, with none on screen".into(), None) };
            for c in word.chars() {
                runner.press(Chord { key: Pressed::Char(c), mods: Default::default() }, Some(&c.to_string()));
            }
            return (format!("type “{word}” into the {label}"), None);
        }
    }
    (describe(step), None)
}
