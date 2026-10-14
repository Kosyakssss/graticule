use graticule_macros::{Commands, Facts};
use kernel::{App, Available, Cx, Message, Ui};
use ledger::{Key, List, Outcome, Selection};

/// How many frames Scope keeps, newest first.
pub const KEPT: usize = 600;

/// The frame budget: no frame takes longer than a 1000 Hz display's refresh.
pub const BUDGET_US: u32 = 1000;

/// Scope's facts: the apps running on this machine, the one it is attached to, and what that app's last frames cost and why they were drawn.
#[derive(Facts, Default)]
pub struct Scope {
    #[session]
    pub view: View,
    #[session]
    pub apps: List<Running>,
    #[session]
    picked: Selection<Running>,
    #[session]
    pub attached: Option<Key<Running>>,
    #[session]
    launch: List<Stage>,
    #[session]
    frames: List<Frame>,
    #[session]
    reading: Selection<Frame>,
    #[session]
    pub looks: u32,
}

#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Apps,
    Time,
    Redraws,
}

/// A Graticule app running on this machine, which Scope can attach to.
#[derive(Facts)]
pub struct Running {
    #[key]
    pub id: Key<Running>,
    pub name: String,
    pub port: u16,
}

#[derive(Facts)]
pub struct Stage {
    #[key]
    id: Key<Stage>,
    pub name: String,
    pub ms: f32,
}

/// One frame of the attached app, as its recorder sent it. Times are in microseconds.
#[derive(Facts)]
pub struct Frame {
    #[key]
    id: Key<Frame>,
    pub number: u64,
    pub at: u64,
    pub cause: String,
    pub update: u32,
    pub view: u32,
    pub layout: u32,
    pub paint: u32,
    pub gpu: u32,
    pub after: u32,
    pub total: u32,
}

impl Frame {
    pub fn of(s: kernel::scope::Stamp) -> Frame {
        Frame {
            id: Key::fixed(s.frame + 1),
            number: s.frame,
            at: s.at,
            cause: s.cause.name().to_string(),
            update: s.update,
            view: s.view,
            layout: s.layout,
            paint: s.paint,
            gpu: s.gpu,
            after: s.after,
            total: s.total,
        }
    }
}

impl Stage {
    pub fn of(n: u64, name: &str, ms: f32) -> Stage {
        Stage { id: Key::fixed(n), name: name.to_string(), ms }
    }
}

impl Running {
    pub fn of(pid: u64, name: &str, port: u16) -> Running {
        Running { id: Key::fixed(pid), name: name.to_string(), port }
    }
}

#[derive(Commands)]
pub enum Cmd {
    #[label("Look for running apps")]
    Look,
    #[label("Attach to “{name}”", short: "Attach")]
    Attach(#[subject] Key<Running>),
    #[label("Detach")]
    Detach,
    #[label("Running apps")]
    Apps,
    #[label("Time")]
    Time,
    #[label("Why it redrew")]
    Redraws,
    #[internal]
    Found(Vec<Running>),
    #[internal]
    Launched(Vec<Stage>),
    #[internal]
    Arrived(Vec<Frame>),
    #[internal]
    Lost,
}

impl App for Scope {
    type Cmd = Cmd;

    fn available(&self, cmd: &Cmd) -> Available {
        let here = |view: View| kernel::no_if(self.view == view, Message::from("It is showing"));
        let attached = || kernel::no_if(self.attached.is_none(), Message::from("Attach to an app first"));
        match cmd {
            Cmd::Attach(app) if self.attached == Some(*app) => Available::No(Message::from("Scope is attached to it")),
            Cmd::Attach(app) => kernel::no_if(!self.apps.has(*app), Message::from("That app has stopped")),
            Cmd::Detach => attached(),
            Cmd::Apps => here(View::Apps),
            Cmd::Time => match attached() {
                Available::Yes => here(View::Time),
                no => no,
            },
            Cmd::Redraws => match attached() {
                Available::Yes => here(View::Redraws),
                no => no,
            },
            Cmd::Look | Cmd::Found(_) | Cmd::Launched(_) | Cmd::Arrived(_) | Cmd::Lost => Available::Yes,
        }
    }

