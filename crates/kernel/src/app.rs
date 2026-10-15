use std::any::Any;
use std::fmt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use ledger::{FactPath, Facts, Field, Key, Outcome, Value};

use crate::command::{Available, Commands};
use crate::live::{Feed, Live};
use crate::message::Message;
use crate::ui::Ui;

/// An app: its facts, the commands that change them, what can happen right now and how the facts look.
pub trait App: Facts + Send {
    type Cmd: Commands<Self>;

    /// Whether a command can run right now, and if not, why. Name every command; a catch-all would let a new command become available by accident.
    fn available(&self, cmd: &Self::Cmd) -> Available;

    /// What each command does. It runs only after `available` said yes, and returns `Outcome::Nothing` if the command can no longer apply.
    fn update(&mut self, cmd: Self::Cmd, cx: &mut Cx<Self::Cmd>) -> Outcome;

    /// Starts the work that runs as long as the app does, such as a feed that writes a live value. Every job shows by name in the status line while it runs.
    fn start(&self, _jobs: &mut Jobs<Self::Cmd>) {}

    /// How the facts look. It is a pure function of the facts: it reads them and never writes back.
    fn view(&self, ui: &mut Ui<Self::Cmd>);

    /// The named states Specimen shows.
    fn states() -> States<Self> {
        States::new()
    }

    /// The app's translations, each a language's tag and its Fluent source: `&[("de", include_str!("../locales/de.ftl"))]`. The one for the system's language is used; English is written in the source and needs none. `bindery messages` writes the English template and says what each translation lacks.
    fn translations() -> crate::language::Translations {
        &[]
    }
}

/// Named sets of facts, so Specimen can show every screen in every state that matters.
pub struct States<F> {
    pub list: Vec<(&'static str, F)>,
}

impl<F> States<F> {
    pub fn new() -> Self {
        Self { list: Vec::new() }
    }

    pub fn add(mut self, name: &'static str, facts: F) -> Self {
        self.list.push((name, facts));
        self
    }
}

impl<F> Default for States<F> {
    fn default() -> Self {
        Self::new()
    }
}

/// A moment in time, in milliseconds since 1970. Formatted one way everywhere.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Moment(pub i64);

impl Moment {
    pub fn now() -> Self {
        Moment(SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64))
    }
}

impl Field for Moment {
    ledger::register!(Moment);
    fn same(&self, other: &Self) -> bool {
        self == other
    }
    fn value(&self) -> Value {
        Value::Int(self.0)
    }
    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Int(i) => Some(Moment(*i)),
            _ => None,
        }
    }
}
impl ledger::Document for Moment {}

/// A file path the user sees. On screen a long one is shortened in the middle.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Path(pub PathBuf);

impl Path {
    pub fn as_path(&self) -> &std::path::Path {
        &self.0
    }

    /// The path in at most `cols` characters, shortened in the middle so its start and its file name stay readable.
    pub fn shortened(&self, cols: usize) -> String {
        let full = self.0.display().to_string();
        let n = full.chars().count();
        if n <= cols || cols < 5 {
            return full;
        }
        let keep = cols - 1;
        let tail = keep / 2 + keep % 2;
        let head = keep - tail;
        let start: String = full.chars().take(head).collect();
        let end: String = full.chars().skip(n - tail).collect();
        format!("{start}…{end}")
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

impl fmt::Debug for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl Field for Path {
    ledger::register!(Path);
    fn same(&self, other: &Self) -> bool {
        self == other
    }
    fn value(&self) -> Value {
        Value::Str(self.0.to_string_lossy().into_owned())
    }
    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Str(s) => Some(Path(PathBuf::from(s))),
            _ => None,
        }
    }
}
impl ledger::Document for Path {}

/// Something that went wrong, with a message for the user.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Failure {
    pub message: String,
}

impl Failure {
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Field for Failure {
    ledger::register!(Failure);
    fn same(&self, other: &Self) -> bool {
        self == other
    }
    fn value(&self) -> Value {
        Value::Str(self.message.clone())
    }
    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Str(s) => Some(Failure::new(s.clone())),
            _ => None,
        }
    }
}
impl ledger::Document for Failure {}

/// What makes a file's contents from a snapshot of the facts, run in the background.
pub type Contents = Box<dyn FnOnce(&dyn Any) -> String + Send>;

/// Work asked for as data. Its result comes back as a command.
pub enum Effect<C> {
    SaveAs { name: String, write: Contents, reply: fn(Result<Path, Failure>) -> C },
    Write { path: Path, write: Contents, reply: fn(Result<Path, Failure>) -> C },
    Open { target: String },
    Copy { text: String },
    Job { name: String, work: ledger::Work, reply: Reply<C> },
    Stream { name: String, field: u16, run: Box<dyn FnOnce(Box<dyn Any + Send>) + Send> },
}

/// What turns a job's finished result back into a command.
pub type Reply<C> = Box<dyn FnOnce(Box<dyn Any + Send>) -> Option<C> + Send>;

/// Work that runs while the app does, started in `App::start`.
pub struct Jobs<C> {
    pub(crate) effects: Vec<Effect<C>>,
}

