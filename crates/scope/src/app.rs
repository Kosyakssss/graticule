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
    #[session]
    boxes: List<Measure>,
    #[session]
    looking: Selection<Measure>,
    #[session]
    commands: List<Listed>,
    #[session]
    listing: Selection<Listed>,
    #[session]
    facts: List<Fact>,
    #[session]
    reading_facts: Selection<Fact>,
    #[session]
    errands: List<Errand>,
    #[session]
    reading_errands: Selection<Errand>,
    #[session]
    flaws: List<Flaw>,
    #[session]
    reading_flaws: Selection<Flaw>,
    #[session]
    steps: List<Step>,
    #[session]
    reading_steps: Selection<Step>,
    #[session]
    pub rewind_to: Option<u64>,
    #[session]
    pub rewinds: u32,
    #[session]
    pub change_at: String,
    #[session]
    pub change_to: String,
    #[session]
    pub changes: u32,
    #[session]
    pub asked: u32,
    #[session]
    pub grid: bool,
    #[session]
    pub outlines: bool,
    #[session]
    pub marks: bool,
}

#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Apps,
    Time,
    Redraws,
    Boxes,
    Commands,
    Facts,
    Effects,
    Taste,
    Rewind,
}

/// A block of lines the attached app sent in answer to an ask, read on Scope's reading thread.
#[derive(Facts)]
pub enum Answer {
    Boxes(Vec<Measure>),
    Commands(Vec<Listed>),
    Facts(Vec<Fact>),
    Effects(Vec<Errand>),
    Taste(Vec<Flaw>),
    History(Vec<Step>),
}

impl Answer {
    /// Reads the lines between `<name> begin` and `<name> done`.
    pub fn read(name: &str, lines: &[String]) -> Option<Answer> {
        let numbered = || lines.iter().zip(1u64..);
        match name {
            "boxes" => {
                let mut depths = std::collections::HashMap::new();
                let mut boxes = Vec::new();
                for (line, n) in numbered() {
                    if let Some((i, b)) = Measure::read(n, line, |p| p.and_then(|p| depths.get(&p).map(|d| d + 1)).unwrap_or(0)) {
                        depths.insert(i, b.depth);
                        boxes.push(b);
                    }
                }
                Some(Answer::Boxes(boxes))
            }
            "commands" => Some(Answer::Commands(numbered().filter_map(|(l, n)| Listed::read(n, l)).collect())),
            "facts" => Some(Answer::Facts(numbered().filter_map(|(l, n)| Fact::read(n, l)).collect())),
            "effects" => Some(Answer::Effects(numbered().filter_map(|(l, n)| Errand::read(n, l)).collect())),
            "taste" => Some(Answer::Taste(numbered().filter_map(|(l, n)| Flaw::read(n, l)).collect())),
            "history" => Some(Answer::History(Step::all(lines))),
            _ => None,
        }
    }
}

/// A box on the attached app's screen that breaks a taste law: which law, the line of code that made the box, and what is wrong.
#[derive(Facts)]
pub struct Flaw {
    #[key]
    id: Key<Flaw>,
    pub law: String,
    pub at: String,
    pub said: String,
}

impl Flaw {
    /// Reads a `flaw` line of the wire.
    pub fn read(n: u64, line: &str) -> Option<Flaw> {
        let mut f = line.strip_prefix("flaw\t")?.split('\t');
        Some(Flaw { id: Key::fixed(n), law: f.next()?.to_string(), at: f.next()?.to_string(), said: f.next().unwrap_or_default().to_string() })
    }
}

/// One step of the attached app's history, on the branch undo and redo follow: `number` steps are applied once it is.
#[derive(Facts)]
pub struct Step {
    #[key]
    id: Key<Step>,
    pub number: u64,
    pub what: String,
    pub applied: bool,
}

impl Step {
    /// Reads a history answer: an `applied` line saying how many steps are applied, then a `step` line for each, the first first. The list starts with the state before any step.
    pub fn all(lines: &[String]) -> Vec<Step> {
        let applied: u64 = lines.iter().find_map(|l| l.strip_prefix("applied\t")?.parse().ok()).unwrap_or(0);
        let start = Step { id: Key::fixed(1), number: 0, what: "Before anything was done".into(), applied: true };
        let steps = lines.iter().filter_map(|l| l.strip_prefix("step\t")).zip(1u64..).map(|(what, n)| Step { id: Key::fixed(n + 1), number: n, what: what.to_string(), applied: n <= applied });
        std::iter::once(start).chain(steps).collect()
    }
}