    fn update(&mut self, cmd: Cmd, cx: &mut Cx<Cmd>) -> Outcome {
        match cmd {
            Cmd::Look => {
                self.looks += 1;
                Outcome::Changed
            }
            Cmd::Found(apps) => {
                let _ = self.apps.replace(apps);
                if self.attached.is_some_and(|a| !self.apps.has(a)) {
                    self.attached = None;
                    self.view = View::Apps;
                }
                Outcome::Changed
            }
            Cmd::Attach(app) => {
                self.attached = Some(app);
                let _ = self.launch.replace([]);
                let _ = self.frames.replace([]);
                self.view = View::Time;
                Outcome::Changed
            }
            Cmd::Detach => {
                self.attached = None;
                self.view = View::Apps;
                Outcome::Changed
            }
            Cmd::Apps => self.show(View::Apps),
            Cmd::Time => self.show(View::Time),
            Cmd::Redraws => self.show(View::Redraws),
            Cmd::Launched(stages) => {
                let _ = self.launch.replace(stages);
                Outcome::Changed
            }
            Cmd::Arrived(frames) => {
                for frame in frames {
                    let _ = self.frames.insert_after(None, frame);
                }
                while self.frames.len() > KEPT {
                    let Some(oldest) = self.frames.keys().next_back() else { break };
                    let _ = self.frames.remove(oldest);
                }
                Outcome::Changed
            }
            Cmd::Lost => {
                let name = self.attached.and_then(|a| self.apps.find(a)).map(|a| a.name.clone()).unwrap_or_default();
                self.attached = None;
                self.view = View::Apps;
                cx.say(Message::plain(format!("“{name}” closed, so Scope let go of it")))
            }
        }
    }

    fn states() -> kernel::States<Self> {
        let attached = |view: View| {
            let app = Running::of(4242, "To do", 50_000);
            let id = app.id;
            let causes = [kernel::scope::Cause::Pointer, kernel::scope::Cause::Key, kernel::scope::Cause::Woken, kernel::scope::Cause::Wheel];
            let frames = (0..40u64)
                .rev()
                .map(|n| {
                    let total = 180 + ((n * 37) % 11) as u32 * 70 + if n == 17 { 900 } else { 0 };
                    let stamp = kernel::scope::Stamp {
                        frame: n,
                        at: 400_000 + n * 16_000,
                        cause: causes[(n % 4) as usize],
                        update: 20,
                        view: total / 5,
                        layout: total / 6,
                        paint: total / 3,
                        gpu: total / 5,
                        after: total / 20,
                        total,
                    };
                    Frame::of(stamp)
                })
                .collect();
            let launch = [("event loop", 4.1), ("system", 2.0), ("window", 31.5), ("gpu", 88.2), ("fonts", 1.2), ("app", 0.6), ("first frame", 9.8)];
            Scope { view, apps: [app].into_iter().collect(), attached: Some(id), launch: launch.iter().zip(1..).map(|((name, ms), n)| Stage::of(n, name, *ms)).collect(), frames, ..Scope::default() }
        };
        let running = Scope { apps: [Running::of(4242, "To do", 50_000), Running::of(4243, "Specimen", 50_001)].into_iter().collect(), ..Scope::default() };
        kernel::States::new().add("No apps", Scope::default()).add("Two apps running", running).add("Time", attached(View::Time)).add("Why it redrew", attached(View::Redraws))
    }

    fn view(&self, ui: &mut Ui<Cmd>) {
        match (self.view, self.attached) {
            (View::Time, Some(_)) => self.time(ui),
            (View::Redraws, Some(_)) => self.redraws(ui),
            _ => self.running(ui),
        }
    }
}

fn ms(us: u32) -> String {
    format!("{:.2}", us as f32 / 1000.0)
}

impl Scope {
    fn show(&mut self, view: View) -> Outcome {
        self.view = view;
        Outcome::Changed
    }

    fn name(&self) -> String {
        self.attached.and_then(|a| self.apps.find(a)).map(|a| a.name.clone()).unwrap_or_default()
    }

    /// The time the slowest 1% of the kept frames took at most, in microseconds.
    fn slowest(&self) -> u32 {
        let mut totals: Vec<u32> = self.frames.iter().map(|f| f.total).collect();
        totals.sort_unstable();
        let at = (totals.len() * 99).div_ceil(100).saturating_sub(1);
        totals.get(at).copied().unwrap_or(0)
    }

