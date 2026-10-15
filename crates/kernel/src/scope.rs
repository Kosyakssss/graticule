//! SC-01 Scope's recorder, in every app: a fixed ring of what each frame cost and why it was drawn, streamed to Scope's window over a local connection when one is attached.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::Instant;

const RING: usize = 1024;

/// What made a frame necessary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Cause {
    #[default]
    Launch,
    Key,
    Typing,
    Pointer,
    Click,
    Wheel,
    Resize,
    Appearance,
    Woken,
    Assistive,
    Uncovered,
    Clock,
    Scope,
}

impl Cause {
    pub const ALL: [Cause; 13] = [
        Cause::Launch,
        Cause::Key,
        Cause::Typing,
        Cause::Pointer,
        Cause::Click,
        Cause::Wheel,
        Cause::Resize,
        Cause::Appearance,
        Cause::Woken,
        Cause::Assistive,
        Cause::Uncovered,
        Cause::Clock,
        Cause::Scope,
    ];

    /// The cause in words, as Scope shows it.
    pub fn name(self) -> &'static str {
        match self {
            Cause::Launch => "launch",
            Cause::Key => "a key",
            Cause::Typing => "typing in another script",
            Cause::Pointer => "the pointer moving",
            Cause::Click => "a click",
            Cause::Wheel => "scrolling",
            Cause::Resize => "a resize",
            Cause::Appearance => "the look changing: the appearance, the theme, the style or how dates are written",
            Cause::Woken => "a thread waking it",
            Cause::Assistive => "an assistive tool",
            Cause::Uncovered => "the window being uncovered",
            Cause::Clock => "a date written as how long ago changing",
            Cause::Scope => "Scope asking",
        }
    }

    fn code(self) -> usize {
        Cause::ALL.iter().position(|c| *c == self).unwrap_or(0)
    }
}

/// One frame: when it was drawn, why, and how long each stage took, in microseconds. `update` is the time spent on the input that led to it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stamp {
    pub frame: u64,
    pub at: u64,
    pub cause: Cause,
    pub update: u32,
    pub view: u32,
    pub layout: u32,
    pub paint: u32,
    pub gpu: u32,
    pub after: u32,
    pub total: u32,
}

impl Stamp {
    /// The stamp as one line of Scope's wire format.
    pub fn line(&self) -> String {
        format!("frame\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", self.frame, self.at, self.cause.code(), self.update, self.view, self.layout, self.paint, self.gpu, self.after, self.total)
    }

    /// Reads a line written by `line`.
    pub fn read(line: &str) -> Option<Stamp> {
        let mut f = line.strip_prefix("frame\t")?.trim_end().split('\t');
        let mut n = || f.next().and_then(|v| v.parse::<u64>().ok());
        let (frame, at, cause) = (n()?, n()?, n()?);
        let mut us = || n().map(|v| v as u32);
        Some(Stamp { frame, at, cause: *Cause::ALL.get(cause as usize)?, update: us()?, view: us()?, layout: us()?, paint: us()?, gpu: us()?, after: us()?, total: us()? })
    }
}

/// What Scope draws over the app's own window: the grid's corners, and the outline of every box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Overlay {
    pub grid: bool,
    pub boxes: bool,
}

/// What Scope asks the attached app for. The app answers after its next frame, in lines of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ask {
    /// Every box in the tree: what it is, where, the line of code that made it and why it is that size.
    Boxes,
    /// Every command, available or not and why, with its keys.
    Commands,
    /// The app's facts as a tree.
    Facts,
    /// The effects the app asked for lately and the jobs still running, with their timings.
    Effects,
    /// Draw this over the window.
    Overlay(Overlay),
}

impl Ask {
    /// The ask as one line of the wire, from Scope to the app.
    pub fn line(&self) -> String {
        match self {
            Ask::Boxes => "ask\tboxes\n".into(),
            Ask::Commands => "ask\tcommands\n".into(),
            Ask::Facts => "ask\tfacts\n".into(),
            Ask::Effects => "ask\teffects\n".into(),
            Ask::Overlay(o) => format!("ask\toverlay\t{}\t{}\n", u8::from(o.grid), u8::from(o.boxes)),
        }
    }