/// One field, item or variant of the attached app's facts, with where it is when it holds a plain value that can be changed from here.
#[derive(Facts)]
pub struct Fact {
    #[key]
    id: Key<Fact>,
    pub depth: u32,
    pub name: String,
    pub value: String,
    pub path: String,
}

impl Fact {
    /// Reads a `fact` line of the wire.
    pub fn read(n: u64, line: &str) -> Option<Fact> {
        let mut f = line.strip_prefix("fact\t")?.split('\t');
        Some(Fact { id: Key::fixed(n), depth: f.next()?.parse().ok()?, name: f.next()?.to_string(), value: f.next().unwrap_or_default().to_string(), path: f.next().unwrap_or_default().to_string() })
    }
}

/// An effect the attached app asked for, or a job it started: what, how long ago in seconds, how long it took in milliseconds, and how it ended.
#[derive(Facts)]
pub struct Errand {
    #[key]
    id: Key<Errand>,
    pub kind: String,
    pub what: String,
    pub ago: String,
    pub took: String,
    pub outcome: String,
}

impl Errand {
    /// Reads an `effect` line of the wire.
    pub fn read(n: u64, line: &str) -> Option<Errand> {
        let mut f = line.strip_prefix("effect\t")?.split('\t');
        Some(Errand {
            id: Key::fixed(n),
            kind: f.next()?.to_string(),
            what: f.next()?.to_string(),
            ago: f.next()?.to_string(),
            took: f.next()?.to_string(),
            outcome: f.next().unwrap_or_default().to_string(),
        })
    }
}

/// One box of the attached app's tree, as it answered when asked.
#[derive(Facts)]
pub struct Measure {
    #[key]
    id: Key<Measure>,
    pub depth: u32,
    pub role: String,
    pub area: String,
    pub at: String,
    pub why: String,
    pub text: String,
}

/// One line of the attached app's commands: a heading, or a command with its keys and why it can't run.
#[derive(Facts)]
pub struct Listed {
    #[key]
    id: Key<Listed>,
    pub heading: bool,
    pub label: String,
    pub keys: String,
    pub why: String,
}

impl Measure {
    /// Reads a `box` line of the wire: the box's index, and the box with `depth` worked out from its parent's index.
    pub fn read(n: u64, line: &str, depth: impl Fn(Option<u64>) -> u32) -> Option<(u64, Measure)> {
        let mut f = line.strip_prefix("box\t")?.split('\t');
        let index: u64 = f.next()?.parse().ok()?;
        let parent = f.next()?.parse().ok();
        let seen = Measure {
            id: Key::fixed(n),
            depth: depth(parent),
            role: f.next()?.to_string(),
            area: f.next()?.to_string(),
            at: f.next()?.to_string(),
            why: f.next()?.to_string(),
            text: f.next().unwrap_or_default().to_string(),
        };
        Some((index, seen))
    }
}