    fn running(&self, ui: &mut Ui<Cmd>) {
        ui.page(Message::from("Scope"), |ui| {
            if self.apps.is_empty() {
                ui.empty(Message::from("No Graticule app is running on this machine. Start one, then look again."), Cmd::Look);
                return;
            }
            ui.muted("The Graticule apps running on this machine. Attach to one to see what each frame costs and why it was drawn.");
            ui.list(&self.apps, Scope::picked, |ui, app| {
                ui.columns(|ui| {
                    ui.column(kernel::Fill, |ui| ui.text(&app.name));
                    ui.column(12, |ui| {
                        ui.link(Cmd::Attach);
                    });
                });
            });
        });
    }

    fn time(&self, ui: &mut Ui<Cmd>) {
        ui.page(Message::plain(format!("Time: {}", self.name())), |ui| {
            let slowest = self.slowest();
            let verdict = if slowest <= BUDGET_US { "within" } else { "over" };
            ui.text(format!("The slowest 1% of the last {} frames took {} ms or less, {verdict} the budget of {} ms.", self.frames.len(), ms(slowest), ms(BUDGET_US)));
            if !self.launch.is_empty() {
                let total: f32 = self.launch.iter().map(|s| s.ms).sum();
                let stages: Vec<String> = self.launch.iter().map(|s| format!("{} {:.1}", s.name, s.ms)).collect();
                ui.muted(format!("Launch took {total:.1} ms: {}.", stages.join(", ")));
            }
            ui.columns(|ui| {
                ui.column(8, |ui| ui.muted("Frame"));
                ui.column(kernel::Fill, |ui| ui.muted("Input, view, layout, paint, GPU, after, total, in ms"));
                ui.column(12, |ui| ui.muted("Of the budget"));
            });
            ui.list(&self.frames, Scope::reading, |ui, f| {
                ui.columns(|ui| {
                    ui.column(8, |ui| ui.mono(f.number.to_string()));
                    ui.column(kernel::Fill, |ui| ui.mono(format!("{} {} {} {} {} {} {}", ms(f.update), ms(f.view), ms(f.layout), ms(f.paint), ms(f.gpu), ms(f.after), ms(f.total))));
                    ui.column(12, |ui| ui.mono(bar(f.total)));
                });
            });
        });
    }

    fn redraws(&self, ui: &mut Ui<Cmd>) {
        ui.page(Message::plain(format!("Why it redrew: {}", self.name())), |ui| {
            let mut causes: Vec<(&str, usize)> = Vec::new();
            for f in self.frames.iter() {
                match causes.iter_mut().find(|(c, _)| *c == f.cause) {
                    Some((_, n)) => *n += 1,
                    None => causes.push((&f.cause, 1)),
                }
            }
            causes.sort_by_key(|c| std::cmp::Reverse(c.1));
            let said: Vec<String> = causes.iter().map(|(c, n)| format!("{c} {n}")).collect();
            ui.text(format!("Of the last {} frames: {}.", self.frames.len(), said.join(", ")));
            ui.list(&self.frames, Scope::reading, |ui, f| {
                ui.columns(|ui| {
                    ui.column(8, |ui| ui.mono(f.number.to_string()));
                    ui.column(12, |ui| ui.mono(format!("{:.3} s", f.at as f64 / 1_000_000.0)));
                    ui.column(kernel::Fill, |ui| ui.text(&f.cause));
                });
            });
        });
    }
}

/// A frame's time as a bar of ten cells, each a tenth of the budget, drawn with block characters so it lines up on the grid. A frame over budget fills the bar and ends in a plus.
fn bar(us: u32) -> String {
    const EIGHTHS: [char; 8] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];
    if us > BUDGET_US {
        return format!("{}+", "█".repeat(10));
    }
    let eighths = (us as u64 * 80).div_ceil(BUDGET_US as u64) as usize;
    let mut s = "█".repeat(eighths / 8);
    if !eighths.is_multiple_of(8) {
        s.push(EIGHTHS[eighths % 8 - 1]);
    }
    s
}
