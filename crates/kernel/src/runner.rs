use std::any::Any;
use std::collections::{HashMap, VecDeque};
use std::fmt::Write as _;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use ledger::{Change, Choose, Edit, ErasedPath, History, Op, Store, Transaction, Value};
use winit::event::{Ime, KeyEvent};

use crate::app::{App, Cx, Effect, Failure, Jobs, Moment, Path, Reply, Said};
use crate::builtin::Builtin;
use crate::command::{Available, Commands, Form};
use crate::components::Look;
use crate::field::{self, Action, Caret};
use crate::hit::Hits;
use crate::kept::Kept;
use crate::keys::{self, Chord, Key, Keymap, Lookup, Mode, Step};
use crate::layout::Layout;
use crate::look::{Choice, Looks, Offered, Style, Switch};
use crate::message::Message;
use crate::metrics::Metrics;
use crate::paint::Painter;
use crate::shell::{Button, Face, Frame, Input, Mods, Seen, Waker};
use crate::text::Text;
use crate::tree::{Id, Kind, ListInfo, NONE, Range32, Tree};
use crate::ui::{Scroll, Scrolls, Ui};
use crate::value::{Calendar, Dates};

const CHOICES: usize = 12;
const PALETTE: Id = Id(0x9a1e_77e5_0000_0001);
const COPY: Id = Id(0x9a1e_77e5_0000_0002);

/// Where the user was: the focused component and its caret. Each undo step keeps it, so undo puts the user back.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Place {
    pub focus: Option<Id>,
    pub caret: Option<Caret>,
}

#[derive(Clone, Debug, Default)]
struct Palette {
    query: String,
    caret: Caret,
    chosen: usize,
    shown: usize,
    asking: Option<Asking>,
}

/// A command the palette is asking the arguments of: its entry and the values read so far.
#[derive(Clone, Debug)]
struct Asking {
    entry: usize,
    got: Vec<Value>,
}

/// A page the kernel draws in place of the app's: the help, or the log of what was done.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sheet {
    Help,
    Log,
}

/// One line of the help page.
enum Help {
    Heading(String),
    Item { label: String, keys: String, state: String },
    Doc(String),
    Dated(Moment, String),
}

/// A command named by a key, the palette or the bar, once its subjects are filled in.
enum Target<C> {
    Builtin(Builtin),
    Host(usize),
    One(C),
    Many(Vec<C>, usize),
    Ask(usize),
    Refused(Option<C>, Message),
}

/// Text chosen with the pointer across the page's text and values: where the drag started and where it is now, each a node and a byte in its text. Kept by id, so it stays on the same text when the tree is built again, and dropped when that text is gone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Reading {
    anchor: (Id, usize),
    head: (Id, usize),
}

/// The list commands act on right now, and the keys they act on.
type Here = Option<(ListInfo, Vec<u128>)>;

/// A derived value worked out on another thread: its field, the versions of the fields it was worked out from, and the value.
type Worked = (u16, Vec<u64>, Box<dyn Any + Send>);

/// What a command was given that isn't the same on every run: whose keys, from which counter, the time and a seed. The command log keeps it, so a replay gives the same facts.
#[derive(Clone, Copy, Debug)]
struct Clock {
    replica: u64,
    counter: u64,
    now: Moment,
    seed: u64,
}

impl Clock {
    fn of(entry: &Value) -> Option<Clock> {
        let (Some(Value::UInt(replica)), Some(Value::UInt(counter)), Some(Value::Int(now)), Some(Value::UInt(seed))) =
            (entry.field("replica"), entry.field("counter"), entry.field("now"), entry.field("seed"))
        else {
            return None;
        };
        Some(Clock { replica: *replica, counter: *counter, now: Moment(*now), seed: *seed })
    }

    fn fields(&self) -> [(String, Value); 4] {
        [
            ("replica".into(), Value::UInt(self.replica)),
            ("counter".into(), Value::UInt(self.counter)),
            ("now".into(), Value::Int(self.now.0)),
            ("seed".into(), Value::UInt(self.seed)),
        ]
    }
}

/// Where committed entries go: nowhere, a list in memory for replays, or the store on disk.
enum Keep {
    Nothing,
    Memory(Vec<Value>),
    Store(Store),
}

/// A command still landing, because it changes many items or runs over many subjects: its operations so far, the subjects still to run and how many there are, kept as one undo step when the last slice lands.
struct Background<C> {
    label: String,
    gesture: u64,
    place: Place,
    ops: Vec<Op>,
    parts: VecDeque<Part>,
    rest: VecDeque<C>,
    left: usize,
}

struct Batch<C> {
    ran: Vec<Value>,
    ops: Vec<Op>,
    deferred: Vec<ledger::Bulk>,
    cx: Cx<C>,
    changed: bool,
    stopped: Option<(usize, Message)>,
    failed: Option<String>,
}

/// An undo or redo too big for one frame, landing a slice per frame: its operations, how many have landed, and where to put the user when the last one does.
struct Turning {
    label: String,
    ops: Vec<Op>,
    next: usize,
    place: Place,
}

struct Part {
    bulk: ledger::Bulk,
    found: Found,
    next: usize,
}

enum Found {
    Searching(mpsc::Receiver<Vec<u128>>),
    Ready(Vec<u128>),
}

/// How much of a bulk change one slice applies: as much as fits in a time, or a set number of items, as replays and laws do.
#[derive(Clone, Copy, Debug)]
pub enum Slice {
    Time(Duration),
    Items(usize),
}

const SLICE: Duration = Duration::from_micros(300);

/// Background work an effect started: its name for the status line, and what turns its result into the app's command. A result of `None` means it ended with nothing to say, such as when the user called off a save.
struct Job<C> {
    name: String,
    receive: mpsc::Receiver<Option<Box<dyn Any + Send>>>,
    reply: Reply<C>,
    lost: Option<C>,
}

/// A command the host around the app adds to its window, such as Specimen's "Show “Empty”". It shows in the bar and the palette beside the app's own; the host learns it was chosen through `Runner::asked`.
#[derive(Clone, Debug)]
pub struct Extra {
    pub name: String,
    pub label: String,
    pub available: Available,
}

/// Runs an app in a window: builds its tree from the facts, lays it out, keeps its quads and turns input into commands.
pub struct Runner<A: App> {
    pub facts: A,
    pub history: History<Place>,
    pub tree: Tree<A::Cmd>,
    scrolls: Scrolls,
    pub kept: Kept,
    hits: Hits,
    hover: Option<Id>,
    pressed: Option<Id>,
    focus: Option<Id>,
    stale: bool,
    laid: bool,
    metrics: Option<Metrics>,
    said: Option<Said>,
    status: u32,
    stack: Vec<u32>,
    widths: Vec<i32>,
    replica: u64,
    counter: u64,
    gesture: u64,
    minimum: Option<(i32, i32)>,
    carets: HashMap<Id, Caret>,
    kill: String,
    typing: Option<(Id, field::Kind, bool)>,
    composing: String,
    composed: Range32,
    reading: Option<Reading>,
    reading_drag: bool,
    drag: Option<Id>,
    pending: Vec<Chord>,
    count: u32,
    keymap: Keymap,
    mode: Mode,
    sheet: Option<(Sheet, i32)>,
    done: VecDeque<(Moment, String)>,
    inspecting: bool,
    naming: Option<String>,
    only_changed: bool,
    palette: Option<Palette>,
    clipboard: Option<arboard::Clipboard>,
    reveal: bool,
    ime: bool,
    repairing: bool,
    keep: Keep,
    background: VecDeque<Background<A::Cmd>>,
    turning: Option<Turning>,
    waker: Option<Waker>,
    replaying: bool,
    effects: Vec<Effect<A::Cmd>>,
    jobs: Vec<Job<A::Cmd>>,
    versions: HashMap<u16, u64>,
    deriving: HashMap<u16, Vec<u64>>,
    working: Vec<u16>,
    derived: (mpsc::Sender<Worked>, mpsc::Receiver<Worked>),
    extras: Vec<Extra>,
    asked: Option<usize>,
    hold: bool,
    fonts: u64,
    stages: crate::scope::Stages,
    drawn_in: Option<(crate::ink::Theme, Style)>,
    switch: Option<Switch>,
    offered: Offered,
    offered_problem: Option<String>,
    calendar: Calendar,
    keys_file: Option<std::path::PathBuf>,
    keys_watch: Option<(notify::RecommendedWatcher, mpsc::Receiver<()>)>,
}

impl<A: App> Runner<A> {
    pub fn new(mut facts: A) -> Self {
        ledger::Field::attach(&mut facts, ErasedPath::ROOT);
        let (keymap, said) = read_keymap::<A>(A::Cmd::keymap(), "The keymap").unwrap_or_else(|said| (Keymap::default(), Some(said)));
        let mut started = Jobs::new();
        facts.start(&mut started);
        Self {
            facts,
            history: History::new(),
            tree: Tree::default(),
            scrolls: Scrolls::new(),
            kept: Kept::default(),
            hits: Hits::default(),
            hover: None,
            pressed: None,
            focus: None,
            stale: true,
            laid: false,
            metrics: None,
            said,
            status: NONE,
            stack: Vec::new(),
            widths: Vec::new(),
            replica: ledger::fresh_replica(),
            counter: 0,
            gesture: 0,
            minimum: None,
            carets: HashMap::new(),
            kill: String::new(),
            typing: None,
            composing: String::new(),
            composed: Range32::default(),
            reading: None,
            reading_drag: false,
            drag: None,
            pending: Vec::new(),
            count: 0,
            keymap,
            mode: Mode::Normal,
            sheet: None,
            done: VecDeque::new(),
            inspecting: false,
            naming: None,
            only_changed: false,
            palette: None,
            clipboard: None,
            reveal: false,
            ime: false,
            repairing: false,
            keep: Keep::Nothing,
            background: VecDeque::new(),
            turning: None,
            waker: None,
            replaying: false,
            effects: started.effects,
            jobs: Vec::new(),
            versions: HashMap::new(),
            deriving: HashMap::new(),
            working: Vec::new(),
            derived: mpsc::channel(),
            extras: Vec::new(),
            asked: None,
            hold: false,
            fonts: 0,
            stages: crate::scope::Stages::default(),
            drawn_in: None,
            switch: None,
            offered: Offered::default(),
            offered_problem: None,
            calendar: Calendar::default(),
            keys_file: None,
            keys_watch: None,
        }
    }

    /// Opens the facts kept in `folder`, or makes them with `fresh` on the first launch, puts the user back where they were and saves every change there from now on. If the folder can't be used, the app runs unsaved and the message line says why.
    pub fn open(folder: &std::path::Path, fresh: impl Fn() -> A) -> Self {
        match Store::open(folder, &fresh) {
            Ok((store, opened)) => {
                let mut runner = Self::new(opened.facts);
                runner.save_to(store, opened.counter);
                if let Some(place) = &opened.place {
                    runner.return_to(place);
                }
                if let Some(trouble) = opened.trouble {
                    runner.said = Some(Said::Failure(trouble));
                }
                runner.keys_file = Some(folder.join(KEYMAP));
                runner.read_keys();
                runner
            }
            Err(e) => {
                let mut runner = Self::new(fresh());
                runner.said = Some(Said::Failure(format!("Changes won't be saved, because {} couldn't be opened: {e}", folder.display())));
                runner
            }
        }
    }

    /// Builds the tree from the facts: the app's view, the palette when it is open, then the window's own lines, then every label and availability filled in.
    pub fn build(&mut self) {
        self.pin();
        if self.calendar.live {
            self.calendar.now = Moment::now();
        }
        self.tree.calendar = self.calendar.clone();
        self.tree.only_changed = self.only_changed;
        let root = self.tree.root();
        A::view(&self.facts, &mut Ui::new(&mut self.tree, &self.facts, &self.scrolls, root));
        self.tree.reindex();
        self.follow();
        let here = self.glance();
        let root = match self.sheet {
            Some((sheet, top)) => {
                let lines = match sheet {
                    Sheet::Help => self.help_lines(&here),
                    Sheet::Log => self.log_lines(),
                };
                let root = self.tree.root();
                self.build_sheet(root, sheet, lines, top);
                root
            }
            None => root,
        };
        if self.palette.is_some() {
            self.build_palette(root, &here);
        }
        let note = self.tree.add(root, Kind::Note, Some(Id::ROOT.mix(1)), std::panic::Location::caller());
        match &self.said {
            Some(Said::Note(m)) => self.tree.nodes[note as usize].text = self.tree.put(m.as_str()),
            Some(Said::Failure(f)) => {
                self.tree.nodes[note as usize].text = self.tree.put(f);
                self.tree.nodes[note as usize].enabled = false;
            }
            None => {}
        }
        self.status = self.tree.add(root, Kind::Status, Some(Id::ROOT.mix(2)), std::panic::Location::caller());
        let bar = self.tree.add(root, Kind::Bar, Some(Id::ROOT.mix(3)), std::panic::Location::caller());
        if self.keymap.modal {
            let m = self.tree.add(bar, Kind::Value { value: crate::value::Value::Mode }, Some(Id::ROOT.mix(10)), std::panic::Location::caller());
            self.tree.nodes[m as usize].text = self.tree.put(self.mode.name());
        }
        if self.pending.is_empty() && self.count == 0 {
            self.build_bar(bar, &here);
        } else {
            self.build_which(bar, &here);
        }
        self.tree.reindex();
        self.resolve();
        self.status();
        self.composed = if self.composing.is_empty() { Range32::default() } else { self.tree.put(&self.composing) };
        self.stale = false;
        self.laid = false;
    }

    fn build_bar(&mut self, bar: u32, here: &Here) {
        for (e, entry) in A::Cmd::entries().iter().enumerate() {
            if entry.internal {
                continue;
            }
            let target = self.target(e, here);
            if let Target::Refused(None, _) = target {
                continue;
            }
            let a = self.tree.add(bar, Kind::Action, Some(Id::ROOT.mix(4).mix(e as u64)), std::panic::Location::caller());
            let keys = self.keymap.keys_for(self.mode, entry.name).unwrap_or_default();
            self.tree.nodes[a as usize].keys = self.tree.put(&keys);
            self.put_target(a, target, entry.name);
        }
        for e in 0..self.extras.len() {
            let a = self.tree.add(bar, Kind::Action, Some(Id::ROOT.mix(9).mix(e as u64)), std::panic::Location::caller());
            self.put_target(a, Target::Host(e), "");
        }
        for b in [Builtin::Undo, Builtin::Redo, Builtin::Palette] {
            let a = self.tree.add(bar, Kind::Action, Some(Id::ROOT.mix(5).mix(b as u64)), std::panic::Location::caller());
            let keys = self.builtin_keys(b);
            self.tree.nodes[a as usize].keys = self.tree.put(&keys);
            self.tree.nodes[a as usize].builtin = Some(b);
        }
    }

