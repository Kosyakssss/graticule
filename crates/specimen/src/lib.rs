//! SP-01, Specimen: every component and every screen of every app, in every state.

use kernel::Available::{No, Yes};
use kernel::accesskit;
use kernel::ink::{Appearance, Theme};
use kernel::kept::Kept;
use kernel::metrics::Metrics;
use kernel::paint::Painter;
use kernel::runner::{Extra, Runner};
use kernel::shell::{Face, Frame, Input, Seen, Waker};
use kernel::{App, Message};

/// Specimen's Screens place: every named state of an app, each live on its own copy of the facts, so clicks and keys run the app's real code without touching what it saved. Effects are shown, not run.
pub struct Screens<A: App> {
    names: Vec<&'static str>,
    runners: Vec<Runner<A>>,
    chosen: usize,
}

impl<A: App> Screens<A> {
    /// The app's named states, or `fallback` alone when it names none.
    pub fn new(fallback: A) -> Self {
        let mut states = A::states().list;
        if states.is_empty() {
            states.push(("Default", fallback));
        }
        let names = states.iter().map(|(name, _)| *name).collect();
        let runners = states
            .into_iter()
            .map(|(_, facts)| {
                let mut runner = Runner::new(facts);
                runner.hold_effects();
                runner
            })
            .collect();
        let mut screens = Screens { names, runners, chosen: 0 };
        screens.offer();
        screens
    }

    /// Lets the screens' jobs wake the window.
    pub fn wake_with(mut self, waker: Waker) -> Self {
        self.runners = self.runners.into_iter().map(|r| r.wake_with(waker.clone())).collect();
        self
    }

    fn offer(&mut self) {
        let extras: Vec<Extra> = self
            .names
            .iter()
            .enumerate()
            .map(|(i, name)| Extra {
                name: format!("show-{}", name.to_lowercase().replace(' ', "-")),
                label: format!("Show “{name}”"),
                available: if i == self.chosen { No(Message::from("It is showing")) } else { Yes },
            })
            .collect();
        for runner in &mut self.runners {
            runner.host(extras.clone());
        }
    }

    fn runner(&mut self) -> &mut Runner<A> {
        &mut self.runners[self.chosen]
    }

    fn switch(&mut self) -> bool {
        let Some(next) = self.runner().asked() else { return false };
        self.chosen = next;
        self.offer();
        self.runner().kept.reset();
        true
    }
}

impl<A: App> Face for Screens<A> {
    fn theme(&self, appearance: Appearance) -> Theme {
        self.runners[self.chosen].theme(appearance)
    }

    fn paint(&mut self, painter: &mut Painter, seen: Seen) {
        self.runner().paint(painter, seen);
    }

    fn input(&mut self, input: Input, frame: &mut Frame) -> bool {
        let redraw = self.runner().input(input, frame);
        self.switch() || redraw
    }

    fn painted(&mut self, frame: &mut Frame) {
        self.runner().painted(frame);
    }

    fn minimum(&self) -> Option<(i32, i32)> {
        self.runners[self.chosen].minimum()
    }

    fn kept(&mut self) -> Option<&mut Kept> {
        self.runner().kept()
    }

    fn quit(&self) -> bool {
        self.runners[self.chosen].quit()
    }

    fn access(&mut self, metrics: Metrics, title: &str) -> Option<accesskit::TreeUpdate> {
        self.runner().access(metrics, title)
    }

    fn act(&mut self, node: u64, action: accesskit::Action) -> bool {
        let acted = self.runner().act(node, action);
        self.switch() || acted
    }
}

/// Renders every named state of an app in both appearances, at its normal size and at the smallest window it allows, into pictures in `folder`, and prints how long each took to paint.
pub fn render<A: App>(folder: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let mut screen = kernel::offscreen::Offscreen::new(1920, 1280, 2.0);
    for (name, facts) in A::states().list.iter() {
        for (look, appearance) in [("light", Appearance::Light), ("dark", Appearance::Dark)] {
            screen.resize(1920, 1280);
            let mut runner = Runner::new(facts.clone());
            let picture = screen.render_face(appearance, &mut runner);
            picture.save_png(&folder.join(format!("{name}, default, {look}.png")))?;
            println!("{name}, default, {look}: painted in {:.2} ms", picture.paint_ms);
            let Some((cols, rows)) = runner.minimum() else { continue };
            let m = screen.metrics;
            screen.resize((cols as f32 * m.cell_w) as u32, (rows as f32 * m.cell_h) as u32);
            let mut runner = Runner::new(facts.clone());
            let picture = screen.render_face(appearance, &mut runner);
            picture.save_png(&folder.join(format!("{name}, smallest, {look}.png")))?;
            println!("{name}, smallest ({cols} by {rows} cells), {look}: painted in {:.2} ms", picture.paint_ms);
        }
    }
    Ok(())
}
