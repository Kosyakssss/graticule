//! SP-01, Specimen: every component and every screen of every app, in every state.

mod catalog;
mod changes;
mod page;
mod rings;
mod style;

use std::sync::mpsc;

pub use catalog::{Catalog, Place};
pub use changes::Changes;
use kernel::Available::{No, Yes};
use kernel::accesskit;
use kernel::ink::Appearance;
use kernel::kept::Kept;
use kernel::metrics::Metrics;
use kernel::paint::Painter;
use kernel::runner::{Extra, Runner};
use kernel::shell::{Face, Frame, Input, Seen, Waker};
use kernel::{App, Message};

/// Specimen: its own places, run by its own app, and an app's named screens, each live on its own copy of the facts, so clicks and keys run the app's real code without touching what it saved. Effects are shown, not run. The places, and on Screens the named states, are commands in the bar wherever you are.
pub struct Specimen<A: App> {
    catalog: Runner<Catalog>,
    names: Vec<&'static str>,
    screens: Vec<Runner<A>>,
    state: usize,
    changes: Runner<Changes>,
    looked: Looked,
    counted: Counted,
    waker: Option<Waker>,
}

/// How far Specimen has got counting which screens draw in which ink, for the Style place. It counts the first time Style opens, on a thread of its own.
enum Counted {
    NotYet,
    Counting(mpsc::Receiver<style::Reaches>),
    Done,
}

/// How far Specimen has got comparing every sheet with its approved proof. It looks the first time Changes opens, on a thread of its own.
enum Looked {
    NotYet,
    Looking(mpsc::Receiver<Vec<changes::Sheet>>),
    Done,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Showing {
    Catalog,
    Screen,
    Changes,
}

impl Specimen<Catalog> {
    /// Specimen on its own: Graticule's components, with no app's screens.
    pub fn alone() -> Self {
        Specimen::with(Vec::new(), Place::Components)
    }
}

impl<A: App> Specimen<A> {
    /// Specimen over an app: its named states, or `fallback` alone when it names none, opened on Screens.
    pub fn over(fallback: A) -> Self {
        let mut states = A::states().list;
        if states.is_empty() {
            states.push(("Default", fallback));
        }
        Specimen::with(states, Place::Screens)
    }

    fn with(states: Vec<(&'static str, A)>, place: Place) -> Self {
        let names = states.iter().map(|(name, _)| *name).collect();
        let screens = states
            .into_iter()
            .map(|(_, facts)| {
                let mut runner = Runner::new(facts);
                runner.hold_effects();
                runner
            })
            .collect();
        let mut catalog = Runner::new(Catalog::default());
        catalog.run(catalog::Cmd::Go(place));
        let mut specimen = Specimen { catalog, names, screens, state: 0, changes: Runner::new(Changes::looking()), looked: Looked::NotYet, counted: Counted::NotYet, waker: None };
        specimen.offer();
        specimen
    }

    /// Lets every runner's jobs, and Specimen's own search for changes, wake the window.
    pub fn wake_with(mut self, waker: Waker) -> Self {
        self.catalog = self.catalog.wake_with(waker.clone());
        self.screens = self.screens.into_iter().map(|r| r.wake_with(waker.clone())).collect();
        self.changes = self.changes.wake_with(waker.clone());
        self.waker = Some(waker);
        self
    }

    fn place(&self) -> Place {
        self.catalog.facts.place
    }

    fn showing(&self) -> Showing {
        match self.place() {
            Place::Screens if !self.screens.is_empty() => Showing::Screen,
            Place::Changes => Showing::Changes,
            _ => Showing::Catalog,
        }
    }

    fn offer(&mut self) {
        let here = self.place();
        let places: Vec<Extra> = Place::ALL
            .iter()
            .map(|&place| Extra {
                name: place.name().to_string(),
                label: place.label().to_string(),
                available: if place == here {
                    No(Message::from("You are here"))
                } else if place == Place::Screens && self.screens.is_empty() {
                    No(Message::from("Specimen was opened on its own; run an app with --specimen to see its screens"))
                } else {
                    Yes
                },
            })
            .collect();
        let states = self.names.iter().enumerate().map(|(i, name)| Extra {
            name: format!("show-{}", name.to_lowercase().replace(' ', "-")),
            label: format!("Show “{name}”"),
            available: if i == self.state { No(Message::from("It is showing")) } else { Yes },
        });
        let with_states: Vec<Extra> = places.iter().cloned().chain(states).collect();
        self.catalog.host(places.clone());
        self.changes.host(places);
        for runner in &mut self.screens {
            runner.host(with_states.clone());
        }
    }

    fn face(&mut self) -> &mut dyn Face {
        match self.showing() {
            Showing::Screen => &mut self.screens[self.state],
            Showing::Changes => &mut self.changes,
            Showing::Catalog => &mut self.catalog,
        }
    }

    fn survey(&mut self) {
        if self.place() == Place::Style && matches!(self.counted, Counted::NotYet) {
            let (send, receive) = mpsc::channel();
            let waker = self.waker.clone();
            let started = std::thread::Builder::new().name("specimen inks".into()).spawn(move || {
                let _ = send.send(style::reach::<A>());
                if let Some(waker) = waker {
                    waker.wake();
                }
            });
            if started.is_ok() {
                self.counted = Counted::Counting(receive);
            }
        }
        if self.place() != Place::Changes || !matches!(self.looked, Looked::NotYet) {
            return;
        }
        let (send, receive) = mpsc::channel();
        let waker = self.waker.clone();
        let started = std::thread::Builder::new().name("specimen changes".into()).spawn(move || {
            let _ = send.send(changes::survey::<A>(&changes::folder()));
            if let Some(waker) = waker {
                waker.wake();
            }
        });
        if started.is_ok() {
            self.looked = Looked::Looking(receive);
        }
    }

    fn collect(&mut self) -> bool {
        if let Counted::Counting(receive) = &self.counted
            && let Ok(found) = receive.try_recv()
        {
            self.counted = Counted::Done;
            self.catalog.run(catalog::Cmd::Reached(found));
            return true;
        }
        let Looked::Looking(receive) = &self.looked else { return false };
        let Ok(sheets) = receive.try_recv() else { return false };
        self.looked = Looked::Done;
        let mut runner = Runner::new(Changes::found(sheets));
        if let Some(waker) = &self.waker {
            runner = runner.wake_with(waker.clone());
        }
        self.changes = runner;
        self.offer();
        if let Some(kept) = self.face().kept() {
            kept.reset();
        }
        true
    }

    fn switch(&mut self) -> bool {
        let asked = match self.showing() {
            Showing::Screen => self.screens[self.state].asked(),
            Showing::Changes => self.changes.asked(),
            Showing::Catalog => self.catalog.asked(),
        };
        let Some(asked) = asked else { return false };
        match Place::ALL.get(asked) {
            Some(&place) => {
                self.catalog.run(catalog::Cmd::Go(place));
            }
            None => self.state = asked - Place::ALL.len(),
        }
        self.offer();
        self.survey();
        if let Some(kept) = self.face().kept() {
            kept.reset();
        }
        true
    }
}

impl<A: App> Face for Specimen<A> {
    fn look(&self, look: kernel::look::Look) -> kernel::look::Look {
        if self.catalog.facts.edited() { self.catalog.facts.edited_look(look.appearance) } else { look }
    }