    fn build_which(&mut self, bar: u32, here: &Here) {
        let mut next: Vec<(Step, Option<String>)> = Vec::new();
        for (chord, name) in self.keymap.next(self.mode, &self.pending) {
            if !next.iter().any(|(c, _)| *c == chord) {
                next.push((chord, name.map(str::to_string)));
            }
        }
        let typed = self.so_far();
        for (k, (chord, name)) in next.into_iter().enumerate() {
            let a = self.tree.add(bar, Kind::Action, Some(Id::ROOT.mix(6).mix(k as u64)), std::panic::Location::caller());
            self.tree.nodes[a as usize].keys = self.tree.put(&format!("{typed} {chord}"));
            match name {
                Some(name) => {
                    let target = self.named(&name, here);
                    self.put_target(a, target, &name);
                }
                None => {
                    let r = self.tree.put("More…");
                    self.tree.nodes[a as usize].text = r;
                    self.tree.nodes[a as usize].name = r;
                }
            }
        }
    }

    fn target_label(&self, target: &Target<A::Cmd>, name: &str) -> String {
        match target {
            Target::One(c) | Target::Refused(Some(c), _) => c.label_string(&self.facts, Form::Long),
            Target::Many(cs, count) => cs.first().map(|c| c.label_string(&self.facts, Form::Many(*count))).unwrap_or_default(),
            Target::Ask(_) => format!("{}…", title(name)),
            Target::Builtin(b) => b.label().to_string(),
            Target::Host(e) => self.extras[*e].label.clone(),
            Target::Refused(None, _) => title(name),
        }
    }

    fn target_state(&self, target: &Target<A::Cmd>) -> String {
        let available = match target {
            Target::One(c) => self.facts.available(c),
            Target::Many(cs, _) => cs.first().map_or(Available::Yes, |c| self.facts.available(c)),
            Target::Builtin(b) => self.builtin_available(*b),
            Target::Host(e) => self.extras[*e].available.clone(),
            Target::Refused(_, why) => Available::No(why.clone()),
            Target::Ask(_) => Available::Yes,
        };
        available.reason().map(|why| why.as_str().to_string()).unwrap_or_default()
    }

    fn help_lines(&self, here: &Here) -> Vec<Help> {
        let mut lines = vec![Help::Heading("Commands".into())];
        for (e, entry) in A::Cmd::entries().iter().enumerate() {
            if entry.internal {
                continue;
            }
            let target = self.target(e, here);
            let keys = self.keymap.keys_for(self.mode, entry.name).unwrap_or_default();
            lines.push(Help::Item { label: self.target_label(&target, entry.name), keys, state: self.target_state(&target) });
            if !entry.doc.is_empty() {
                lines.push(Help::Doc(entry.doc.to_string()));
            }
        }
        for e in 0..self.extras.len() {
            let target = Target::Host(e);
            lines.push(Help::Item { label: self.target_label(&target, ""), keys: String::new(), state: self.target_state(&target) });
        }
        if !self.keymap.bindings.is_empty() {
            lines.push(Help::Heading(if self.keymap.modal { format!("Keys, in {} mode", self.mode.name()) } else { "Keys".into() }));
            let mut bindings: Vec<_> = self.keymap.bindings.iter().filter(|b| b.mode.is_none_or(|m| m == self.mode)).collect();
            bindings.sort_by_key(|b| keys::label(&b.keys));
            let mut shown: Vec<Vec<Chord>> = Vec::new();
            for b in bindings {
                for depth in 1..b.keys.len() {
                    let prefix = b.keys[..depth].to_vec();
                    if !shown.contains(&prefix) {
                        lines.push(Help::Item { label: "More…".into(), keys: format!("{}{}", "  ".repeat(depth - 1), keys::label(&prefix)), state: String::new() });
                        shown.push(prefix);
                    }
                }
                let depth = b.keys.len() - 1;
                let keys = format!("{}{}{}", "  ".repeat(depth), keys::label(&b.keys), if b.motion { " motion" } else { "" });
                let target = self.named(&b.command, here);
                lines.push(Help::Item { label: self.target_label(&target, &b.command), keys, state: self.target_state(&target) });
            }
        }
        lines.push(Help::Heading("Graticule\u{2019}s own".into()));
        for b in Builtin::ALL {
            let target = Target::Builtin(b);
            lines.push(Help::Item { label: self.target_label(&target, ""), keys: self.builtin_keys(b), state: self.target_state(&target) });
        }
        lines
    }

    fn log_lines(&self) -> Vec<Help> {
        match self.done.len() {
            0 => vec![Help::Doc("Nothing yet: what you do in this window shows here, newest first".into())],
            _ => self.done.iter().rev().map(|(at, what)| Help::Dated(*at, what.clone())).collect(),
        }
    }

    fn did(&mut self, what: String) {
        if self.done.len() == DONE_KEPT {
            self.done.pop_front();
        }
        self.done.push_back((Moment::now(), what));
    }

    fn build_sheet(&mut self, root: u32, sheet: Sheet, lines: Vec<Help>, top: i32) {
        let screen = self.metrics.map_or(crate::grid::Area::new(0, 0, 100, 40), |m| m.screen());
        let budget = (screen.rows - HELP_CHROME).max(1);
        let full = (screen.cols - 4).max(1) as usize;
        let wide = ((screen.cols - 4 - HELP_KEYS) / 2).max(0) as usize;
        let stacked = wide < HELP_LABEL as usize;
        let tall = |line: &Help| -> i32 {
            let rows = |s: &str, w: usize| s.chars().count().div_ceil(w).max(1) as i32;
            match line {
                Help::Heading(_) => 1,
                Help::Doc(text) => rows(text, full),
                Help::Item { label, state, .. } if stacked => rows(label, full) + 1 + rows(state, full),
                Help::Item { label, state, .. } => rows(label, wide).max(rows(state, wide)),
                Help::Dated(_, text) => rows(text, full.saturating_sub(HELP_DATE as usize).max(1)),
            }
        };
        let mut last = lines.len();
        let mut used = 0;
        while last > 0 && used + tall(&lines[last - 1]) <= budget {
            used += tall(&lines[last - 1]);
            last -= 1;
        }
        let last = last.min(lines.len().saturating_sub(1));
        let top = top.clamp(0, last as i32);
        let mut shown = 0;
        let mut used = 0;
        while (top + shown) < lines.len() as i32 && used + tall(&lines[(top + shown) as usize]) <= budget {
            used += tall(&lines[(top + shown) as usize]);
            shown += 1;
        }
        let shown = shown.max(1);
        self.sheet = Some((sheet, top));
        let mut ui = Ui::<A::Cmd>::new(&mut self.tree, &self.facts, &self.scrolls, root);
        let name = match sheet {
            Sheet::Help => "Help",
            Sheet::Log => "What was done, newest first",
        };
        ui.page(Message::plain(format!("{name}, lines {} to {} of {}", top + 1, (top + shown).min(lines.len() as i32), lines.len())), |ui| {
            ui.column(crate::tree::Fill, |ui| {
                for line in lines.into_iter().skip(top as usize).take(shown as usize) {
                    match line {
                        Help::Heading(text) => ui.strong(text),
                        Help::Doc(text) => ui.muted(text),
                        Help::Dated(at, text) => ui.columns(|ui| {
                            ui.column(crate::tree::cols(HELP_DATE), |ui| ui.date(at));
                            ui.column(crate::tree::Fill, |ui| ui.text(text));
                        }),
                        Help::Item { label, keys, state } => ui.columns(|ui| {
                            ui.column(crate::tree::Fill.min(HELP_LABEL), |ui| ui.text(label));
                            ui.column(crate::tree::cols(HELP_KEYS), |ui| ui.mono(keys));
                            ui.column(crate::tree::Fill.min(HELP_LABEL), |ui| ui.muted(state));
                        }),
                    }
                }
            });
        });
        self.tree.reindex();
    }

    fn scroll_help(&mut self, by: i32) -> bool {
        let Some((sheet, top)) = self.sheet else { return false };
        self.sheet = Some((sheet, top.saturating_add(by).max(0)));
        self.stale = true;
        true
    }

    fn inspect_at(&mut self, col: i32, row: i32) -> bool {
        let deepest = self.tree.nodes.iter().rposition(|n| n.kind != Kind::Root && n.area.contains(col, row)).map(|i| i as u32);
        match self.hit(col, row).or(deepest) {
            Some(i) => self.describe(i),
            None => false,
        }
    }

    fn describe(&mut self, i: u32) -> bool {
        self.inspecting = false;
        let n = self.tree.nodes[i as usize];
        let name = if n.name.is_empty() { self.tree.str(n.text) } else { self.tree.str(n.name) }.to_string();
        let command = n.builtin.map(Builtin::name).or_else(|| (n.cmd != NONE).then(|| self.tree.commands[n.cmd as usize].name())).or_else(|| (n.asks != NONE).then(|| A::Cmd::entries()[n.asks as usize].name));
        let acts = command.is_some() || n.host != NONE;
        if !acts {
            let what = n.kind.role();
            let said = if name.is_empty() { format!("This is a {what}, with no command") } else { format!("This is a {what}, “{name}”, with no command") };
            return self.say(Message::plain(said));
        }
        let mut parts = vec![name];
        let keys = match (self.tree.str(n.keys), command) {
            ("", Some(c)) => self.keymap.keys_for(self.mode, c).or_else(|| Builtin::find(c).map(|b| self.builtin_keys(b))).unwrap_or_default(),
            (keys, _) => keys.to_string(),
        };
        parts.push(if keys.is_empty() { "No keys".into() } else { format!("Keys: {keys}") });
        if let Some(doc) = command.and_then(|c| A::Cmd::entries().iter().find(|e| e.name == c)).map(|e| e.doc).filter(|d| !d.is_empty()) {
            parts.push(doc.trim_end_matches('.').to_string());
        }
        parts.push(match self.tree.str(n.reason) {
            _ if n.enabled => "It can run now".into(),
            "" => "It can\u{2019}t run now".into(),
            why => format!("It can\u{2019}t run now: {why}"),
        });
        self.say(Message::plain(parts.join(". ")))
    }

    fn build_palette(&mut self, root: u32, here: &Here) {
        let Some(palette) = self.palette.clone() else { return };
        let p = self.tree.add(root, Kind::Palette, Some(Id::ROOT.mix(7)), std::panic::Location::caller());
        let f = self.tree.add(p, Kind::Field, Some(PALETTE), std::panic::Location::caller());
        self.tree.nodes[f as usize].text = self.tree.put(&palette.query);
        self.tree.nodes[f as usize].span = crate::tree::Fill;
        if let Some(asking) = &palette.asking {
            let entry = A::Cmd::entries()[asking.entry];
            let Some(arg) = entry.args.get(asking.got.len()) else { return };
            let wanted = if arg.name.is_empty() { arg.kind.to_string() } else { format!("{}, {}", arg.name, arg.kind) };
            let asked = self.tree.put(&format!("{}: {wanted}", title(entry.name)));
            self.tree.nodes[f as usize].name = asked;
            let t = self.tree.add(p, Kind::Text { mono: false, tone: crate::tree::Tone::Muted, strong: false }, Some(Id::ROOT.mix(11)), std::panic::Location::caller());
            self.tree.nodes[t as usize].text = asked;
            return;
        }
        self.tree.nodes[f as usize].name = self.tree.put("Type to find a command");
        let mut found: Vec<(String, Target<A::Cmd>, String)> = Vec::new();
        for (e, entry) in A::Cmd::entries().iter().enumerate() {
            if entry.internal {
                continue;
            }
            let target = self.target(e, here);
            let label = match &target {
                Target::One(c) | Target::Refused(Some(c), _) => c.label_string(&self.facts, Form::Long),
                Target::Many(cs, count) => cs.first().map(|c| c.label_string(&self.facts, Form::Many(*count))).unwrap_or_default(),
                Target::Ask(_) => format!("{}…", title(entry.name)),
                Target::Refused(None, _) | Target::Builtin(_) | Target::Host(_) => title(entry.name),
            };
            let keys = self.keymap.keys_for(self.mode, entry.name).unwrap_or_default();
            found.push((label, target, keys));
        }
        for (e, extra) in self.extras.iter().enumerate() {
            found.push((extra.label.clone(), Target::Host(e), String::new()));
        }
        for b in Builtin::ALL {
            if b != Builtin::Palette {
                found.push((b.label().to_string(), Target::Builtin(b), self.builtin_keys(b)));
            }
        }
        found.retain(|(label, ..)| matches(&palette.query, label));
        found.truncate(CHOICES);
        let chosen = palette.chosen.min(found.len().saturating_sub(1));
        for (k, (label, target, keys)) in found.into_iter().enumerate() {
            let c = self.tree.add(p, Kind::Choice { chosen: k == chosen }, Some(Id::ROOT.mix(8).mix(k as u64)), std::panic::Location::caller());
            self.tree.nodes[c as usize].keys = self.tree.put(&keys);
            self.tree.nodes[c as usize].span = crate::tree::Fill;
            self.put_target(c, target, &label);
        }
        if let Some(open) = &mut self.palette {
            open.shown = self.tree.children(p).count() - 1;
            open.chosen = chosen;
        }
    }

    fn put_target(&mut self, node: u32, target: Target<A::Cmd>, fallback: &str) {
        match target {
            Target::Builtin(b) => self.tree.nodes[node as usize].builtin = Some(b),
            Target::Host(e) => {
                let extra = &self.extras[e];
                let (label, available) = (extra.label.clone(), extra.available.clone());
                let r = self.tree.put(&label);
                let n = &mut self.tree.nodes[node as usize];
                (n.host, n.text, n.name) = (e as u32, r, r);
                if let Available::No(reason) = available {
                    let r = self.tree.put(reason.as_str());
                    let n = &mut self.tree.nodes[node as usize];
                    (n.reason, n.enabled) = (r, false);
                }
            }
            Target::One(cmd) => self.push_command(node, cmd),
            Target::Ask(e) => {
                let r = self.tree.put(&format!("{}…", title(A::Cmd::entries()[e].name)));
                let n = &mut self.tree.nodes[node as usize];
                (n.asks, n.text, n.name) = (e as u32, r, r);
            }
            Target::Many(mut cmds, count) => {
                self.tree.nodes[node as usize].many = count as u32;
                if !cmds.is_empty() {
                    self.push_command(node, cmds.swap_remove(0));
                }
            }
            Target::Refused(cmd, reason) => {
                match cmd {
                    Some(cmd) => self.push_command(node, cmd),
                    None => {
                        let r = self.tree.put(&title(fallback));
                        self.tree.nodes[node as usize].text = r;
                        self.tree.nodes[node as usize].name = r;
                    }
                }
                let r = self.tree.put(reason.as_str());
                let n = &mut self.tree.nodes[node as usize];
                n.reason = r;
                n.enabled = false;
            }
        }
    }

    fn push_command(&mut self, node: u32, cmd: A::Cmd) {
        self.tree.nodes[node as usize].cmd = self.tree.commands.len() as u32;
        self.tree.commands.push(cmd);
    }