    /// Reads a line written by `line`.
    pub fn read(line: &str) -> Option<Ask> {
        let mut f = line.strip_prefix("ask\t")?.trim_end().split('\t');
        match f.next()? {
            "boxes" => Some(Ask::Boxes),
            "commands" => Some(Ask::Commands),
            "facts" => Some(Ask::Facts),
            "effects" => Some(Ask::Effects),
            "overlay" => Some(Ask::Overlay(Overlay { grid: f.next()? == "1", boxes: f.next()? == "1" })),
            _ => None,
        }
    }
}

/// The facts as lines of the wire: `fact <depth> <name> <value>` for every field, item and variant, at most `most` of them, between `facts begin` and `facts done`.
pub fn facts_lines(facts: &ledger::Value, most: usize) -> String {
    let mut out = String::from("facts\tbegin\n");
    let mut left = most;
    fact_lines(facts, 0, "facts", &mut out, &mut left);
    if left == 0 {
        out.push_str("fact\t0\t…\tmore than can be shown\n");
    }
    out.push_str("facts\tdone\n");
    out
}

fn fact_lines(value: &ledger::Value, depth: usize, name: &str, out: &mut String, left: &mut usize) {
    use ledger::Value as V;
    if *left == 0 {
        return;
    }
    *left -= 1;
    let one_line = |s: &str| s.chars().map(|c| if c.is_control() { ' ' } else { c }).take(120).collect::<String>();
    let said = match value {
        V::Unit => String::new(),
        V::Bool(b) => if *b { "yes" } else { "no" }.into(),
        V::Int(n) => n.to_string(),
        V::UInt(n) => n.to_string(),
        V::Float(f) => f.to_string(),
        V::Str(s) => format!("“{}”", one_line(s)),
        V::Key(k) => format!("{k:x}"),
        V::None => "none".into(),
        V::Some(inner) => {
            *left += 1;
            return fact_lines(inner, depth, name, out, left);
        }
        V::List(items) if items.len() == 1 => "1 item".into(),
        V::List(items) => format!("{} items", items.len()),
        V::Map(_) => String::new(),
        V::Variant(variant, _) => variant.clone(),
    };
    out.push_str(&format!("fact\t{depth}\t{}\t{said}\n", one_line(name)));
    match value {
        V::List(items) => {
            for (i, item) in items.iter().enumerate() {
                fact_lines(item, depth + 1, &format!("{}", i + 1), out, left);
            }
        }
        V::Map(fields) => {
            for (field, v) in fields {
                fact_lines(v, depth + 1, field, out, left);
            }
        }
        V::Variant(_, inner) => match inner.as_ref() {
            V::Unit => {}
            V::Map(fields) => {
                for (field, v) in fields {
                    fact_lines(v, depth + 1, field, out, left);
                }
            }
            inner => fact_lines(inner, depth + 1, "with", out, left),
        },
        _ => {}
    }
}

enum Out {
    Stamp(Stamp),
    Text(String),
}

/// The time spent in the stages a face measures itself, in microseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stages {
    pub view: u32,
    pub layout: u32,
    pub paint: u32,
}

struct Shared {
    ring: Vec<Stamp>,
    next: usize,
    launch: Vec<(&'static str, f32)>,
}

/// The recorder. The main thread writes a stamp a frame into a fixed ring and, while Scope is attached, offers it to a bounded channel that never blocks or allocates; a thread of its own waits for Scope and does the sending.
pub struct Recorder {
    started: Instant,
    frame: u64,
    cause: Option<Cause>,
    update: u32,
    shared: Arc<Mutex<Shared>>,
    attached: Arc<AtomicBool>,
    send: SyncSender<Out>,
    file: Option<PathBuf>,
}

/// Where running apps say how Scope can reach them: one file per process, holding the app's name and its port on this machine.
pub fn folder() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("Graticule").join("Scope"))
}