impl<C: 'static> Jobs<C> {
    pub(crate) fn new() -> Self {
        Self { effects: Vec::new() }
    }

    /// Runs `source` on its own thread with the writing end of a `#[live]` field, for as long as it keeps going: `jobs.stream(t!("Market data"), market::connect(), Ticker::quotes)`.
    pub fn stream<F: Field, T: Send + Sync + 'static>(&mut self, name: impl Into<Message>, source: impl FnOnce(Feed<T>) + Send + 'static, field: FactPath<F, Live<T>>) {
        let Some(field) = field.erased().first_field() else { return };
        let run = move |feed: Box<dyn Any + Send>| {
            if let Ok(feed) = feed.downcast::<Feed<T>>() {
                source(*feed);
            }
        };
        self.effects.push(Effect::Stream { name: name.into().to_string(), field, run: Box::new(run) });
    }

    /// Runs `work` once in the background, like `Cx::job`.
    pub fn job<T: Send + 'static>(&mut self, name: impl Into<Message>, work: impl FnOnce() -> T + Send + 'static, reply: fn(T) -> C) {
        self.effects.push(job(name.into(), work, reply));
    }
}

fn job<C: 'static, T: Send + 'static>(name: Message, work: impl FnOnce() -> T + Send + 'static, reply: fn(T) -> C) -> Effect<C> {
    let work: ledger::Work = Box::new(move || Box::new(work()));
    let reply: Reply<C> = Box::new(move |done: Box<dyn Any + Send>| done.downcast::<T>().ok().map(|done| reply(*done)));
    Effect::Job { name: name.to_string(), work, reply }
}

impl<C> fmt::Debug for Effect<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Effect::SaveAs { name, .. } => write!(f, "save “{name}” as…"),
            Effect::Write { path, .. } => write!(f, "write {}", path.0.display()),
            Effect::Open { target } => write!(f, "open {target}"),
            Effect::Copy { text } => write!(f, "copy {} characters", text.chars().count()),
            Effect::Job { name, .. } => write!(f, "run “{name}”"),
            Effect::Stream { name, .. } => write!(f, "stream “{name}”"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Said {
    Note(Message),
    Failure(String),
}

/// What `update` may ask for: new keys, the time, effects and messages. Everything that isn't the same on every run comes from here and is written into the command log, so replaying the log gives the same facts.
pub struct Cx<C> {
    pub(crate) replica: u64,
    pub(crate) counter: u64,
    pub(crate) now: Moment,
    pub(crate) seed: u64,
    pub(crate) effects: Vec<Effect<C>>,
    pub(crate) said: Vec<Said>,
    pub(crate) handed: Vec<Value>,
}

impl<C> Cx<C> {
    pub fn new(replica: u64, counter: u64, now: Moment, seed: u64) -> Self {
        Self { replica, counter, now, seed, effects: Vec::new(), said: Vec::new(), handed: Vec::new() }
    }

    /// A new key that no device has handed out before.
    pub fn key<T>(&mut self) -> Key<T> {
        self.counter += 1;
        let key = Key::new(self.replica, self.counter);
        self.handed.push(Value::Key(key.raw()));
        key
    }

    pub fn now(&mut self) -> Moment {
        self.handed.push(Value::Int(self.now.0));
        self.now
    }

    pub fn random(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.handed.push(Value::UInt(self.seed));
        self.seed
    }

    /// Shows a message in the message line. Saying something doesn't change the facts.
    pub fn say(&mut self, message: Message) -> Outcome {
        self.said.push(Said::Note(message));
        Outcome::Nothing
    }

    pub fn say_failure(&mut self, failure: Failure) -> Outcome {
        self.said.push(Said::Failure(failure.message));
        Outcome::Nothing
    }

    /// Asks where to save a file, makes its contents from a snapshot of the facts in the background and writes it. The result comes back as `reply`.
    pub fn save_as<F: 'static>(&mut self, name: &str, write: fn(&F) -> String, reply: fn(Result<Path, Failure>) -> C) {
        let make = move |facts: &dyn Any| facts.downcast_ref::<F>().map(write).unwrap_or_default();
        self.effects.push(Effect::SaveAs { name: name.to_string(), write: Box::new(make), reply });
    }

    /// Makes a file's contents from a snapshot of the facts in the background and writes it to a path the app already has, making its folder if needed. The result comes back as `reply`.
    pub fn write<F: 'static>(&mut self, path: Path, write: impl FnOnce(&F) -> String + Send + 'static, reply: fn(Result<Path, Failure>) -> C) {
        let make = move |facts: &dyn Any| facts.downcast_ref::<F>().map(write).unwrap_or_default();
        self.effects.push(Effect::Write { path, write: Box::new(make), reply });
    }

    pub fn open(&mut self, target: impl Into<String>) {
        self.effects.push(Effect::Open { target: target.into() });
    }

    pub fn copy(&mut self, text: impl Into<String>) {
        self.effects.push(Effect::Copy { text: text.into() });
    }

    /// Runs `work` in the background, shown by `name` in the status line while it runs. Its result comes back as `reply`. Give it the parts of the facts it needs; Ledger's types share their storage, so cloning a long list costs little.
    pub fn job<T: Send + 'static>(&mut self, name: impl Into<Message>, work: impl FnOnce() -> T + Send + 'static, reply: fn(T) -> C)
    where
        C: 'static,
    {
        self.effects.push(job(name.into(), work, reply));
    }

    pub fn effects(&self) -> &[Effect<C>] {
        &self.effects
    }

    pub fn said(&self) -> &[Said] {
        &self.said
    }
}