    fn resolve(&mut self) {
        for i in 0..self.tree.nodes.len() {
            let n = self.tree.nodes[i];
            if let Some(b) = n.builtin {
                let label = match (b, self.undo_label(), self.history.redo_label()) {
                    (Builtin::Undo, Some(step), _) => format!("Undo {step}"),
                    (Builtin::Redo, _, Some(step)) => format!("Redo {step}"),
                    _ => b.label().to_string(),
                };
                let r = self.tree.put(&label);
                self.tree.nodes[i].name = r;
                self.tree.nodes[i].text = r;
                if let Available::No(reason) = self.builtin_available(b) {
                    let r = self.tree.put(reason.as_str());
                    self.tree.nodes[i].reason = r;
                    self.tree.nodes[i].enabled = false;
                }
                continue;
            }
            if n.cmd == NONE {
                continue;
            }
            let short = matches!(n.kind, Kind::Button | Kind::Link) && self.tree.in_row(i as u32);
            let cmd = &self.tree.commands[n.cmd as usize];
            let available = if n.many > 0 { Available::Yes } else { self.facts.available(cmd) };
            let long = if n.many > 0 { Form::Many(n.many as usize) } else { Form::Long };
            let name = label(&mut self.tree.text, cmd, &self.facts, long);
            let text = if short { label(&mut self.tree.text, cmd, &self.facts, Form::Short) } else { name };
            let node = &mut self.tree.nodes[i];
            node.name = name;
            if !matches!(n.kind, Kind::Check { .. }) {
                node.text = text;
            }
            if let Available::No(reason) = available {
                node.enabled = false;
                let r = self.tree.put(reason.as_str());
                self.tree.nodes[i].reason = r;
            }
        }
        for i in 0..self.tree.nodes.len() {
            self.tree.hash_node(i);
        }
    }

    fn status(&mut self) {
        if self.status == NONE {
            return;
        }
        let chosen = || self.tree.nodes.iter().position(|n| n.kind == Kind::Choice { chosen: true }).map(|i| i as u32);
        let at = match self.hover.and_then(|id| self.tree.find(id)) {
            Some(i) => Some(i),
            None if self.palette.is_some() => chosen(),
            None => self.focus.and_then(|id| self.tree.find(id)),
        };
        let range = if !self.pending.is_empty() {
            let typed = keys::label(&self.pending);
            self.tree.put(&format!("{typed} …"))
        } else {
            match at.map(|i| self.tree.nodes[i as usize]) {
                Some(n) if !n.name.is_empty() => {
                    let (name, reason) = (n.name, n.reason);
                    let start = self.tree.text.len() as u32;
                    let label = self.tree.str(name).to_string();
                    let why = self.tree.str(reason).to_string();
                    let _ = if why.is_empty() { write!(self.tree.text, "{label}") } else { write!(self.tree.text, "{label}: {why}") };
                    Range32 { start, end: self.tree.text.len() as u32 }
                }
                _ => Range32::default(),
            }
        };
        let range = match self.progress() {
            Some(progress) => {
                let start = self.tree.text.len() as u32;
                let after = self.tree.str(range).to_string();
                let _ = if after.is_empty() { write!(self.tree.text, "{progress}") } else { write!(self.tree.text, "{progress} · {after}") };
                Range32 { start, end: self.tree.text.len() as u32 }
            }
            None => range,
        };
        self.tree.nodes[self.status as usize].text = range;
        self.tree.hash_node(self.status as usize);
    }

    fn layout(&mut self, p: &mut Painter) {
        for _ in 0..4 {
            let mut layout = Layout::new(p.text, p.m, std::mem::take(&mut self.stack), std::mem::take(&mut self.widths));
            layout.run(&mut self.tree);
            self.minimum = Some(layout.minimum(&self.tree));
            (self.stack, self.widths) = layout.finish();
            if !self.settle() {
                break;
            }
            self.build();
        }
        self.reveal = false;
        self.hits.fill(&self.tree, p.m.cols(), p.m.rows());
        self.laid = true;
    }

    fn settle(&mut self) -> bool {
        let mut moved = false;
        for l in 0..self.tree.lists.len() {
            let info = self.tree.lists[l];
            let n = self.tree.nodes[info.node as usize];
            let Kind::List { first, .. } = n.kind else { continue };
            let top = self.tree.children(info.node).find_map(|c| match self.tree.nodes[c as usize].kind {
                Kind::ListRow { key, .. } => Some(key),
                _ => None,
            });
            let visible = self.tree.children(info.node).filter(|&c| {
                let a = self.tree.nodes[c as usize].area;
                a.rows > 0 && a.row + a.rows <= n.area.row + n.area.rows
            });
            let visible = visible.count().max(1) as u32;
            let s = self.scrolls.entry(n.id).or_default();
            if s.rows != n.area.rows {
                s.rows = n.area.rows;
                moved = true;
            }
            s.anchor = top;
            s.index = first;
            if let Some(f) = info.focus_index.filter(|_| self.reveal) {
                let to = if f < first {
                    Some(f)
                } else if f >= first + visible {
                    Some(f + 1 - visible)
                } else {
                    None
                };
                if let Some(to) = to {
                    *s = Scroll { anchor: None, index: to, ..*s };
                    moved = true;
                }
            }
        }
        moved
    }

    fn look(&self) -> impl Fn(Id) -> Look + use<A> {
        let (hover, pressed) = (self.hover, self.pressed);
        let focus = if self.palette.is_some() { Some(PALETTE) } else { self.focus };
        let composed = self.composed;
        let chosen: HashMap<Id, (u32, u32)> = self.chosen().into_iter().map(|(i, r)| (self.tree.nodes[i as usize].id, (r.start as u32, r.end as u32))).collect();
        let caret = match &self.palette {
            Some(p) => Some((PALETTE, p.caret)),
            None => self.focus.and_then(|f| self.carets.get(&f).map(|c| (f, *c))),
        };
        move |id| {
            let at = caret.filter(|(f, _)| *f == id).map(|(_, c)| c);
            Look {
                hovered: hover == Some(id),
                pressed: pressed == Some(id) && hover == Some(id),
                focused: focus == Some(id),
                caret: at.map_or(u32::MAX, |c| c.caret as u32),
                anchor: at.map_or(u32::MAX, |c| c.anchor as u32),
                composing: if focus == Some(id) { composed } else { Range32::default() },
                chosen: chosen.get(&id).copied().unwrap_or_default(),
            }
        }
    }

    fn here(&self) -> Here {
        let list = self.current_list()?;
        let keys = if list.chosen > 1 { (list.keys)(&self.facts as &dyn Any, list.items, list.selection, None) } else { list.focused.into_iter().collect() };
        Some((list, keys))
    }

    fn glance(&self) -> Here {
        let list = self.current_list()?;
        match list.focused.or(list.anchor) {
            Some(one) => Some((list, vec![one])),
            None => self.here(),
        }
    }

    fn current_list(&self) -> Option<ListInfo> {
        let from_focus = self.focus.and_then(|id| self.tree.find(id)).and_then(|i| self.tree.ancestor(i, |n| matches!(n.kind, Kind::List { .. })));
        let l = match from_focus {
            Some(i) => self.tree.nodes[i as usize].list,
            None if !self.tree.lists.is_empty() => 0,
            None => NONE,
        };
        self.tree.lists.get(l as usize).copied()
    }

    fn target(&self, e: usize, here: &Here) -> Target<A::Cmd> {
        self.target_with(e, here, &[])
    }

    fn target_with(&self, e: usize, here: &Here, args: &[Value]) -> Target<A::Cmd> {
        let entry = A::Cmd::entries()[e];
        let asking = entry.args.len() > args.len();
        let make = |k| A::Cmd::make(e, k, args);
        if !entry.subject {
            return match make(None) {
                _ if asking => Target::Ask(e),
                Some(cmd) => Target::One(cmd),
                None => Target::Refused(None, Message::from("Its arguments couldn't be read")),
            };
        }
        let Some((list, subjects)) = here.as_ref().filter(|(l, _)| entry.subject_type.is_none_or(|t| t() == l.item)) else {
            return Target::Refused(None, Message::from("There is nothing here for it to act on"));
        };
        let focused = || list.focused.and_then(|k| make(Some(k)));
        match subjects.as_slice() {
            [] => Target::Refused(None, Message::from("Nothing is chosen")),
            _ if asking => Target::Ask(e),
            _ if self.count > 1 && !entry.many => Target::Refused(focused(), Message::from("It takes one item at a time, so a count can't go with it")),
            _ if self.count > 1 && list.chosen > 1 => Target::Refused(focused(), Message::from("Several items are chosen already, so a count has nothing to add")),
            _ if self.count > 1 => {
                let Some(from) = list.focus_index else { return Target::Refused(None, Message::from("Nothing is chosen")) };
                let cmds: Vec<_> = (from..from.saturating_add(self.count)).map_while(|i| (list.nth)(&self.facts as &dyn Any, list.items, i)).filter_map(|k| make(Some(k))).collect();
                let n = cmds.len();
                Target::Many(cmds, n)
            }
            some if entry.many && list.chosen > 1 => Target::Many(some.iter().filter_map(|k| make(Some(*k))).collect(), list.chosen as usize),
            [one] => make(Some(*one)).map_or(Target::Refused(None, Message::from("Nothing is chosen")), Target::One),
            _ => list.focused.and_then(|k| make(Some(k))).map_or(Target::Refused(None, Message::from("Nothing is chosen")), Target::One),
        }
    }

    fn named(&self, name: &str, here: &Here) -> Target<A::Cmd> {
        if let Some(b) = Builtin::find(name) {
            return Target::Builtin(b);
        }
        if let Some(e) = self.extras.iter().position(|x| x.name == name) {
            return Target::Host(e);
        }
        match A::Cmd::entries().iter().position(|e| e.name == name && !e.internal) {
            Some(e) => self.target(e, here),
            None => Target::Refused(None, Message::plain(format!("No command is called “{name}”"))),
        }
    }

    fn builtin_keys(&self, b: Builtin) -> String {
        match self.keymap.keys_for(self.mode, b.name()) {
            Some(k) => k,
            None => b.convention().first().map(Chord::to_string).unwrap_or_default(),
        }
    }

    fn builtin_available(&self, b: Builtin) -> Available {
        match b {
            Builtin::Undo if self.undo_label().is_none() => Available::No(Message::from("Nothing to undo")),
            Builtin::Copy if self.chosen().is_empty() => Available::No(Message::from("No text is chosen: drag across text to choose some")),
            Builtin::Undo | Builtin::Redo if self.turning.is_some() => Available::No(Message::from("Wait for the change in progress to finish")),
            Builtin::Redo if !self.background.is_empty() => Available::No(Message::from("Wait for the change in progress to finish")),
            Builtin::Redo if self.history.redo_label().is_none() => Available::No(Message::from("Nothing to redo")),
            Builtin::Up | Builtin::Down | Builtin::ExtendUp | Builtin::ExtendDown | Builtin::First | Builtin::Last | Builtin::SelectAll if self.tree.lists.is_empty() => {
                Available::No(Message::from("There is no list here"))
            }
            Builtin::LightAppearance if self.offered.appearance == Choice::Light => Available::No(Message::from("The light appearance is in use already")),
            Builtin::DarkAppearance if self.offered.appearance == Choice::Dark => Available::No(Message::from("The dark appearance is in use already")),
            Builtin::SystemAppearance if self.offered.appearance == Choice::System => Available::No(Message::from("The system's appearance is followed already")),
            Builtin::NextTheme if self.offered.themes < 2 => Available::No(Message::from("There is only one theme")),
            Builtin::NextStyle if self.offered.styles < 2 => Available::No(Message::from("There is only one style")),
            Builtin::AbsoluteDates if self.offered.dates == Dates::Absolute => Available::No(Message::from("Dates are written as the day and time already")),
            Builtin::RelativeDates if self.offered.dates == Dates::Relative => Available::No(Message::from("Dates are written as how long ago already")),
            Builtin::ChangedSettings if self.only_changed => Available::No(Message::from("Only the changed settings show already")),
            Builtin::AllSettings if !self.only_changed => Available::No(Message::from("Every setting shows already")),
            Builtin::EditKeys if self.keys_file.is_none() => Available::No(Message::from("This window keeps no files, so its keys can't be edited")),
            Builtin::NormalMode | Builtin::InsertMode | Builtin::SelectMode if !self.keymap.modal => Available::No(Message::from("These keys have no modes")),
            Builtin::NormalMode | Builtin::InsertMode | Builtin::SelectMode if b.mode() == Some(self.mode) => Available::No(Message::plain(format!("In {} mode already", self.mode.name()))),
            _ => Available::Yes,
        }
    }

    /// What went wrong in the last command, if it failed.
    pub fn failure(&self) -> Option<&str> {
        match &self.said {
            Some(Said::Failure(f)) => Some(f),
            _ => None,
        }
    }

    /// What the message line says after the last command: its note or its failure, if it said anything.
    pub fn said(&self) -> Option<&Said> {
        self.said.as_ref()
    }

    fn say(&mut self, message: Message) -> bool {
        self.said = Some(Said::Note(message));
        self.stale = true;
        true
    }

    /// Runs a command the way every entry point does: only if it is available, inside a transaction. Says why when it can't, and returns whether it ran.
    pub fn run(&mut self, cmd: A::Cmd) -> bool {
        if let Available::No(reason) = self.facts.available(&cmd) {
            self.say(reason);
            return false;
        }
        self.transact(vec![cmd], None, 0);
        true
    }

    fn repeat(&mut self, cmd: A::Cmd, times: u32) -> bool {
        if let Available::No(reason) = self.facts.available(&cmd) {
            return self.say(reason);
        }
        self.transact(vec![cmd; times as usize], None, 0);
        true
    }

    fn builtin_times(&mut self, b: Builtin, times: u32) -> bool {
        let mut ran = self.builtin(b);
        for _ in 1..if b.repeats() { times } else { 1 } {
            if !self.builtin_available(b).is_yes() {
                break;
            }
            ran |= self.builtin(b);
        }
        ran
    }

    /// Runs one command over several subjects as one transaction. Subjects it can't take are skipped, and the message line says which and why.
    pub fn run_many(&mut self, cmds: Vec<A::Cmd>) -> bool {
        let mut skipped: Vec<(String, Message)> = Vec::new();
        let mut take = Vec::new();
        for cmd in cmds {
            match self.facts.available(&cmd) {
                Available::No(reason) => skipped.push((cmd.label_string(&self.facts, Form::Long), reason)),
                Available::Yes => take.push(cmd),
            }
        }
        if !take.is_empty() {
            self.transact(take, None, 0);
        }
        match skipped.first() {
            Some((what, why)) if skipped.len() == 1 => self.said = Some(Said::Note(Message::plain(format!("{what} was skipped: {}", why.as_str())))),
            Some((what, why)) => self.said = Some(Said::Note(Message::plain(format!("{} were skipped, among them {what}: {}", skipped.len(), why.as_str())))),
            None => {}
        }
        self.stale = true;
        true
    }