impl Listed {
    /// Reads a `heading` or `command` line of the wire.
    pub fn read(n: u64, line: &str) -> Option<Listed> {
        if let Some(h) = line.strip_prefix("heading\t") {
            return Some(Listed { id: Key::fixed(n), heading: true, label: h.to_string(), keys: String::new(), why: String::new() });
        }
        let mut f = line.strip_prefix("command\t")?.split('\t');
        Some(Listed { id: Key::fixed(n), heading: false, label: f.next()?.to_string(), keys: f.next().unwrap_or_default().to_string(), why: f.next().unwrap_or_default().to_string() })
    }
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
    #[label("Boxes")]
    Boxes,
    #[label("Commands")]
    Commands,
    #[label("Facts")]
    Facts,
    #[label("Effects and jobs")]
    Effects,
    #[label("Taste")]
    Taste,
    #[label("Rewind")]
    Rewind,
    #[label("Go to “{what}”", short: "Go here")]
    GoTo(#[subject] Key<Step>),
    #[label("Change “{name}”…", short: "Change…")]
    Change {
        #[subject]
        fact: Key<Fact>,
        to: String,
    },
    #[label("Ask again")]
    Again,
    #[internal]
    Found(Vec<Running>),
    #[internal]
    Launched(Vec<Stage>),
    #[internal]
    Arrived(Vec<Frame>),
    #[internal]
    Lost,
    #[internal]
    Answered(Answer),
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
            Cmd::Boxes => match attached() {
                Available::Yes => here(View::Boxes),
                no => no,
            },
            Cmd::Commands => match attached() {
                Available::Yes => here(View::Commands),
                no => no,
            },
            Cmd::Facts => match attached() {
                Available::Yes => here(View::Facts),
                no => no,
            },
            Cmd::Effects => match attached() {
                Available::Yes => here(View::Effects),
                no => no,
            },
            Cmd::Taste => match attached() {
                Available::Yes => here(View::Taste),
                no => no,
            },
            Cmd::Rewind => match attached() {
                Available::Yes => here(View::Rewind),
                no => no,
            },
            Cmd::GoTo(step) => match self.steps.find(*step) {
                None => Available::No(Message::from("That step is gone")),
                Some(s) if self.current() == Some(s.number) => Available::No(Message::from("The app is there now")),
                Some(_) => Available::Yes,
            },
            Cmd::Change { fact, .. } => match self.facts.find(*fact) {
                None => Available::No(Message::from("That fact is gone")),
                Some(f) if f.path.is_empty() => Available::No(Message::from("Only a plain value can be changed from here: yes or no, a number, words, or an option that holds nothing")),
                Some(_) => Available::Yes,
            },
            Cmd::Again => kernel::no_if(matches!(self.view, View::Apps | View::Time | View::Redraws), Message::from("Time and redraws arrive by themselves")),
            Cmd::Look | Cmd::Found(_) | Cmd::Launched(_) | Cmd::Arrived(_) | Cmd::Lost | Cmd::Answered(_) => Available::Yes,
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
                let _ = self.boxes.replace([]);
                let _ = self.commands.replace([]);
                let _ = self.facts.replace([]);
                let _ = self.errands.replace([]);
                let _ = self.flaws.replace([]);
                let _ = self.steps.replace([]);
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
            Cmd::Boxes => {
                self.asked += 1;
                self.show(View::Boxes)
            }
            Cmd::Commands => {
                self.asked += 1;
                self.show(View::Commands)
            }
            Cmd::Again => {
                self.asked += 1;
                Outcome::Changed
            }
            Cmd::Facts => {
                self.asked += 1;
                self.show(View::Facts)
            }
            Cmd::Effects => {
                self.asked += 1;
                self.show(View::Effects)
            }
            Cmd::Taste => {
                self.asked += 1;
                self.show(View::Taste)
            }
            Cmd::Rewind => {
                self.asked += 1;
                self.show(View::Rewind)
            }
            Cmd::Change { fact, to } => {
                let Some(path) = self.facts.find(fact).map(|f| f.path.clone()) else { return Outcome::Nothing };
                (self.change_at, self.change_to) = (path, to);
                self.changes += 1;
                Outcome::Changed
            }
            Cmd::GoTo(step) => {
                let Some(number) = self.steps.find(step).map(|s| s.number) else { return Outcome::Nothing };
                self.rewind_to = Some(number);
                self.rewinds += 1;
                Outcome::Changed
            }
            Cmd::Answered(answer) => {
                let _ = match answer {
                    Answer::Boxes(boxes) => self.boxes.replace(boxes),
                    Answer::Commands(commands) => self.commands.replace(commands),
                    Answer::Facts(facts) => self.facts.replace(facts),
                    Answer::Effects(errands) => self.errands.replace(errands),
                    Answer::Taste(flaws) => self.flaws.replace(flaws),
                    Answer::History(steps) => self.steps.replace(steps),
                };
                Outcome::Changed
            }
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
        let boxes = [
            "box\t0\t\twindow\t0 0 80 30\tapps/todo/src/main.rs:90\tthe whole window\t",
            "box\t1\t0\tpage\t0 0 80 28\tapps/todo/src/main.rs:91\t80 columns, as wide as its window; 28 rows, given by its window\tTo do",
            "box\t2\t1\tfield\t2 2 60 1\tapps/todo/src/main.rs:93\t60 columns, as the code asks; 1 row, what it needs at that width\t",
            "box\t3\t1\tlist\t2 4 76 22\tapps/todo/src/main.rs:96\t76 columns, as wide as its page; 22 rows of the 40 it needs, so the rest scrolls or is cut\t",
            "box\t4\t3\tcheckbox\t2 4 40 1\tapps/todo/src/main.rs:98\t40 columns, as wide as what it shows; 1 row, what it needs at that width\tWater the plants",
        ];
        let mut depths = std::collections::HashMap::new();
        let mut seen = Vec::new();
        for (line, n) in boxes.iter().zip(1..) {
            if let Some((i, b)) = Measure::read(n, line, |p| p.and_then(|p| depths.get(&p).map(|d| d + 1)).unwrap_or(0)) {
                depths.insert(i, b.depth);
                seen.push(b);
            }
        }
        let commands = [
            "heading\tCommands",
            "command\tAdd a task\tenter\t",
            "command\tMark “Water the plants” done\tx\t",
            "command\tRemove done tasks\t\tNothing is done yet",
            "heading\tGraticule’s own",
            "command\tUndo\tctrl+z\tThere is nothing to undo",
        ];
        let commands = commands.iter().zip(1..).filter_map(|(l, n)| Listed::read(n, l)).collect();
        let measured = Scope { boxes: seen.into_iter().collect(), outlines: true, ..attached(View::Boxes) };
        let listed = Scope { commands, ..attached(View::Commands) };
        let facts = [
            "fact\t0\tfacts\t",
            "fact\t1\ttasks\t2 items",
            "fact\t2\t1\t",
            "fact\t3\ttitle\t“Water the plants”\ts:d:0/#1/1",
            "fact\t3\tdone\tno\tb:d:0/#1/2",
            "fact\t2\t2\t",
            "fact\t3\ttitle\t“Call the bank”\ts:d:0/#2/1",
            "fact\t3\tdone\tyes\tb:d:0/#2/2",
            "fact\t1\tfilter\tAll\tv:s:1",
        ];
        let facts = facts.iter().zip(1..).filter_map(|(l, n)| Fact::read(n, l)).collect();
        let errands = ["effect\tjob\tFetching the feeds\t2.4\t2400.0\trunning", "effect\twrite\ttasks.txt\t31.0\t3.2\tdone", "effect\topen\thttps://example.org\t95.5\t0.0\tdone"];
        let errands = errands.iter().zip(1..).filter_map(|(l, n)| Errand::read(n, l)).collect();
        let read_facts = Scope { facts, ..attached(View::Facts) };
        let effects = Scope { errands, ..attached(View::Effects) };
        let flaws = ["flaw\ton the grid\tapps/todo/src/main.rs:104\ta text at column 70, row 6, 14 by 1 cells leaves its parent, a column at column 70, row 6, 10 by 1 cells"];
        let flaws = flaws.iter().zip(1..).filter_map(|(l, n)| Flaw::read(n, l)).collect();
        let tasteful = Scope { marks: true, ..attached(View::Taste) };
        let flawed = Scope { flaws, marks: true, ..attached(View::Taste) };
        let history: Vec<String> = ["applied\t2", "step\tAdd “Water the plants”", "step\tAdd “Call the bank”", "step\tMark “Call the bank” done"].iter().map(|s| s.to_string()).collect();
        let rewind = Scope { steps: Step::all(&history).into_iter().collect(), ..attached(View::Rewind) };
        kernel::States::new()
            .add("No apps", Scope::default())
            .add("Two apps running", running)
            .add("Time", attached(View::Time))
            .add("Why it redrew", attached(View::Redraws))
            .add("Boxes", measured)
            .add("Commands", listed)
            .add("Facts", read_facts)
            .add("Effects and jobs", effects)
            .add("Taste", tasteful)
            .add("A taste law broken", flawed)
            .add("Rewind", rewind)
    }