    fn paint(&mut self, painter: &mut Painter, seen: Seen) {
        self.face().paint(painter, seen);
    }

    fn input(&mut self, input: Input, frame: &mut Frame) -> bool {
        let collected = matches!(input, Input::Woken) && self.collect();
        let redraw = self.face().input(input, frame);
        self.switch() || redraw || collected
    }

    fn painted(&mut self, frame: &mut Frame) {
        self.face().painted(frame);
    }

    fn minimum(&self) -> Option<(i32, i32)> {
        match self.showing() {
            Showing::Screen => self.screens[self.state].minimum(),
            Showing::Changes => self.changes.minimum(),
            Showing::Catalog => self.catalog.minimum(),
        }
    }

    fn menu(&mut self, frame: &mut Frame) -> Vec<kernel::menu::MenuItem> {
        self.face().menu(frame)
    }

    fn choose(&mut self, id: u64) -> bool {
        let chosen = self.face().choose(id);
        self.switch() || chosen
    }

    fn changes(&self) -> Option<kernel::Moment> {
        match self.showing() {
            Showing::Screen => self.screens[self.state].changes(),
            Showing::Changes => self.changes.changes(),
            Showing::Catalog => self.catalog.changes(),
        }
    }

    fn kept(&mut self) -> Option<&mut Kept> {
        self.face().kept()
    }

    fn switch(&mut self) -> Option<kernel::look::Switch> {
        self.face().switch()
    }

    fn offered(&mut self, looks: &kernel::look::Looks) {
        self.catalog.offered(looks);
        self.changes.offered(looks);
        for screen in &mut self.screens {
            screen.offered(looks);
        }
    }

    fn quit(&self) -> bool {
        match self.showing() {
            Showing::Screen => self.screens[self.state].quit(),
            Showing::Changes => self.changes.quit(),
            Showing::Catalog => self.catalog.quit(),
        }
    }

    fn access(&mut self, metrics: Metrics, title: &str) -> Option<accesskit::TreeUpdate> {
        self.face().access(metrics, title)
    }

    fn act(&mut self, node: u64, action: accesskit::Action) -> bool {
        let acted = self.face().act(node, action);
        self.switch() || acted
    }

    fn stages(&self) -> kernel::scope::Stages {
        match self.showing() {
            Showing::Screen => self.screens[self.state].stages(),
            Showing::Changes => self.changes.stages(),
            Showing::Catalog => self.catalog.stages(),
        }
    }

    fn keep(&self) -> ledger::Value {
        let screens = self.screens.iter().map(Face::keep).collect();
        ledger::Value::Map(vec![("catalog".into(), self.catalog.keep()), ("state".into(), ledger::Value::UInt(self.state as u64)), ("screens".into(), ledger::Value::List(screens))])
    }