    fn transact(&mut self, cmds: Vec<A::Cmd>, clock: Option<Clock>, more: usize) {
        self.turn(Slice::Items(usize::MAX), None);
        let many = cmds.len() + more;
        let repeated = many > 1 && cmds.windows(2).all(|w| ledger::Field::value(&w[0]) == ledger::Field::value(&w[1]));
        let named = self.naming.take();
        let (label, gesture) = match cmds.first().and_then(Commands::as_edit) {
            _ if let Some(named) = named => {
                self.gesture += 1;
                (format!("“{named}”"), self.gesture)
            }
            Some(edit) if edit.is_typing() => ("typing".to_string(), edit.gesture),
            _ if repeated => {
                self.gesture += 1;
                (cmds.first().map_or_else(String::new, |c| format!("“{}” {many} times", c.label_string(&self.facts, Form::Long))), self.gesture)
            }
            _ => {
                self.gesture += 1;
                let form = if many > 1 { Form::Many(many) } else { Form::Long };
                (cmds.first().map_or_else(String::new, |c| format!("“{}”", c.label_string(&self.facts, form))), self.gesture)
            }
        };
        let clock = clock.unwrap_or_else(|| self.clock());
        let sliced = !self.replaying && self.waker.is_some() && many > 1 && !repeated;
        let mut queue: VecDeque<A::Cmd> = cmds.into();
        let marks = self.marks();
        let place = self.place();
        let batch = self.batch(&mut queue, &clock, sliced.then_some(Slice::Time(SLICE)), repeated, false);
        if let Some(why) = batch.failed {
            self.said = Some(Said::Failure(format!("{label} failed, and nothing changed: {why}")));
            return;
        }
        let left = queue.len() + more;
        let mut entry = vec![("do".into(), Value::Str("command".into())), ("commands".into(), Value::List(batch.ran))];
        entry.extend(clock.fields());
        if left > 0 {
            entry.push(("more".into(), Value::UInt(left as u64)));
        }
        let stopped = batch.stopped;
        let ops = self.conclude(&label, batch.ops, batch.cx, batch.changed);
        if let Some((done, reason)) = stopped {
            self.said = Some(Said::Note(Message::plain(format!("{label} stopped after {done}: {}", reason.as_str()))));
        }
        if !ops.is_empty() || !batch.deferred.is_empty() || left > 0 {
            self.record(Value::Map(entry), &ops);
        }
        if !batch.deferred.is_empty() || left > 0 {
            let parts = batch.deferred.into_iter().map(|bulk| self.search(bulk)).collect();
            self.background.push_back(Background { label, gesture, place, ops, parts, rest: queue, left });
            return;
        }
        let what = done_as(&label);
        if self.history.commit(ops, || label, gesture, place) == ledger::Kept::Step {
            self.did(what);
        }
        self.repair(marks);
    }

    fn clock(&self) -> Clock {
        Clock { replica: self.replica, counter: self.counter, now: Moment::now(), seed: self.counter ^ 0x9e37_79b9_7f4a_7c15 }
    }