impl Recorder {
    /// Starts recording. Scope can attach once the app is listening on a port on this machine only, which it says in `folder()`. What Scope asks is handed to `asked`, on a thread of its own.
    pub fn new(app: &str, started: Instant, asked: impl Fn(Ask) + Send + Sync + 'static) -> Recorder {
        let shared = Arc::new(Mutex::new(Shared { ring: Vec::with_capacity(RING), next: 0, launch: Vec::new() }));
        let attached = Arc::new(AtomicBool::new(false));
        let (send, receive) = sync_channel(RING);
        let mut recorder = Recorder { started, frame: 0, cause: None, update: 0, shared, attached, send, file: None };
        if let Ok(listener) = TcpListener::bind(("127.0.0.1", 0))
            && let Ok(port) = listener.local_addr().map(|a| a.port())
            && let Some(folder) = folder()
            && std::fs::create_dir_all(&folder).is_ok()
        {
            let file = folder.join(std::process::id().to_string());
            if std::fs::write(&file, format!("{app}\n{port}\n")).is_ok() {
                recorder.file = Some(file);
                let (shared, attached) = (recorder.shared.clone(), recorder.attached.clone());
                let asked: Arc<dyn Fn(Ask) + Send + Sync> = Arc::new(asked);
                let _ = std::thread::Builder::new().name("scope".into()).spawn(move || serve(listener, shared, attached, receive, asked));
            }
        }
        recorder
    }

    /// Keeps the launch's stages, which Scope is sent when it attaches.
    pub fn launched(&mut self, stages: &[(&'static str, f32)]) {
        if let Ok(mut s) = self.shared.lock() {
            s.launch = stages.to_vec();
        }
    }

    /// Notes input that asked for a frame, and the time spent on it. The first cause since the last frame is the one recorded.
    pub fn wanted(&mut self, cause: Cause, spent: u32) {
        self.cause.get_or_insert(cause);
        self.update = self.update.saturating_add(spent);
    }

    /// Notes time spent on input that didn't ask for a frame.
    pub fn spent(&mut self, spent: u32) {
        self.update = self.update.saturating_add(spent);
    }

    /// Records a drawn frame.
    pub fn drawn(&mut self, stages: Stages, gpu: u32, after: u32, total: u32) {
        let stamp = Stamp {
            frame: self.frame,
            at: self.started.elapsed().as_micros() as u64,
            cause: self.cause.take().unwrap_or_default(),
            update: std::mem::take(&mut self.update),
            view: stages.view,
            layout: stages.layout,
            paint: stages.paint,
            gpu,
            after,
            total,
        };
        self.frame += 1;
        if let Ok(mut s) = self.shared.lock() {
            if s.ring.len() < RING {
                s.ring.push(stamp);
            } else {
                let at = s.next;
                s.ring[at] = stamp;
            }
            s.next = (s.next + 1) % RING;
        }
        if self.attached.load(Ordering::Relaxed) {
            let _ = self.send.try_send(Out::Stamp(stamp));
        }
    }

    /// Sends Scope the answer to something it asked, if it is still attached.
    pub fn answer(&self, text: String) {
        if self.attached.load(Ordering::Relaxed) {
            let _ = self.send.try_send(Out::Text(text));
        }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        if let Some(file) = &self.file {
            let _ = std::fs::remove_file(file);
        }
    }
}

fn serve(listener: TcpListener, shared: Arc<Mutex<Shared>>, attached: Arc<AtomicBool>, receive: Receiver<Out>, asked: Arc<dyn Fn(Ask) + Send + Sync>) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        if let Ok(reading) = stream.try_clone() {
            let asked = asked.clone();
            let _ = std::thread::Builder::new().name("scope asks".into()).spawn(move || {
                for line in BufReader::new(reading).lines() {
                    let Ok(line) = line else { break };
                    if let Some(ask) = Ask::read(&line) {
                        asked(ask);
                    }
                }
                asked(Ask::Overlay(Overlay::default()));
            });
        }
        while receive.try_recv().is_ok() {}
        let backlog = match shared.lock() {
            Ok(s) => {
                let (old, new) = s.ring.split_at(s.next.min(s.ring.len()));
                let mut lines: String = s.launch.iter().map(|(stage, ms)| format!("launch\t{stage}\t{ms}\n")).collect();
                lines.extend(new.iter().chain(old).map(Stamp::line));
                lines
            }
            Err(_) => String::new(),
        };
        attached.store(true, Ordering::Relaxed);
        stream_to(stream, &backlog, &receive);
        attached.store(false, Ordering::Relaxed);
    }
}

fn stream_to(mut stream: TcpStream, backlog: &str, receive: &Receiver<Out>) {
    if stream.write_all(backlog.as_bytes()).is_err() {
        return;
    }
    while let Ok(out) = receive.recv() {
        let written = match out {
            Out::Stamp(stamp) => stream.write_all(stamp.line().as_bytes()),
            Out::Text(text) => stream.write_all(text.as_bytes()),
        };
        if written.is_err() {
            return;
        }
    }
}