    fn restore(&mut self, kept: &ledger::Value) {
        if let Some(catalog) = kept.field("catalog") {
            self.catalog.restore(catalog);
        }
        if let Some(ledger::Value::UInt(state)) = kept.field("state") {
            self.state = (*state as usize).min(self.screens.len().saturating_sub(1));
        }
        if let Some(ledger::Value::List(screens)) = kept.field("screens") {
            for (runner, screen) in self.screens.iter_mut().zip(screens) {
                runner.restore(screen);
            }
        }
        self.offer();
    }
}

/// Renders every named state of an app in both appearances, at its normal size and at the smallest window it allows, into pictures in `folder`, then the Changes page for them, and prints how long each took to paint. It also writes `index.html` there, a page with every picture and what changed, with any images from `references` pinned beside the state they are named after.
pub fn render<A: App>(folder: &std::path::Path, references: Option<&std::path::Path>) -> Result<(), String> {
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let mut screen = kernel::offscreen::Offscreen::new(1920, 1280, 2.0);
    for (name, facts) in A::states().list.iter() {
        for (look, appearance) in [("light", Appearance::Light), ("dark", Appearance::Dark)] {
            screen.resize(1920, 1280);
            let mut runner = Runner::new(facts.clone());
            let picture = screen.render_face(kernel::look::Look::of(appearance), &mut runner);
            picture.save_png(&folder.join(format!("{name}, default, {look}.png")))?;
            println!("{name}, default, {look}: painted in {:.2} ms", picture.paint_ms);
            let Some((cols, rows)) = runner.minimum() else { continue };
            let m = screen.metrics;
            screen.resize((cols as f32 * m.cell_w) as u32, (rows as f32 * m.cell_h) as u32);
            let mut runner = Runner::new(facts.clone());
            let picture = screen.render_face(kernel::look::Look::of(appearance), &mut runner);
            picture.save_png(&folder.join(format!("{name}, smallest, {look}.png")))?;
            println!("{name}, smallest ({cols} by {rows} cells), {look}: painted in {:.2} ms", picture.paint_ms);
        }
    }
    screen.resize(1920, 1280);
    let sheets = changes::survey::<A>(&changes::folder());
    let mut runner = Runner::new(Changes::found(sheets.clone()));
    let picture = screen.render_face(kernel::look::Look::of(Appearance::Light), &mut runner);
    picture.save_png(&folder.join("Changes.png"))?;
    println!("Changes: painted in {:.2} ms", picture.paint_ms);
    let app = std::env::current_exe().ok().and_then(|exe| exe.file_stem().map(|s| s.to_string_lossy().into_owned())).unwrap_or_default();
    let names: Vec<&str> = A::states().list.iter().map(|(name, _)| *name).collect();
    page::write(folder, &app, &names, &sheets, references)?;
    println!("The page for review: {}", folder.join("index.html").display());
    Ok(())
}

/// Compares every named state of an app, at its normal and smallest size, with the proof last approved, and prints what changed in words. Fails when anything changed or was never approved, so a check can stop on it.
pub fn changes<A: App>() -> Result<(), String> {
    let folder = changes::folder();
    let sheets = changes::survey::<A>(&folder);
    let waiting: Vec<&changes::Sheet> = sheets.iter().filter(|s| s.verdict != changes::Verdict::Same).collect();
    for sheet in &waiting {
        match sheet.verdict {
            changes::Verdict::New => println!("{}: never approved", sheet.name),
            _ => println!("{}:\n  {}", sheet.name, sheet.said.replace('\n', "\n  ")),
        }
    }
    match waiting.len() {
        0 => {
            println!("Nothing renders differently from what was approved, in any of {} sheets.", sheets.len());
            Ok(())
        }
        n => Err(format!("{n} of {} sheets differ from what was approved in {}", sheets.len(), folder.display())),
    }
}

/// Approves every named state of an app as it renders now, writing its proofs.
pub fn approve<A: App>() -> Result<(), String> {
    let folder = changes::folder();
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let sheets = changes::survey::<A>(&folder);
    for sheet in &sheets {
        std::fs::write(&sheet.file, &sheet.now).map_err(|e| format!("{} couldn't be saved: {e}", sheet.file))?;
    }
    println!("Approved {} sheets in {}", sheets.len(), folder.display());
    Ok(())
}

/// Runs one of Specimen's modes without a window, when the command line asks for one: `--render <folder> [--references <folder>]`, `--changes`, `--approve`, or `--assay` for Assay's report. Returns `None` for anything else, so the caller opens its window.
pub fn offscreen<A: App>(mut args: impl Iterator<Item = String>) -> Option<Result<(), String>> {
    Some(match args.next()?.as_str() {
        "--render" => {
            let folder = std::path::PathBuf::from(args.next().unwrap_or_else(|| ".".into()));
            let references = (args.next().as_deref() == Some("--references")).then(|| args.next()).flatten().map(std::path::PathBuf::from);
            render::<A>(&folder, references.as_deref())
        }
        "--changes" => changes::<A>(),
        "--approve" => approve::<A>(),
        "--assay" => assay::report::<A>(),
        _ => return None,
    })
}