    fn batch(&mut self, queue: &mut VecDeque<A::Cmd>, clock: &Clock, slice: Option<Slice>, repeated: bool, checked: bool) -> Batch<A::Cmd> {
        let mut cx = Cx::new(clock.replica, clock.counter, clock.now, clock.seed);
        let before = self.facts.snap();
        let _ = ledger::take_replaced();
        let started = Instant::now();
        let tx = Transaction::begin();
        let (mut ran, mut changed, mut stopped) = (Vec::new(), false, None);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            while let Some(cmd) = queue.pop_front() {
                let full = match slice {
                    Some(Slice::Time(budget)) => started.elapsed() >= budget,
                    Some(Slice::Items(most)) => ran.len() >= most,
                    None => false,
                };
                if full && !ran.is_empty() {
                    queue.push_front(cmd);
                    break;
                }
                if (repeated || checked)
                    && let Available::No(reason) = self.facts.available(&cmd)
                {
                    if repeated {
                        stopped = Some((ran.len(), reason));
                        break;
                    }
                    continue;
                }
                ran.push(ledger::Field::value(&cmd));
                changed |= self.facts.update(cmd, &mut cx).changed();
            }
        }));
        let deferred = ledger::deferred();
        let mut ops = tx.end();
        self.facts.diff(&before, ErasedPath::ROOT, &mut ops);
        self.stale = true;
        let failed = outcome.err().map(|panic| {
            for op in ops.iter().rev() {
                let inverse = op.inverse();
                self.facts.apply(inverse.path().steps(), &inverse);
            }
            panic.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| panic.downcast_ref::<String>().cloned()).unwrap_or_default()
        });
        Batch { ran, ops, deferred, cx, changed, stopped, failed }
    }

    fn conclude(&mut self, label: &str, mut ops: Vec<Op>, mut cx: Cx<A::Cmd>, changed: bool) -> Vec<Op> {
        self.counter = cx.counter;
        self.said = cx.said.pop();
        if self.hold && !cx.effects.is_empty() {
            let asked: Vec<String> = cx.effects.iter().map(|e| format!("{e:?}")).collect();
            self.said = Some(Said::Note(Message::plain(format!("Specimen didn't run what the app asked for: {}", asked.join(", then ")))));
        } else if !self.replaying {
            self.effects.append(&mut cx.effects);
        }
        if cfg!(debug_assertions) && !changed && !ops.is_empty() {
            self.said = Some(Said::Failure(format!("{label} returned Outcome::Nothing but changed the facts")));
        }
        let replaced = ledger::take_replaced();
        if !replaced.is_empty() {
            ledger::Field::attach(&mut self.facts, ErasedPath::ROOT);
            let names: Vec<String> = replaced.iter().map(|t| shortened(t)).collect();
            self.said =
                Some(Said::Failure(format!("{label} replaced a {} by assigning a new one, which undo can't take back; change it with its own methods, like replace or set", names.join(" and a "))));
        }
        let pruned = self.prune(&ops);
        ops.extend(pruned);
        ops
    }

    fn more(&mut self, given: Option<(Vec<A::Cmd>, Clock, usize)>, slice: Slice) -> bool {
        let now = self.clock();
        let Some(background) = self.background.front_mut() else { return false };
        let (mut queue, clock, slice, left) = match given {
            Some((cmds, clock, left)) => (cmds.into(), clock, None, Some(left)),
            None if !background.rest.is_empty() => (std::mem::take(&mut background.rest), now, Some(slice), None),
            None => return false,
        };
        let label = background.label.clone();
        let checked = left.is_none();
        let batch = self.batch(&mut queue, &clock, slice, false, checked);
        if let Some(why) = batch.failed {
            self.revert();
            self.said = Some(Said::Failure(format!("{label} failed partway, and nothing changed: {why}")));
            return true;
        }
        let left = left.unwrap_or(queue.len());
        let mut entry = vec![("do".into(), Value::Str("more".into())), ("commands".into(), Value::List(batch.ran))];
        entry.extend(clock.fields());
        entry.push(("left".into(), Value::UInt(left as u64)));
        let ops = self.conclude(&label, batch.ops, batch.cx, batch.changed);
        self.record(Value::Map(entry), &ops);
        let parts: Vec<Part> = batch.deferred.into_iter().map(|bulk| self.search(bulk)).collect();
        let Some(background) = self.background.front_mut() else { return true };
        background.ops.extend(ops);
        background.parts.extend(parts);
        background.rest = queue;
        background.left = left;
        self.land();
        true
    }

    fn record(&mut self, entry: Value, ops: &[Op]) {
        for op in ops {
            if let Some(field) = op.path().first_field() {
                *self.versions.entry(field).or_default() += 1;
            }
        }
        match &mut self.keep {
            Keep::Nothing => {}
            Keep::Memory(entries) => entries.push(entry),
            Keep::Store(store) => {
                store.record(entry, ops.to_vec(), self.counter);
                if let Some(trouble) = store.trouble() {
                    self.said = Some(Said::Failure(trouble));
                }
            }
        }
    }

    /// Keeps every entry of the command log in memory, for replaying it.
    pub fn remember(&mut self) {
        self.keep = Keep::Memory(Vec::new());
    }

    /// The command log kept in memory since `remember`.
    pub fn entries(&self) -> &[Value] {
        match &self.keep {
            Keep::Memory(entries) => entries,
            _ => &[],
        }
    }

    /// Runs a command log again, entry by entry, with the keys, times and random numbers each command was given the first time.
    pub fn replay(&mut self, entries: &[Value]) -> bool {
        let repairing = std::mem::replace(&mut self.repairing, true);
        let replaying = std::mem::replace(&mut self.replaying, true);
        let mut whole = true;
        for entry in entries {
            match entry.field("do") {
                Some(Value::Str(what)) if what == "undo" || what == "redo" => whole &= self.undo(what == "undo"),
                Some(Value::Str(what)) if what == "stop" => whole &= self.stop(),
                Some(Value::Str(what)) if what == "turn" || what == "failed" => {}
                Some(Value::Str(what)) if what == "more" => {
                    let cmds = match entry.field("commands") {
                        Some(Value::List(cmds)) => cmds.iter().map(<A::Cmd as ledger::Field>::from_value).collect::<Option<Vec<_>>>(),
                        _ => None,
                    };
                    match (cmds, Clock::of(entry), entry.field("left")) {
                        (Some(cmds), Some(clock), Some(Value::UInt(left))) => whole &= self.more(Some((cmds, clock, *left as usize)), Slice::Items(usize::MAX)),
                        _ => whole = false,
                    }
                }
                Some(Value::Str(what)) if what == "slice" => match entry.field("count") {
                    Some(Value::UInt(count)) => whole &= self.slice(Slice::Items(*count as usize)),
                    _ => whole = false,
                },
                Some(Value::Str(what)) if what == "command" => {
                    let clock = Clock::of(entry);
                    let cmds = match entry.field("commands") {
                        Some(Value::List(cmds)) => cmds.iter().map(<A::Cmd as ledger::Field>::from_value).collect::<Option<Vec<_>>>(),
                        _ => None,
                    };
                    match (cmds, clock) {
                        (Some(cmds), Some(clock)) => {
                            let more = match entry.field("more") {
                                Some(Value::UInt(more)) => *more as usize,
                                _ => 0,
                            };
                            self.transact(cmds, Some(clock), more)
                        }
                        _ => whole = false,
                    }
                }
                _ => whole = false,
            }
        }
        self.repairing = repairing;
        self.replaying = replaying;
        whole
    }

    fn place(&self) -> Place {
        Place { focus: self.focus, caret: self.focus.and_then(|f| self.carets.get(&f).copied()) }
    }

    fn restore(&mut self, place: Place) {
        self.focus = place.focus;
        self.typing = None;
        if let (Some(focus), Some(caret)) = (place.focus, place.caret) {
            self.carets.insert(focus, caret);
        }
    }

    fn prune(&mut self, ops: &[Op]) -> Vec<Op> {
        let inserted: std::collections::HashSet<u128> = ops
            .iter()
            .filter_map(|op| match op {
                Op::Insert { key, .. } => Some(*key),
                _ => None,
            })
            .collect();
        let deleted: Vec<(std::any::TypeId, u128)> =
            ops.iter().filter_map(|op| op.deleted().filter(|k| !inserted.contains(k)).and_then(|k| Some((self.facts.item_type(op.path().steps())?, k)))).collect();
        if deleted.is_empty() {
            return Vec::new();
        }
        let tx = Transaction::begin();
        self.facts.prune(&deleted);
        tx.end()
    }

    fn undo(&mut self, back: bool) -> bool {
        if back && !self.background.is_empty() {
            return self.stop();
        }
        if self.turning.is_some() || (!back && !self.background.is_empty()) {
            return false;
        }
        let step = if back { self.history.undo_label() } else { self.history.redo_label() }.map(str::to_string).unwrap_or_default();
        let place = self.place();
        let turn = if back { self.history.undo(place) } else { self.history.redo() };
        let Some(turn) = turn else { return false };
        self.did(format!("{} {step}", if back { "Undid" } else { "Redid" }));
        let label = format!("{} {step}", if back { "Undoing" } else { "Redoing" });
        self.turning = Some(Turning { label, ops: turn.ops, next: 0, place: turn.place });
        self.said = None;
        let slice = if self.waker.is_none() || self.replaying { Slice::Items(usize::MAX) } else { Slice::Time(SLICE) };
        self.turn(slice, Some(Value::Map(vec![("do".into(), Value::Str(if back { "undo" } else { "redo" }.into()))])));
        true
    }

    fn turn(&mut self, slice: Slice, entry: Option<Value>) -> bool {
        let Some(turning) = &mut self.turning else { return false };
        let (started, from) = (Instant::now(), turning.next);
        while turning.next < turning.ops.len() {
            match slice {
                Slice::Time(budget) if turning.next > from && started.elapsed() >= budget => break,
                Slice::Items(most) if turning.next - from >= most => break,
                _ => {}
            }
            let op = &turning.ops[turning.next];
            self.facts.apply(op.path().steps(), op);
            turning.next += 1;
        }
        let finished = turning.next >= turning.ops.len();
        let mut ops = turning.ops[from..turning.next].to_vec();
        let pruned = self.prune(&ops);
        ops.extend(pruned);
        self.record(entry.unwrap_or_else(|| Value::Map(vec![("do".into(), Value::Str("turn".into()))])), &ops);
        if finished && let Some(done) = self.turning.take() {
            self.restore(done.place);
        }
        self.stale = true;
        true
    }

    /// Where the user is, as the store keeps it: the focused component, its caret and where each list is scrolled.
    pub fn whereabouts(&self) -> Value {
        let caret = self.place().caret.map_or(Value::None, |c| Value::List(vec![Value::UInt(c.caret as u64), Value::UInt(c.anchor as u64)]));
        let scrolls = self.scrolls.iter().map(|(id, s)| Value::List(vec![Value::UInt(id.0), s.anchor.map_or(Value::None, Value::Key), Value::UInt(s.index as u64)]));
        Value::Map(vec![("focus".into(), self.focus.map_or(Value::None, |f| Value::UInt(f.0))), ("caret".into(), caret), ("scrolls".into(), Value::List(scrolls.collect()))])
    }

    /// Puts the user back where `whereabouts` said they were.
    pub fn return_to(&mut self, place: &Value) {
        if let Some(Value::UInt(focus)) = place.field("focus") {
            self.focus = Some(Id(*focus));
            if let Some(Value::List(c)) = place.field("caret")
                && let [Value::UInt(caret), Value::UInt(anchor)] = c.as_slice()
            {
                self.carets.insert(Id(*focus), Caret { caret: *caret as usize, anchor: *anchor as usize });
            }
        }
        if let Some(Value::List(scrolls)) = place.field("scrolls") {
            for s in scrolls {
                if let Value::List(parts) = s
                    && let [Value::UInt(id), anchor, Value::UInt(index)] = parts.as_slice()
                {
                    let anchor = match anchor {
                        Value::Key(k) => Some(*k),
                        _ => None,
                    };
                    self.scrolls.insert(Id(*id), Scroll { anchor, index: *index as u32, rows: 0 });
                }
            }
        }
        self.stale = true;
    }

    /// Saves into `store` from now on, and goes on from where its facts were opened.
    pub fn save_to(&mut self, store: Store, counter: u64) {
        self.replica = store.replica;
        self.counter = self.counter.max(counter);
        self.keep = Keep::Store(store);
    }

    fn search(&self, mut bulk: ledger::Bulk) -> Part {
        let Some(search) = bulk.search() else { return Part { bulk, found: Found::Ready(Vec::new()), next: 0 } };
        if self.replaying {
            return Part { bulk, found: Found::Ready(search()), next: 0 };
        }
        let (send, receive) = mpsc::channel();
        let waker = self.waker.clone();
        let started = std::thread::Builder::new().name("graticule job".into()).spawn(move || {
            let _ = send.send(search());
            if let Some(waker) = waker {
                waker.wake();
            }
        });
        match started {
            Ok(_) => Part { bulk, found: Found::Searching(receive), next: 0 },
            Err(_) => Part { bulk, found: Found::Ready(Vec::new()), next: 0 },
        }
    }

    /// Applies the next slice of the oldest bulk change still landing, and returns whether anything moved. When its last slice lands, the whole change becomes one undo step.
    pub fn slice(&mut self, slice: Slice) -> bool {
        if self.turning.is_some() {
            return self.turn(slice, None);
        }
        if self.background.front().is_some_and(|b| !b.rest.is_empty()) {
            return self.more(None, slice);
        }
        let Some(background) = self.background.front_mut() else { return false };
        let Some(part) = background.parts.front_mut() else { return false };
        if let Found::Searching(receive) = &part.found {
            let got = match slice {
                Slice::Time(_) => receive.try_recv().map_err(|e| e == mpsc::TryRecvError::Disconnected),
                Slice::Items(_) => receive.recv().map_err(|_| true),
            };
            match got {
                Ok(keys) => part.found = Found::Ready(keys),
                Err(false) => return false,
                Err(true) => {
                    part.found = Found::Ready(Vec::new());
                    self.said = Some(Said::Failure("A bulk change couldn't search the list, so it changed nothing more".into()));
                }
            }
        }
        let Found::Ready(keys) = &part.found else { return false };
        if part.next == 0 {
            background.ops.reserve(keys.len());
        }
        let started = Instant::now();
        let tx = Transaction::begin();
        let mut count = 0;
        match ledger::Field::reach_mut(&mut self.facts, part.bulk.list.steps()) {
            Some(list) => {
                while part.next < keys.len() {
                    match slice {
                        Slice::Time(budget) if started.elapsed() >= budget => break,
                        Slice::Items(most) if count >= most => break,
                        _ => {}
                    }
                    part.bulk.change(list, keys[part.next]);
                    part.next += 1;
                    count += 1;
                }
            }
            None => part.next = keys.len(),
        }
        let finished = part.next >= keys.len();
        let mut ops = tx.end();
        if count == 0 && !finished {
            return false;
        }
        let pruned = self.prune(&ops);
        ops.extend(pruned);
        self.record(Value::Map(vec![("do".into(), Value::Str("slice".into())), ("count".into(), Value::UInt(count as u64))]), &ops);
        self.stale = true;
        let Some(background) = self.background.front_mut() else { return true };
        background.ops.extend(ops);
        if finished {
            background.parts.pop_front();
        }
        self.land();
        true
    }

    fn land(&mut self) {
        if self.background.front().is_some_and(|b| b.parts.is_empty() && b.rest.is_empty() && b.left == 0)
            && let Some(done) = self.background.pop_front()
        {
            let what = done_as(&done.label);
            if self.history.commit(done.ops, || done.label, done.gesture, done.place) == ledger::Kept::Step {
                self.did(what);
            }
        }
    }

    /// Lands every bulk change still running, waiting for their searches.
    pub fn finish(&mut self) {
        self.turn(Slice::Items(usize::MAX), None);
        while self.slice(Slice::Items(usize::MAX)) {}
    }

    fn stop(&mut self) -> bool {
        let Some(background) = self.background.pop_back() else { return false };
        self.take_back(background, "stop");
        true
    }

    fn revert(&mut self) {
        if let Some(background) = self.background.pop_front() {
            self.take_back(background, "failed");
        }
    }

    fn take_back(&mut self, background: Background<A::Cmd>, why: &str) {
        let tx = Transaction::begin();
        for op in background.ops.iter().rev() {
            let inverse = op.inverse();
            self.facts.apply(inverse.path().steps(), &inverse);
        }
        let ops = tx.end();
        self.record(Value::Map(vec![("do".into(), Value::Str(why.into()))]), &ops);
        self.restore(background.place);
        self.said = Some(Said::Note(Message::plain(format!("{} was stopped, and nothing changed", background.label))));
        self.stale = true;
    }

    fn undo_label(&self) -> Option<&str> {
        match self.background.back() {
            Some(background) => Some(&background.label),
            None => self.history.undo_label(),
        }
    }

    fn progress(&self) -> Option<String> {
        if let Some(turning) = &self.turning {
            return Some(format!("{}: {} of {}", turning.label, turning.next, turning.ops.len()));
        }
        let working = || A::derivations().iter().find(|d| self.working.contains(&d.field)).map(|d| format!("Working out “{}”", d.name));
        let Some(background) = self.background.front() else { return self.jobs.first().map(|job| job.name.clone()).or_else(working) };
        if background.left > 0 {
            return Some(format!("{}: {} to go", background.label, background.left));
        }
        let part = background.parts.front()?;
        Some(match &part.found {
            Found::Searching(_) => format!("{}: searching", background.label),
            Found::Ready(keys) => format!("{}: {} of {}", background.label, part.next, keys.len()),
        })
    }

    fn pin(&mut self) -> bool {
        let mut moved = Vec::new();
        self.facts.moving(&mut |field, value| {
            if value.pin() {
                moved.push(field);
            }
        });
        let any = !moved.is_empty();
        for field in moved {
            *self.versions.entry(field).or_default() += 1;
        }
        self.derive();
        any
    }

    fn derive(&mut self) {
        for d in A::derivations() {
            let from: Vec<u64> = d.from.iter().map(|f| self.versions.get(f).copied().unwrap_or_default()).collect();
            if self.deriving.get(&d.field) == Some(&from) {
                continue;
            }
            let Some(work) = self.facts.derive_work(d.field) else { continue };
            self.deriving.insert(d.field, from.clone());
            if !self.working.contains(&d.field) {
                self.working.push(d.field);
            }
            self.facts.moving(&mut |field, value| {
                if field == d.field {
                    value.coming(true);
                }
            });
            let send = self.derived.0.clone();
            let field = d.field;
            match self.waker.clone() {
                Some(waker) => {
                    let started = std::thread::Builder::new().name(format!("graticule deriving {}", d.name)).spawn(move || {
                        if send.send((field, from, work())).is_ok() {
                            waker.wake();
                        }
                    });
                    if let Err(e) = started {
                        self.said = Some(Said::Failure(format!("Working out “{}” couldn't start: {e}", d.name)));
                    }
                }
                None => {
                    let _ = send.send((field, from, work()));
                }
            }
        }
        if self.waker.is_none() {
            self.arrive();
        }
    }

    fn arrive(&mut self) {
        let mut landed = false;
        while let Ok((field, from, value)) = self.derived.1.try_recv() {
            if self.deriving.get(&field) != Some(&from) {
                continue;
            }
            let mut value = Some(value);
            self.facts.moving(&mut |f, moving| {
                if f == field
                    && let Some(value) = value.take()
                {
                    moving.arrive(value);
                    moving.coming(false);
                }
            });
            self.working.retain(|w| *w != field);
            *self.versions.entry(field).or_default() += 1;
            landed = true;
        }
        if landed {
            self.stale = true;
            self.derive();
        }
    }

    /// Lets jobs wake the window when their results are ready.
    pub fn wake_with(mut self, waker: Waker) -> Self {
        let wake = waker.clone();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || wake.wake());
        self.facts.moving(&mut |_, value| value.wake_with(wake.clone()));
        self.keys_watch = self.keys_file.as_deref().and_then(|file| watch_keys(file, waker.clone()));
        self.waker = Some(waker);
        self
    }

    fn read_keys(&mut self) {
        let Some(file) = &self.keys_file else { return };
        let read = match std::fs::read_to_string(file) {
            Ok(source) => read_keymap::<A>(&source, KEYMAP),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => read_keymap::<A>(A::Cmd::keymap(), "The keymap"),
            Err(e) => Err(Said::Failure(format!("{KEYMAP} couldn't be read: {e}"))),
        };
        match read {
            Ok((keymap, said)) => {
                self.keymap = keymap;
                if !self.keymap.modal {
                    self.mode = Mode::Normal;
                }
                self.pending.clear();
                if said.is_some() {
                    self.said = said;
                }
            }
            Err(said) => self.said = Some(said),
        }
        self.stale = true;
    }

    fn edit_keys(&mut self) -> bool {
        let Some(file) = self.keys_file.clone() else { return false };
        if !file.exists() {
            let written = file.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(&file, A::Cmd::keymap()));
            if let Err(e) = written {
                return self.say(Message::plain(format!("{KEYMAP} couldn't be made: {e}")));
            }
        }
        self.effects.push(Effect::Open { target: file.to_string_lossy().into_owned() });
        self.stale = true;
        true
    }

    /// The effects the last commands asked for that haven't started yet. Specimen shows these instead of running them.
    pub fn effects(&self) -> &[Effect<A::Cmd>] {
        &self.effects
    }

    fn start(&mut self, effect: Effect<A::Cmd>, window: &winit::window::Window) {
        match effect {
            Effect::SaveAs { name, write, reply } => {
                let dialog = rfd::AsyncFileDialog::new().set_file_name(&name).set_parent(window).save_file();
                let (file, facts) = (name.clone(), self.facts.clone());
                self.saving(format!("Saving “{name}”"), reply, move || {
                    pollster::block_on(dialog).map(|chosen| {
                        let path = chosen.path().to_path_buf();
                        std::fs::write(&path, write(&facts)).map(|()| Path(path)).map_err(|e| Failure::new(format!("“{file}” couldn't be saved: {e}")))
                    })
                });
            }
            Effect::Write { path, write, reply } => {
                let name = path.0.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let facts = self.facts.clone();
                self.saving(format!("Saving “{name}”"), reply, move || {
                    let written = path.0.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(&path.0, write(&facts)));
                    Some(written.map(|()| path).map_err(|e| Failure::new(format!("“{name}” couldn't be saved: {e}"))))
                });
            }
            Effect::Job { name, work, reply } => self.spawn(name, move || Some(work()), reply, None),
            Effect::Stream { name, field, run } => {
                let mut feed = None;
                self.facts.moving(&mut |at, value| {
                    if at == field {
                        feed = value.feed();
                    }
                });
                match feed {
                    Some(feed) => self.spawn(name, move || {
                        run(feed);
                        None
                    }, Box::new(|_| None), None),
                    None => self.said = Some(Said::Failure(format!("“{name}” has no live value to write"))),
                }
            }
            Effect::Open { target } => {
                let opener = if cfg!(target_os = "macos") {
                    "open"
                } else if cfg!(windows) {
                    "explorer"
                } else {
                    "xdg-open"
                };
                if let Err(e) = std::process::Command::new(opener).arg(&target).spawn() {
                    self.said = Some(Said::Failure(format!("{target} couldn't be opened: {e}")));
                }
            }
            Effect::Copy { text } => self.copy(text),
        }
        self.stale = true;
    }

    fn saving(&mut self, name: String, reply: fn(Result<Path, Failure>) -> A::Cmd, work: impl FnOnce() -> Option<Result<Path, Failure>> + Send + 'static) {
        let lost = reply(Err(Failure::new(format!("{name} stopped before it finished"))));
        let back: Reply<A::Cmd> = Box::new(move |done| done.downcast::<Result<Path, Failure>>().ok().map(|done| reply(*done)));
        self.spawn(name, move || work().map(|done| Box::new(done) as Box<dyn Any + Send>), back, Some(lost));
    }

    fn spawn(&mut self, name: String, work: impl FnOnce() -> Option<Box<dyn Any + Send>> + Send + 'static, reply: Reply<A::Cmd>, lost: Option<A::Cmd>) {
        let (send, receive) = mpsc::channel();
        let waker = self.waker.clone();
        let started = std::thread::Builder::new().name("graticule job".into()).spawn(move || {
            let _ = send.send(work());
            if let Some(waker) = waker {
                waker.wake();
            }
        });
        match started {
            Ok(_) => self.jobs.push(Job { name, receive, reply, lost }),
            Err(e) => self.said = Some(Said::Failure(format!("{name} couldn't start: {e}"))),
        }
    }

    fn collect(&mut self) {
        self.arrive();
        if self.keys_watch.as_ref().is_some_and(|(_, changed)| changed.try_iter().count() > 0) {
            self.read_keys();
        }
        let mut results = Vec::new();
        let mut at = 0;
        while at < self.jobs.len() {
            match self.jobs[at].receive.try_recv() {
                Err(mpsc::TryRecvError::Empty) => at += 1,
                Ok(done) => {
                    let job = self.jobs.remove(at);
                    results.extend(done.and_then(job.reply));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    let job = self.jobs.remove(at);
                    match job.lost {
                        Some(lost) => results.push(lost),
                        None => self.said = Some(Said::Failure(format!("“{}” stopped before it finished", job.name))),
                    }
                    self.stale = true;
                }
            }
        }
        for cmd in results {
            self.run(cmd);
        }
    }

    /// Adds the host's own commands to the window.
    pub fn host(&mut self, extras: Vec<Extra>) {
        self.extras = extras;
        self.stale = true;
    }

    /// Which of the host's commands was chosen since the last call.
    pub fn asked(&mut self) -> Option<usize> {
        self.asked.take()
    }

    /// Keeps effects instead of running them: Specimen shows what the app asked for, and no panel opens.
    pub fn hold_effects(&mut self) {
        self.hold = true;
        if !self.effects.is_empty() {
            let asked: Vec<String> = self.effects.drain(..).map(|e| format!("{e:?}")).collect();
            self.said = Some(Said::Note(Message::plain(format!("Specimen didn't start what the app runs: {}", asked.join(", then ")))));
        }
    }

    fn ask(&mut self, e: usize) -> bool {
        match self.extras.get(e).map(|x| x.available.clone()) {
            Some(Available::No(reason)) => self.say(reason),
            Some(Available::Yes) => {
                self.asked = Some(e);
                true
            }
            None => false,
        }
    }

    fn marks(&self) -> Vec<(Id, u128, u32)> {
        self.tree.lists.iter().filter_map(|l| Some((self.tree.nodes[l.node as usize].id, l.focused?, l.focus_index?))).collect()
    }

    fn repair(&mut self, marks: Vec<(Id, u128, u32)>) {
        if self.repairing || marks.is_empty() {
            return;
        }
        self.repairing = true;
        self.build();
        for (id, focused, index) in marks {
            let Some(list) = self.tree.find(id).and_then(|i| self.tree.lists.get(self.tree.nodes[i as usize].list as usize).copied()) else { continue };
            let facts = &self.facts as &dyn Any;
            if list.count == 0 || !(list.keys)(facts, list.items, list.selection, Some((focused, focused))).is_empty() || list.focused.is_some_and(|f| f != focused) {
                continue;
            }
            let Some(key) = (list.nth)(facts, list.items, index.min(list.count - 1)) else { continue };
            self.gesture += 1;
            if let Some(cmd) = <A::Cmd as Commands<A>>::edit(Edit::new(list.selection, Change::Select(Choose::Only(key)), self.gesture)) {
                let said = self.said.take();
                self.run(cmd);
                self.said = said;
            }
        }
        self.repairing = false;
    }

    /// Runs the command a keymap file or the palette names, with its subjects filled in from the current list.
    pub fn run_named(&mut self, name: &str) -> bool {
        if self.stale {
            self.build();
        }
        let here = self.here();
        let target = self.named(name, &here);
        let times = std::mem::take(&mut self.count).max(1);
        match target {
            Target::Builtin(b) => self.builtin_times(b, times),
            Target::Ask(e) => self.ask_for(e),
            Target::Host(e) => self.ask(e),
            Target::One(cmd) if times > 1 => self.repeat(cmd, times),
            Target::One(cmd) => {
                self.run(cmd);
                true
            }
            Target::Many(cmds, _) => self.run_many(cmds),
            Target::Refused(_, reason) => self.say(reason),
        }
    }

    fn fire(&mut self, i: u32) -> bool {
        if self.inspecting {
            return self.describe(i);
        }
        let fired = self.fire_once(i);
        if self.count > 0 {
            self.count = 0;
            self.stale = true;
        }
        fired
    }

    fn fire_once(&mut self, i: u32) -> bool {
        let n = self.tree.nodes[i as usize];
        let why = self.tree.str(n.reason).to_string();
        if !n.enabled && (n.cmd == NONE || !why.is_empty()) {
            return !why.is_empty() && self.say(Message::plain(why));
        }
        if let Some(b) = n.builtin {
            return self.builtin(b);
        }
        if n.host != NONE {
            return self.ask(n.host as usize);
        }
        if n.asks != NONE {
            return self.ask_for(n.asks as usize);
        }
        if n.set != NONE
            && let Some(path) = n.path
        {
            let (value, said) = self.tree.sets[n.set as usize].clone();
            self.gesture += 1;
            let Some(cmd) = <A::Cmd as Commands<A>>::edit(Edit::new(path, Change::Set(value), self.gesture)) else { return false };
            self.naming = Some(said);
            let ran = self.run(cmd);
            self.naming = None;
            return ran;
        }
        if n.cmd == NONE {
            return false;
        }
        let cmd = self.tree.commands[n.cmd as usize].clone();
        if n.many > 0
            && let Some(e) = cmd.entry()
            && let Target::Many(cmds, _) = self.target(e, &self.here())
        {
            return self.run_many(cmds);
        }
        self.run(cmd);
        true
    }

    fn builtin(&mut self, b: Builtin) -> bool {
        let b = match b {
            Builtin::Up if self.selecting() => Builtin::ExtendUp,
            Builtin::Down if self.selecting() => Builtin::ExtendDown,
            b => b,
        };
        if let Available::No(reason) = self.builtin_available(b) {
            return self.say(reason);
        }
        match b {
            Builtin::Palette => {
                self.palette = Some(Palette::default());
                self.pending.clear();
                self.count = 0;
                self.stale = true;
                true
            }
            Builtin::Undo | Builtin::Redo => self.undo(b == Builtin::Undo),
            Builtin::Next | Builtin::Previous => self.cycle(b == Builtin::Next),
            Builtin::Cancel => self.cancel(),
            Builtin::EditKeys => self.edit_keys(),
            Builtin::Help => {
                self.sheet = if matches!(self.sheet, Some((Sheet::Help, _))) { None } else { Some((Sheet::Help, 0)) };
                self.stale = true;
                true
            }
            Builtin::ChangedSettings | Builtin::AllSettings => {
                self.only_changed = b == Builtin::ChangedSettings;
                self.stale = true;
                true
            }
            Builtin::Log => {
                self.sheet = if matches!(self.sheet, Some((Sheet::Log, _))) { None } else { Some((Sheet::Log, 0)) };
                self.stale = true;
                true
            }
            Builtin::WhatsThis => {
                self.inspecting = true;
                self.say(Message::from("Click anything, or press Return on it, to see what it is and what it does; Escape stops"))
            }
            Builtin::NormalMode | Builtin::InsertMode | Builtin::SelectMode => {
                self.mode = b.mode().unwrap_or_default();
                self.pending.clear();
                self.stale = true;
                true
            }
            Builtin::Copy => {
                let text = self.chosen_text();
                self.copy(text);
                false
            }
            Builtin::LightAppearance | Builtin::DarkAppearance | Builtin::SystemAppearance | Builtin::NextTheme | Builtin::NextStyle | Builtin::AbsoluteDates | Builtin::RelativeDates => {
                self.switch = b.switch();
                self.did(b.label().to_string());
                true
            }
            _ => {
                let Some(list) = self.current_list() else { return false };
                let keys = |span| (list.keys)(&self.facts as &dyn Any, list.items, list.selection, Some(span));
                let start = if list.focused.is_none() { None } else { Some(()) };
                let choose = match b {
                    Builtin::Up => start.map_or(list.last, |_| list.before).map(Choose::Only),
                    Builtin::Down => start.map_or(list.first, |_| list.after).map(Choose::Only),
                    Builtin::First => list.first.map(Choose::Only),
                    Builtin::Last => list.last.map(Choose::Only),
                    Builtin::ExtendUp | Builtin::ExtendDown => {
                        let to = if b == Builtin::ExtendUp { list.before } else { list.after };
                        match (list.anchor.or(list.focused), to) {
                            (Some(from), Some(to)) => Some(Choose::Extend { to, between: keys((from, to)) }),
                            _ => None,
                        }
                    }
                    Builtin::SelectAll => match (list.first, list.last) {
                        (Some(first), Some(last)) => Some(Choose::All(keys((first, last)))),
                        _ => None,
                    },
                    _ => None,
                };
                self.focus = Some(self.tree.nodes[list.node as usize].id);
                self.stale = true;
                match choose {
                    Some(choose) => self.select(&list, choose),
                    None => true,
                }
            }
        }
    }

    fn select(&mut self, list: &ListInfo, choose: Choose) -> bool {
        self.gesture += 1;
        self.reveal = true;
        match <A::Cmd as Commands<A>>::edit(Edit::new(list.selection, Change::Select(choose), self.gesture)) {
            Some(cmd) => self.run(cmd),
            None => false,
        }
    }

    fn focusable(&self, i: usize) -> bool {
        match self.tree.nodes[i].kind {
            Kind::Field | Kind::Button | Kind::Link | Kind::Check { .. } => !self.tree.in_row(i as u32) && self.tree.ancestor(i as u32, |n| n.kind == Kind::Palette).is_none(),
            Kind::List { .. } => true,
            _ => false,
        }
    }

    fn cycle(&mut self, forward: bool) -> bool {
        let order: Vec<Id> = (0..self.tree.nodes.len()).filter(|&i| self.focusable(i)).map(|i| self.tree.nodes[i].id).collect();
        if order.is_empty() {
            return false;
        }
        let len = order.len();
        let next = match self.focus.and_then(|f| order.iter().position(|&o| o == f)) {
            Some(p) if forward => (p + 1) % len,
            Some(p) => (p + len - 1) % len,
            None if forward => 0,
            None => len - 1,
        };
        self.focus_on(order[next], true);
        true
    }

    fn focus_on(&mut self, id: Id, select_all: bool) {
        self.focus = Some(id);
        self.typing = None;
        if let Some(i) = self.tree.find(id)
            && self.tree.nodes[i as usize].kind == Kind::Field
        {
            let len = self.tree.str(self.tree.nodes[i as usize].text).len();
            let caret = if select_all { Caret { caret: len, anchor: 0 } } else { Caret::at(len) };
            self.carets.insert(id, caret);
        }
        self.stale = true;
    }

    fn cancel(&mut self) -> bool {
        if self.sheet.take().is_some() {
            self.stale = true;
            return true;
        }
        if std::mem::take(&mut self.inspecting) {
            self.said = None;
            self.stale = true;
            return true;
        }
        if self.palette.take().is_some() || !self.pending.is_empty() || self.count > 0 {
            self.pending.clear();
            self.count = 0;
            self.stale = true;
            return true;
        }
        if self.focused_field().is_some() {
            self.focus = None;
            self.stale = true;
            return true;
        }
        if self.reading.take().is_some() {
            return true;
        }
        match self.current_list() {
            Some(list) if list.chosen > 1 => match list.focused {
                Some(k) => self.select(&list, Choose::Focus(k)),
                None => false,
            },
            _ => false,
        }
    }

    fn focused_field(&self) -> Option<u32> {
        let i = self.tree.find(self.focus?)?;
        let n = self.tree.nodes[i as usize];
        (n.kind == Kind::Field && n.path.is_some()).then_some(i)
    }

    fn focused_control(&self) -> Option<u32> {
        let i = self.tree.find(self.focus?)?;
        matches!(self.tree.nodes[i as usize].kind, Kind::Button | Kind::Link | Kind::Check { .. }).then_some(i)
    }

    fn caret(&self, i: u32) -> Caret {
        let n = self.tree.nodes[i as usize];
        let len = self.tree.str(n.text).len();
        self.carets.get(&n.id).copied().unwrap_or(Caret::at(len))
    }

    fn follow(&mut self) {
        let Some(i) = self.focused_field() else { return };
        let n = self.tree.nodes[i as usize];
        if let Some(c) = self.carets.get_mut(&n.id) {
            c.follow(self.tree.str(n.text), None);
        }
    }

    fn key(&mut self, event: &KeyEvent, mods: Mods) -> bool {
        let Some(chord) = Chord::of(event, mods) else { return false };
        if !self.composing.is_empty() {
            return false;
        }
        self.press(chord, event.text.as_deref())
    }

    /// Handles a key and the text it types: the palette when it is open, then the focused field, then a focused control's own keys, then the keymap, then the platform's conventions.
    pub fn press(&mut self, chord: Chord, text: Option<&str>) -> bool {
        if self.stale {
            self.build();
        }
        let text = text.filter(|t| !t.is_empty() && !t.chars().any(char::is_control)).map(str::to_string);
        if self.palette.is_some() {
            return self.palette_key(chord, text);
        }
        if self.sheet.is_some() && self.pending.is_empty() && chord.mods == Mods::default() {
            let page = self.metrics.map_or(30, |m| m.screen().rows - HELP_CHROME).max(1);
            let by = match chord.key {
                Key::Up => -1,
                Key::Down => 1,
                Key::PageUp => -page,
                Key::PageDown => page,
                Key::Home => i32::MIN,
                Key::End => i32::MAX,
                _ => 0,
            };
            if by != 0 {
                return self.scroll_help(by);
            }
        }
        let typing = !self.keymap.modal || self.mode == Mode::Insert;
        if self.pending.is_empty()
            && typing
            && let Some(i) = self.focused_field()
        {
            if chord.types()
                && let Some(t) = text
            {
                return self.field_action(i, field::insert(self.caret(i), &t));
            }
            if field::handles(chord.key, chord.mods) {
                let value = self.tree.str(self.tree.nodes[i as usize].text).to_string();
                let action = field::key(&value, self.caret(i), chord.key, chord.mods, &mut self.kill);
                return self.field_action(i, action);
            }
        }
        let control = if self.pending.is_empty() && self.count == 0 && chord.mods == Mods::default() && matches!(chord.key, Key::Enter | Key::Char(' ')) { self.focused_control() } else { None };
        if typing && let Some(i) = control {
            return self.fire(i);
        }
        if (!self.pending.is_empty() || self.count > 0) && chord.key == Key::Escape {
            self.pending.clear();
            self.count = 0;
            self.stale = true;
            return true;
        }
        let lookup = self.keymap.lookup(self.mode, &self.pending, chord);
        if self.pending.is_empty()
            && matches!(lookup, Lookup::Nothing)
            && chord.mods == Mods::default()
            && let Key::Char(c) = chord.key
            && let Some(digit) = c.to_digit(10)
            && (digit > 0 || self.count > 0)
        {
            self.count = self.count.saturating_mul(10).saturating_add(digit).min(MOST_TIMES);
            self.stale = true;
            return true;
        }
        match lookup {
            Lookup::Run(name) => {
                let name = name.to_string();
                self.pending.clear();
                return self.run_named(&name);
            }
            Lookup::More => {
                self.pending.push(chord);
                self.stale = true;
                return true;
            }
            Lookup::Nothing if !self.pending.is_empty() || self.count > 0 => {
                self.pending.push(chord);
                let typed = self.so_far();
                if let (true, Some(b)) = (self.pending.len() == 1, Builtin::ALL.into_iter().find(|b| b.convention().contains(&chord))) {
                    self.pending.clear();
                    let times = std::mem::take(&mut self.count);
                    self.stale = true;
                    return self.builtin_times(b, times);
                }
                self.pending.clear();
                self.count = 0;
                return self.say(Message::plain(format!("{typed} does nothing")));
            }
            Lookup::Operator(name) => {
                let name = name.to_string();
                let typed = format!("{} {chord}", self.so_far());
                self.pending.clear();
                let times = std::mem::take(&mut self.count).max(1);
                self.stale = true;
                return self.operate(&name, chord, times, &typed);
            }
            Lookup::Nothing => {}
        }
        match (Builtin::ALL.into_iter().find(|b| b.convention().contains(&chord)), control) {
            (Some(b), _) => self.builtin(b),
            (None, Some(i)) => self.fire(i),
            (None, None) => false,
        }
    }

    fn operate(&mut self, name: &str, motion: Chord, times: u32, typed: &str) -> bool {
        let movement = match self.keymap.lookup(self.mode, &[], motion) {
            Lookup::Run(m) => Builtin::find(m),
            _ => Builtin::ALL.into_iter().find(|b| b.convention().contains(&motion)),
        };
        let Some(movement) = movement.filter(|b| matches!(b, Builtin::Up | Builtin::Down | Builtin::ExtendUp | Builtin::ExtendDown | Builtin::First | Builtin::Last)) else {
            return self.say(Message::plain(format!("{typed} does nothing: {motion} is not a movement")));
        };
        let Some(e) = A::Cmd::entries().iter().position(|e| e.name == name && !e.internal) else {
            return self.say(Message::plain(format!("No command is called “{name}”")));
        };
        let entry = A::Cmd::entries()[e];
        if !entry.subject || !entry.many || !entry.args.is_empty() {
            return self.say(Message::plain(format!("{} takes one item at a time, so it can't act over a movement", title(name))));
        }
        let Some(list) = self.current_list().filter(|l| entry.subject_type.is_none_or(|t| t() == l.item)) else {
            return self.say(Message::from("There is nothing here for it to act on"));
        };
        let Some(from) = list.focus_index else { return self.say(Message::from("Nothing is chosen")) };
        let last = list.count.saturating_sub(1);
        let to = match movement {
            Builtin::Up | Builtin::ExtendUp => from.saturating_sub(times),
            Builtin::Down | Builtin::ExtendDown => from.saturating_add(times).min(last),
            Builtin::First => 0,
            _ => last,
        };
        let cmds = (from.min(to)..=from.max(to)).filter_map(|i| (list.nth)(&self.facts as &dyn Any, list.items, i)).filter_map(|k| A::Cmd::make(e, Some(k), &[])).collect();
        self.run_many(cmds)
    }

    fn selecting(&self) -> bool {
        self.keymap.modal && self.mode == Mode::Select
    }

    fn so_far(&self) -> String {
        let keys = keys::label(&self.pending);
        match self.count {
            0 => keys,
            n if keys.is_empty() => n.to_string(),
            n => format!("{n} {keys}"),
        }
    }

    fn field_action(&mut self, i: u32, action: Action) -> bool {
        let n = self.tree.nodes[i as usize];
        match action {
            Action::Ignored => true,
            Action::Moved(c) => {
                self.carets.insert(n.id, c);
                self.typing = None;
                true
            }
            Action::Splice { range, text, caret, kind } => self.splice(i, range, text, caret, kind),
            Action::Copy(s) => {
                self.copy(s);
                true
            }
            Action::Cut { range, text } => {
                self.copy(text);
                let caret = range.start;
                self.splice(i, range, String::new(), caret, field::Kind::Alone)
            }
            Action::Paste => {
                let pasted = self.paste();
                if pasted.is_empty() {
                    return true;
                }
                let range = self.caret(i).selection();
                let caret = range.start + pasted.len();
                self.splice(i, range, pasted, caret, field::Kind::Alone)
            }
            Action::Submit => {
                self.typing = None;
                if n.submit != NONE {
                    let cmd = self.tree.commands[n.submit as usize].clone();
                    self.run(cmd);
                }
                true
            }
            Action::SelectAll => {
                let len = self.tree.str(n.text).len();
                self.carets.insert(n.id, Caret { caret: len, anchor: 0 });
                true
            }
        }
    }

    fn splice(&mut self, i: u32, range: std::ops::Range<usize>, text: String, caret: usize, kind: field::Kind) -> bool {
        let n = self.tree.nodes[i as usize];
        let Some(path) = n.path else { return false };
        let space = text.starts_with(char::is_whitespace);
        let continues = kind != field::Kind::Alone && matches!(self.typing, Some((id, k, after_space)) if id == n.id && k == kind && (after_space || !space));
        if !continues {
            self.gesture += 1;
        }
        self.typing = (kind != field::Kind::Alone).then_some((n.id, kind, text.ends_with(char::is_whitespace)));
        let Some(cmd) = <A::Cmd as Commands<A>>::edit(Edit::new(path, Change::Splice { range, text }, self.gesture)) else { return false };
        if self.run(cmd) {
            self.carets.insert(n.id, Caret::at(caret));
        }
        true
    }

    fn ask_for(&mut self, entry: usize) -> bool {
        self.palette = Some(Palette { asking: Some(Asking { entry, got: Vec::new() }), ..Palette::default() });
        self.pending.clear();
        self.stale = true;
        true
    }

    fn answer(&mut self, mut palette: Palette, mut asking: Asking) -> bool {
        let entry = A::Cmd::entries()[asking.entry];
        let Some(arg) = entry.args.get(asking.got.len()) else { return true };
        match (arg.read)(&palette.query) {
            Ok(value) => asking.got.push(value),
            Err(why) => {
                palette.asking = Some(asking);
                self.palette = Some(palette);
                return self.say(why);
            }
        }
        if asking.got.len() < entry.args.len() {
            self.palette = Some(Palette { asking: Some(asking), ..Palette::default() });
            return true;
        }
        let here = self.here();
        let times = std::mem::take(&mut self.count).max(1);
        match self.target_with(asking.entry, &here, &asking.got) {
            Target::One(cmd) if times > 1 => self.repeat(cmd, times),
            Target::One(cmd) => self.run(cmd),
            Target::Many(cmds, _) => self.run_many(cmds),
            Target::Refused(_, reason) => self.say(reason),
            Target::Ask(_) | Target::Builtin(_) | Target::Host(_) => true,
        }
    }

    fn palette_key(&mut self, chord: Chord, text: Option<String>) -> bool {
        let Some(mut palette) = self.palette.take() else { return false };
        self.stale = true;
        if chord.key == Key::Enter
            && let Some(asking) = palette.asking.take()
        {
            return self.answer(palette, asking);
        }
        match chord.key {
            Key::Escape => return true,
            Key::Up => palette.chosen = palette.chosen.saturating_sub(1),
            Key::Down => palette.chosen = (palette.chosen + 1).min(palette.shown.saturating_sub(1)),
            Key::Enter => {
                return match self.tree.nodes.iter().position(|n| n.kind == Kind::Choice { chosen: true }) {
                    Some(i) => self.fire(i as u32),
                    None => true,
                };
            }
            _ => {
                let action = match text.filter(|_| chord.types()) {
                    Some(t) => field::insert(palette.caret, &t),
                    None => field::key(&palette.query, palette.caret, chord.key, chord.mods, &mut self.kill),
                };
                self.query(&mut palette, action);
            }
        }
        self.palette = Some(palette);
        true
    }

    fn query(&mut self, palette: &mut Palette, action: Action) {
        let (range, text) = match action {
            Action::Moved(c) => {
                palette.caret = c;
                return;
            }
            Action::SelectAll => {
                palette.caret = Caret { caret: palette.query.len(), anchor: 0 };
                return;
            }
            Action::Copy(s) => {
                self.copy(s);
                return;
            }
            Action::Splice { range, text, .. } => (range, text),
            Action::Cut { range, text } => {
                self.copy(text);
                (range, String::new())
            }
            Action::Paste => (palette.caret.selection(), self.paste()),
            Action::Ignored | Action::Submit => return,
        };
        let caret = range.start + text.len();
        palette.query.replace_range(range, &text);
        palette.caret = Caret::at(caret);
        palette.chosen = 0;
    }

    fn clipboard(&mut self) -> Option<&mut arboard::Clipboard> {
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
            if self.clipboard.is_none() {
                self.say(Message::from("The clipboard can't be reached"));
            }
        }
        self.clipboard.as_mut()
    }

    fn copy(&mut self, text: String) {
        if let Some(c) = self.clipboard() {
            let _ = c.set_text(text);
        }
    }

    fn paste(&mut self) -> String {
        let text = self.clipboard().and_then(|c| c.get_text().ok()).unwrap_or_default();
        text.split(['\r', '\n']).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ")
    }

    fn typed(&mut self, text: &str) -> bool {
        if let Some(mut palette) = self.palette.take() {
            let action = field::insert(palette.caret, text);
            self.query(&mut palette, action);
            self.palette = Some(palette);
            self.stale = true;
            return true;
        }
        match self.focused_field() {
            Some(i) => self.field_action(i, field::insert(self.caret(i), text)),
            None => false,
        }
    }

    fn activate(&mut self, i: u32, mods: Mods) -> bool {
        let n = self.tree.nodes[i as usize];
        match n.kind {
            Kind::Button | Kind::Link | Kind::Action | Kind::Check { .. } => self.fire(i),
            Kind::Choice { .. } => {
                self.palette = None;
                self.stale = true;
                self.fire(i)
            }
            Kind::ListRow { key, .. } => {
                let Some(list) = self.tree.ancestor(n.parent, |p| matches!(p.kind, Kind::List { .. })).map(|l| self.tree.nodes[l as usize].list).and_then(|l| self.tree.lists.get(l as usize).copied())
                else {
                    return false;
                };
                self.focus = Some(self.tree.nodes[list.node as usize].id);
                let choose = if mods.primary {
                    Choose::Toggle(key)
                } else if mods.shift
                    && let Some(from) = list.anchor.or(list.focused)
                {
                    Choose::Extend { to: key, between: (list.keys)(&self.facts as &dyn Any, list.items, list.selection, Some((from, key))) }
                } else {
                    Choose::Only(key)
                };
                self.select(&list, choose)
            }
            _ => false,
        }
    }

    /// Clicks whatever acts at this cell, as the mouse would.
    pub fn click(&mut self, col: i32, row: i32) -> bool {
        if self.inspecting {
            return self.inspect_at(col, row);
        }
        match self.hit(col, row) {
            Some(i) => self.activate(i, Mods::default()),
            None => false,
        }
    }

    fn hit(&self, col: i32, row: i32) -> Option<u32> {
        if self.stale || !self.laid {
            return None;
        }
        self.hits.at(col, row).filter(|&i| (i as usize) < self.tree.nodes.len())
    }

    fn readable(&self, frame: &Frame) -> Option<u32> {
        let (x, y) = frame.pointer?;
        let (col, row) = frame.metrics.cell_at(x, y);
        self.tree.nodes.iter().rposition(|n| matches!(n.kind, Kind::Text { .. } | Kind::Value { .. }) && n.area.contains(col, row)).map(|i| i as u32)
    }

    /// The bytes of a text or value node under the pointer: a point for one click, the word for two, all of it for three. A value is chosen whole.
    fn read_at(&self, frame: &mut Frame, i: u32, clicks: u32) -> std::ops::Range<usize> {
        let n = self.tree.nodes[i as usize];
        let s = self.tree.str(n.text);
        let (m, (x, y)) = (frame.metrics, frame.pointer.unwrap_or_default());
        let r = m.rect(n.area);
        let style = match n.kind {
            Kind::Text { mono: true, .. } | Kind::Value { .. } => m.mono(),
            Kind::Text { strong: true, .. } => m.sans(600.0),
            _ => m.sans(400.0),
        };
        if clicks >= 3 || (clicks == 2 && matches!(n.kind, Kind::Value { .. })) {
            return 0..s.len();
        }
        if let Kind::Value { .. } = n.kind {
            let at = if x < r.x + r.w / 2.0 { 0 } else { s.len() };
            return at..at;
        }
        if style.face == crate::text::Face::Mono {
            let local = x - r.x;
            let at = s
                .char_indices()
                .map(|(i, _)| i)
                .chain([s.len()])
                .map(|k| (k, (crate::paint::advance(frame.text, m, &s[..k], style) - local).abs()))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map_or(0, |(k, _)| k);
            return if clicks == 2 { field::word_at(s, at).selection() } else { at..at };
        }
        let spans = [(s, crate::text::SpanStyle { weight: style.weight as u16, ..crate::text::SpanStyle::plain(style.face) })];
        if clicks == 2 {
            return frame.text.word_at(&spans, style.size, m.cell_h, r.w, x - r.x, y - r.y);
        }
        let at = frame.text.byte_at(&spans, style.size, m.cell_h, r.w, x - r.x, y - r.y).min(s.len());
        at..at
    }

    /// The text and value nodes the chosen text covers, in reading order, with the bytes chosen in each.
    fn chosen(&self) -> Vec<(u32, std::ops::Range<usize>)> {
        let Some(reading) = self.reading else { return Vec::new() };
        let (Some(a), Some(h)) = (self.tree.find(reading.anchor.0), self.tree.find(reading.head.0)) else { return Vec::new() };
        let ((from, start), (to, end)) = if (a, reading.anchor.1) <= (h, reading.head.1) { ((a, reading.anchor.1), (h, reading.head.1)) } else { ((h, reading.head.1), (a, reading.anchor.1)) };
        (from..=to)
            .filter(|&i| matches!(self.tree.nodes[i as usize].kind, Kind::Text { .. } | Kind::Value { .. }))
            .filter_map(|i| {
                let len = self.tree.str(self.tree.nodes[i as usize].text).len();
                let range = if i == from { start.min(len) } else { 0 }..if i == to { end.min(len) } else { len };
                (range.start < range.end).then_some((i, range))
            })
            .collect()
    }

    /// The context menu for the cell at `col`, `row`: on a list row, that row's commands, choosing the row first unless it is chosen already, so a menu on one of several chosen rows acts on all of them; anywhere else, the commands that need no subject. Copy comes first while text is chosen. Each entry is the bar's own control, with its label, availability and key; one that can't run says why in its label, since a native menu has no status line.
    pub fn menu_at(&mut self, col: i32, row: i32) -> Vec<crate::menu::MenuItem> {
        let on_row = self.tree.nodes.iter().rposition(|n| matches!(n.kind, Kind::ListRow { .. }) && n.area.contains(col, row));
        if let Some(i) = on_row
            && let Kind::ListRow { key, selected: false, .. } = self.tree.nodes[i].kind
            && let Some(list) = self.tree.ancestor(i as u32, |n| matches!(n.kind, Kind::List { .. })).and_then(|l| self.tree.lists.get(self.tree.nodes[l as usize].list as usize).copied())
        {
            self.select(&list, Choose::Only(key));
        }
        if self.stale {
            self.build();
        }
        let mut items = Vec::new();
        if !self.chosen().is_empty() {
            items.push(crate::menu::MenuItem { id: COPY.0, label: Builtin::Copy.label().into(), keys: Builtin::Copy.convention().first().copied(), enabled: true });
        }
        let Some(bar) = self.tree.nodes.iter().position(|n| n.kind == Kind::Bar) else { return items };
        for i in self.tree.children(bar as u32).collect::<Vec<_>>() {
            let n = self.tree.nodes[i as usize];
            if n.kind != Kind::Action || n.builtin.is_some() {
                continue;
            }
            let entry = (n.cmd != NONE).then(|| self.tree.commands[n.cmd as usize].name()).and_then(|name| A::Cmd::entries().iter().find(|e| e.name == name));
            if entry.is_some_and(|e| e.subject) != on_row.is_some() {
                continue;
            }
            let keys = entry.and_then(|e| self.keymap.chord_for(self.mode, e.name));
            let (name, why) = (self.tree.str(n.name), self.tree.str(n.reason));
            let label = if n.enabled || why.is_empty() { name.to_string() } else { format!("{name}: {why}") };
            items.push(crate::menu::MenuItem { id: n.id.0, label, keys, enabled: n.enabled });
        }
        items
    }

    /// Runs the menu entry with this id, as clicking its control in the bar would.
    pub fn choose_from_menu(&mut self, id: u64) -> bool {
        if Id(id) == COPY {
            let text = self.chosen_text();
            self.copy(text);
            return false;
        }
        match self.tree.find(Id(id)) {
            Some(i) => self.activate(i, Mods::default()),
            None => false,
        }
    }

    /// The chosen text as it reads: pieces on one row joined by a space, rows by a new line.
    fn chosen_text(&self) -> String {
        let mut out = String::new();
        let mut row = None;
        for (i, range) in self.chosen() {
            let n = self.tree.nodes[i as usize];
            if let Some(r) = row {
                out.push(if r == n.area.row { ' ' } else { '\n' });
            }
            out.push_str(&self.tree.str(n.text)[range]);
            row = Some(n.area.row);
        }
        out
    }

    fn hovered(&self, frame: &Frame) -> Option<Id> {
        let (x, y) = frame.pointer?;
        let (col, row) = frame.metrics.cell_at(x, y);
        self.hit(col, row).map(|i| self.tree.nodes[i as usize].id)
    }

    fn caret_at(&self, text: &mut Text, m: Metrics, i: u32, x: f32) -> usize {
        let n = self.tree.nodes[i as usize];
        let value = self.tree.str(n.text);
        let r = m.rect(n.area);
        let mono = m.mono();
        let before = crate::paint::advance(text, m, &value[..self.caret(i).caret.min(value.len())], mono);
        let shift = if self.focus == Some(n.id) { (before - (r.w - m.cell_w * 1.5)).max(0.0) } else { 0.0 };
        let local = x - (r.x + m.cell_w * 0.5) + shift;
        let mut best = (0, f32::MAX);
        for at in value.char_indices().map(|(i, _)| i).chain([value.len()]) {
            let d = (crate::paint::advance(text, m, &value[..at], mono) - local).abs();
            if d < best.1 {
                best = (at, d);
            }
        }
        best.0
    }

    fn scroll(&mut self, col: i32, row: i32, by: i32) -> bool {
        let Some(i) = self.tree.nodes.iter().rposition(|n| matches!(n.kind, Kind::List { .. }) && n.area.contains(col, row)) else { return false };
        let n = self.tree.nodes[i];
        let Kind::List { count, first, .. } = n.kind else { return false };
        let last = (count as i32 - n.area.rows).max(0);
        let to = (first as i32 + by).clamp(0, last) as u32;
        if to == first {
            return false;
        }
        let key = self.tree.children(i as u32).find_map(|c| match self.tree.nodes[c as usize].kind {
            Kind::ListRow { key, index, .. } if index == to => Some(key),
            _ => None,
        });
        let s = self.scrolls.entry(n.id).or_default();
        *s = Scroll { anchor: key, index: to, ..*s };
        self.stale = true;
        true
    }
}