    fn view(&self, ui: &mut Ui<Cmd>) {
        match (self.view, self.attached) {
            (View::Time, Some(_)) => self.time(ui),
            (View::Redraws, Some(_)) => self.redraws(ui),
            (View::Boxes, Some(_)) => self.measured(ui),
            (View::Commands, Some(_)) => self.listed(ui),
            (View::Facts, Some(_)) => self.read_facts(ui),
            (View::Effects, Some(_)) => self.effects(ui),
            (View::Taste, Some(_)) => self.taste(ui),
            (View::Rewind, Some(_)) => self.rewind(ui),
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

impl Scope {
    fn measured(&self, ui: &mut Ui<Cmd>) {
        ui.page(Message::plain(format!("Boxes: {}", self.name())), |ui| {
            ui.muted("Every box on the app's screen, inside the one it sits in: its place in cells, the line of code that made it, and why it is that size.");
            ui.switch("Show the grid over the app", Scope::grid);
            ui.switch("Outline every box in the app", Scope::outlines);
            if self.boxes.is_empty() {
                ui.empty(Message::from("No answer yet. The app answers after its next frame."), Cmd::Again);
                return;
            }
            ui.list(&self.boxes, Scope::looking, |ui, b| {
                let indent = "    ".repeat(b.depth.min(12) as usize);
                let text = if b.text.is_empty() { String::new() } else { format!(" “{}”", b.text) };
                ui.columns(|ui| {
                    ui.column(kernel::Fill, |ui| ui.text(format!("{indent}{}{text}", b.role)));
                    ui.column(14, |ui| ui.mono(&b.area));
                    ui.column(kernel::Fill, |ui| ui.mono(&b.at));
                    ui.column(kernel::Fill, |ui| ui.muted(&b.why));
                });
            });
        });
    }

    fn read_facts(&self, ui: &mut Ui<Cmd>) {
        ui.page(Message::plain(format!("Facts: {}", self.name())), |ui| {
            ui.muted("The app's facts as they are now, each field inside the one it belongs to.");
            if self.facts.is_empty() {
                ui.empty(Message::from("No answer yet. The app answers after its next frame."), Cmd::Again);
                return;
            }
            ui.list(&self.facts, Scope::reading_facts, |ui, f| {
                let indent = "    ".repeat(f.depth.min(12) as usize);
                ui.columns(|ui| {
                    ui.column(kernel::Fill, |ui| ui.text(format!("{indent}{}", f.name)));
                    ui.column(kernel::Fill, |ui| ui.mono(&f.value));
                });
            });
        });
    }

    fn effects(&self, ui: &mut Ui<Cmd>) {
        ui.page(Message::plain(format!("Effects and jobs: {}", self.name())), |ui| {
            ui.muted("What the app asked for lately, newest first: files written, links opened, jobs started, and how long each took.");
            if self.errands.is_empty() {
                ui.empty(Message::from("The app hasn't asked for anything yet."), Cmd::Again);
                return;
            }
            ui.list(&self.errands, Scope::reading_errands, |ui, e| {
                ui.columns(|ui| {
                    ui.column(8, |ui| ui.text(&e.kind));
                    ui.column(kernel::Fill, |ui| ui.text(&e.what));
                    ui.column(14, |ui| ui.mono(format!("{} s ago", e.ago)));
                    ui.column(16, |ui| ui.mono(format!("{} {} ms", if e.outcome == "running" { "for" } else { "took" }, e.took)));
                    ui.column(10, |ui| ui.text(&e.outcome));
                });
            });
        });
    }

    /// How many steps the attached app has applied, as it last said.
    fn current(&self) -> Option<u64> {
        self.steps.iter().filter(|s| s.applied).map(|s| s.number).max()
    }

    fn rewind(&self, ui: &mut Ui<Cmd>) {
        ui.page(Message::plain(format!("Rewind: {}", self.name())), |ui| {
            ui.muted("Every step the app's history holds on this branch, first first. Going to a step undoes or redoes everything after it, one step at a time, so the app's own undo brings it back.");
            if self.steps.is_empty() {
                ui.empty(Message::from("No answer yet. The app answers after its next frame."), Cmd::Again);
                return;
            }
            let current = self.current();
            ui.list(&self.steps, Scope::reading_steps, |ui, s| {
                ui.columns(|ui| {
                    ui.column(4, |ui| ui.muted(if Some(s.number) == current { "now" } else { "" }));
                    ui.column(6, |ui| ui.mono(s.number.to_string()));
                    ui.column(kernel::Fill, |ui| if s.applied { ui.text(&s.what) } else { ui.muted(&s.what) });
                    ui.column(10, |ui| {
                        ui.link(Cmd::GoTo);
                    });
                });
            });
        });
    }

    fn taste(&self, ui: &mut Ui<Cmd>) {
        ui.page(Message::plain(format!("Taste: {}", self.name())), |ui| {
            ui.muted("The boxes on the app's screen that break a taste law, as Assay judges them.");
            ui.switch("Mark each one on the app", Scope::marks);
            if self.flaws.is_empty() {
                ui.empty(Message::from("Nothing on the app's screen breaks a taste law."), Cmd::Again);
                return;
            }
            ui.list(&self.flaws, Scope::reading_flaws, |ui, f| {
                ui.columns(|ui| {
                    ui.column(14, |ui| ui.text(&f.law));
                    ui.column(28, |ui| ui.mono(&f.at));
                    ui.column(kernel::Fill, |ui| ui.text(&f.said));
                });
            });
        });
    }

    fn listed(&self, ui: &mut Ui<Cmd>) {
        ui.page(Message::plain(format!("Commands: {}", self.name())), |ui| {
            ui.muted("Every command the app has right now, with its keys, and why it can't run when it can't.");
            if self.commands.is_empty() {
                ui.empty(Message::from("No answer yet. The app answers after its next frame."), Cmd::Again);
                return;
            }
            ui.list(&self.commands, Scope::listing, |ui, c| {
                if c.heading {
                    ui.heading(&c.label);
                    return;
                }
                ui.columns(|ui| {
                    ui.column(kernel::Fill, |ui| ui.text(&c.label));
                    ui.column(16, |ui| ui.mono(&c.keys));
                    ui.column(kernel::Fill, |ui| ui.muted(if c.why.is_empty() { "Can run" } else { &c.why }));
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