impl<A: App> Drop for Runner<A> {
    fn drop(&mut self) {
        if let Keep::Store(_) = &self.keep {
            self.finish();
        }
        if let Keep::Store(store) = &self.keep {
            store.place(self.whereabouts());
        }
    }
}

impl<A: App> Face for Runner<A> {
    fn paint(&mut self, p: &mut Painter, _: Seen) {
        if self.metrics != Some(p.m) || self.fonts != p.text.atlas || self.drawn_in != Some((p.theme, p.style)) {
            self.metrics = Some(p.m);
            self.fonts = p.text.atlas;
            self.drawn_in = Some((p.theme, p.style));
            self.kept.reset();
            self.laid = false;
        }
        let started = Instant::now();
        if self.stale {
            self.build();
        }
        let built = Instant::now();
        if !self.laid {
            self.layout(p);
        }
        let laid = Instant::now();
        let look = self.look();
        let mut kept = std::mem::take(&mut self.kept);
        kept.update(p, &self.tree, look);
        self.kept = kept;
        let micros = |from: Instant, to: Instant| (to - from).as_micros().min(u32::MAX as u128) as u32;
        self.stages = crate::scope::Stages { view: micros(started, built), layout: micros(built, laid), paint: micros(laid, Instant::now()) };
    }

    fn stages(&self) -> crate::scope::Stages {
        self.stages
    }

    fn keep(&self) -> Value {
        let facts = match self.keep {
            Keep::Store(_) => Value::None,
            Keep::Nothing | Keep::Memory(_) => ledger::Field::value(&self.facts),
        };
        Value::Map(vec![("facts".into(), facts), ("place".into(), self.whereabouts())])
    }

    fn restore(&mut self, kept: &Value) {
        match kept.field("facts") {
            None | Some(Value::None) => {}
            Some(value) => match A::from_value(value) {
                Some(mut facts) => {
                    ledger::Field::attach(&mut facts, ErasedPath::ROOT);
                    self.facts = facts;
                }
                None => self.said = Some(Said::Note(Message::from("The facts from before the reload don't fit this build, so it starts afresh"))),
            },
        }
        if let Some(place) = kept.field("place") {
            self.return_to(place);
        }
        self.stale = true;
    }

    fn painted(&mut self, frame: &mut Frame) {
        for effect in std::mem::take(&mut self.effects) {
            self.start(effect, frame.window);
        }
        if (self.turning.is_some()
            || self.background.front().is_some_and(|b| !b.rest.is_empty())
            || self.background.front().and_then(|b| b.parts.front()).is_some_and(|p| matches!(p.found, Found::Ready(_))))
            && let Some(waker) = &self.waker
        {
            waker.wake();
        }
        let field = if self.palette.is_some() { self.tree.find(PALETTE) } else { self.focused_field() };
        let want = field.is_some();
        if want != self.ime {
            frame.window.set_ime_allowed(want);
            self.ime = want;
        }
        if let Some(i) = field {
            let (m, n) = (frame.metrics, self.tree.nodes[i as usize]);
            let r = m.rect(n.area);
            let value = self.tree.str(n.text);
            let caret = field::boundary(value, self.caret(i).caret.min(value.len()));
            let before = crate::paint::advance(frame.text, m, &value[..caret], m.mono());
            let composed = crate::paint::advance(frame.text, m, &self.composing, m.mono());
            let x = r.x + m.cell_w * 0.5 + before - (before + composed - (r.w - m.cell_w * 1.5)).max(0.0);
            frame.window.set_ime_cursor_area(winit::dpi::PhysicalPosition::new(x as f64, r.y as f64), winit::dpi::PhysicalSize::new(m.cell_w as f64, r.h as f64));
        }
    }

    fn kept(&mut self) -> Option<&mut Kept> {
        Some(&mut self.kept)
    }

    fn switch(&mut self) -> Option<Switch> {
        self.switch.take()
    }

    fn offered(&mut self, looks: &Looks) {
        if !self.calendar.live || self.calendar.dates != looks.dates {
            self.calendar = Calendar::live(looks.dates);
            self.stale = true;
        }
        let offered = looks.offered();
        if offered != self.offered {
            self.offered = offered;
            self.stale = true;
        }
        let problem = looks.problems.first().cloned();
        if problem != self.offered_problem {
            if let Some(p) = &problem {
                self.said = Some(Said::Note(Message::plain(format!("A look in the looks folder can't be used: {p}"))));
                self.stale = true;
            }
            self.offered_problem = problem;
        }
    }

    fn minimum(&self) -> Option<(i32, i32)> {
        self.minimum
    }

    fn menu(&mut self, frame: &mut Frame) -> Vec<crate::menu::MenuItem> {
        let Some((x, y)) = frame.pointer else { return Vec::new() };
        let (col, row) = frame.metrics.cell_at(x, y);
        self.menu_at(col, row)
    }

    fn choose(&mut self, id: u64) -> bool {
        self.choose_from_menu(id)
    }

    fn changes(&self) -> Option<Moment> {
        self.tree.changes.filter(|_| self.calendar.live)
    }

    fn access(&mut self, m: Metrics, title: &str) -> Option<accesskit::TreeUpdate> {
        let focus = if self.palette.is_some() { Some(PALETTE) } else { self.focus };
        Some(crate::access::update(&self.tree, m, title, focus))
    }

    fn act(&mut self, node: u64, action: accesskit::Action) -> bool {
        let Some(i) = self.tree.find(Id(node)) else { return false };
        match action {
            accesskit::Action::Click => self.activate(i, Mods::default()),
            accesskit::Action::Focus => {
                self.focus_on(Id(node), false);
                true
            }
            _ => false,
        }
    }

    fn input(&mut self, input: Input, frame: &mut Frame) -> bool {
        match input {
            Input::Moved { x, .. } => {
                if self.reading_drag
                    && let Some(i) = self.readable(frame)
                {
                    let head = (self.tree.nodes[i as usize].id, self.read_at(frame, i, 1).start);
                    let Some(reading) = &mut self.reading else { return false };
                    let moved = reading.head != head;
                    reading.head = head;
                    return moved;
                }
                if let Some(id) = self.drag
                    && let Some(i) = self.tree.find(id)
                {
                    let at = self.caret_at(frame.text, frame.metrics, i, x);
                    let c = self.carets.entry(id).or_default();
                    c.caret = at;
                    return true;
                }
                let hover = self.hovered(frame);
                if hover == self.hover {
                    return false;
                }
                self.hover = hover;
                self.status();
                true
            }
            Input::Left => {
                if self.hover.is_none() {
                    return false;
                }
                self.hover = None;
                self.status();
                true
            }
            Input::Press { button: Button::Left, clicks } => {
                if self.inspecting
                    && let Some((x, y)) = frame.pointer
                {
                    let (col, row) = frame.metrics.cell_at(x, y);
                    return self.inspect_at(col, row);
                }
                self.pressed = self.hovered(frame);
                if self.pressed.is_none() {
                    let had = self.reading.take().is_some();
                    let Some(i) = self.readable(frame) else { return had };
                    let (id, range) = (self.tree.nodes[i as usize].id, self.read_at(frame, i, clicks));
                    self.reading = Some(Reading { anchor: (id, range.start), head: (id, range.end) });
                    self.reading_drag = clicks == 1;
                    return true;
                }
                let Some(i) = self.pressed.and_then(|id| self.tree.find(id)) else { return false };
                let n = self.tree.nodes[i as usize];
                if n.kind == Kind::Field && n.path.is_some() {
                    let (x, _) = frame.pointer.unwrap_or_default();
                    self.reading = None;
                    let at = self.caret_at(frame.text, frame.metrics, i, x);
                    let before = self.caret(i);
                    let was = self.focus == Some(n.id);
                    self.focus_on(n.id, false);
                    let value = self.tree.str(n.text);
                    let caret = match clicks {
                        2 => field::word_at(value, at),
                        3.. => Caret { caret: value.len(), anchor: 0 },
                        _ if frame.mods.shift && was => Caret { caret: at, anchor: before.anchor },
                        _ => Caret::at(at),
                    };
                    self.carets.insert(n.id, caret);
                    self.drag = (clicks == 1).then_some(n.id);
                }
                true
            }
            Input::Release { button: Button::Left } => {
                self.drag = None;
                if std::mem::take(&mut self.reading_drag) && self.reading.is_some_and(|r| r.anchor == r.head) {
                    self.reading = None;
                }
                let Some(pressed) = self.pressed.take() else { return false };
                if self.hovered(frame) != Some(pressed) {
                    return true;
                }
                if let Some(i) = self.tree.find(pressed) {
                    self.activate(i, frame.mods);
                }
                true
            }
            Input::Wheel { dy, .. } => {
                let Some((x, y)) = frame.pointer else { return false };
                let (col, row) = frame.metrics.cell_at(x, y);
                let by = -(dy / frame.metrics.cell_h).round() as i32;
                if self.sheet.is_some() {
                    return by != 0 && self.scroll_help(by);
                }
                by != 0 && self.scroll(col, row, by)
            }
            Input::Key(event) => self.key(&event, frame.mods),
            Input::Ime(Ime::Preedit(text, _)) => {
                if text == self.composing {
                    return false;
                }
                self.composing = text;
                self.stale = true;
                true
            }
            Input::Ime(Ime::Commit(text)) => {
                self.composing.clear();
                self.stale = true;
                self.typed(&text)
            }
            Input::Appearance(_) => {
                self.kept.reset();
                true
            }
            Input::Woken => {
                if self.pin() {
                    self.stale = true;
                }
                self.collect();
                self.slice(Slice::Time(SLICE));
                true
            }
            Input::Clock => {
                self.stale = true;
                true
            }
            Input::Resized => {
                if self.sheet.is_some() {
                    self.stale = true;
                }
                true
            }
            _ => false,
        }
    }
}

fn read_keymap<A: App>(source: &str, file: &str) -> Result<(Keymap, Option<Said>), Said> {
    let keymap = Keymap::parse(source).map_err(|e| Said::Failure(format!("{file} has a mistake on {e}")))?;
    let known = |name: &str| Builtin::find(name).is_some() || A::Cmd::entries().iter().any(|e| e.name == name && !e.internal);
    let unknown = keymap.bindings.iter().find(|b| !known(&b.command)).map(|b| b.command.clone());
    let said = unknown.map(|c| Said::Failure(format!("{file} names “{c}”, which is not a command")));
    Ok((keymap, said))
}

const HELP_CHROME: i32 = 9;
const HELP_KEYS: i32 = 20;
const HELP_LABEL: i32 = 16;
const HELP_DATE: i32 = 18;
const DONE_KEPT: usize = 500;

const MOST_TIMES: u32 = 9999;

fn done_as(label: &str) -> String {
    match label.strip_prefix('“').and_then(|l| l.strip_suffix('”')) {
        Some(plain) => plain.to_string(),
        None if label == "typing" => "Typed".into(),
        None => label.to_string(),
    }
}

const KEYMAP: &str = "keymap.txt";

fn watch_keys(file: &std::path::Path, waker: Waker) -> Option<(notify::RecommendedWatcher, mpsc::Receiver<()>)> {
    use notify::Watcher;
    let folder = file.parent()?;
    std::fs::create_dir_all(folder).ok()?;
    let (send, receive) = mpsc::channel();
    let name = file.file_name()?.to_owned();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else { return };
        if !event.kind.is_access() && event.paths.iter().any(|p| p.file_name() == Some(name.as_os_str())) && send.send(()).is_ok() {
            waker.wake();
        }
    })
    .ok()?;
    watcher.watch(folder, notify::RecursiveMode::NonRecursive).ok()?;
    Some((watcher, receive))
}

fn title(name: &str) -> String {
    let mut words = name.replace('-', " ");
    if let Some(first) = words.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    words
}

fn matches(query: &str, label: &str) -> bool {
    let mut label = label.chars().flat_map(char::to_lowercase);
    query.chars().filter(|c| !c.is_whitespace()).flat_map(char::to_lowercase).all(|q| label.any(|l| l == q))
}

fn label<F, C: Commands<F>>(text: &mut String, cmd: &C, facts: &F, form: Form) -> Range32 {
    let start = text.len() as u32;
    cmd.label(facts, form, text);
    Range32 { start, end: text.len() as u32 }
}

/// Where this app keeps its facts: a folder named after the program, in the operating system's place for app data.
pub fn folder() -> std::path::PathBuf {
    let name = std::env::current_exe().ok().and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned())).unwrap_or_else(|| "app".into());
    dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("Graticule").join(name)
}

fn shortened(type_name: &str) -> String {
    let mut out = String::new();
    let mut word = String::new();
    for c in type_name.chars().chain(std::iter::once(' ')) {
        if c.is_alphanumeric() || c == '_' || c == ':' {
            word.push(c);
        } else {
            out.push_str(word.rsplit("::").next().unwrap_or_default());
            word.clear();
            out.push(c);
        }
    }
    out.trim_end().to_string()
}
