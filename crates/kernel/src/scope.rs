//! SC-01 Scope's recorder, in every app: a fixed ring of what each frame cost and why it was drawn, streamed to Scope's window over a local connection when one is attached.

use std::io::Write;
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
}

impl Cause {
    pub const ALL: [Cause; 11] = [Cause::Launch, Cause::Key, Cause::Typing, Cause::Pointer, Cause::Click, Cause::Wheel, Cause::Resize, Cause::Appearance, Cause::Woken, Cause::Assistive, Cause::Uncovered];

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
            Cause::Appearance => "the appearance changing",
            Cause::Woken => "a thread waking it",
            Cause::Assistive => "an assistive tool",
            Cause::Uncovered => "the window being uncovered",
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
    send: SyncSender<Stamp>,
    file: Option<PathBuf>,
}

/// Where running apps say how Scope can reach them: one file per process, holding the app's name and its port on this machine.
pub fn folder() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("Graticule").join("Scope"))
}

impl Recorder {
    /// Starts recording. Scope can attach once the app is listening on a port on this machine only, which it says in `folder()`.
    pub fn new(app: &str, started: Instant) -> Recorder {
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
                let _ = std::thread::Builder::new().name("scope".into()).spawn(move || serve(listener, shared, attached, receive));
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
            let _ = self.send.try_send(stamp);
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

fn serve(listener: TcpListener, shared: Arc<Mutex<Shared>>, attached: Arc<AtomicBool>, receive: Receiver<Stamp>) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
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

fn stream_to(mut stream: TcpStream, backlog: &str, receive: &Receiver<Stamp>) {
    if stream.write_all(backlog.as_bytes()).is_err() {
        return;
    }
    while let Ok(stamp) = receive.recv() {
        if stream.write_all(stamp.line().as_bytes()).is_err() {
            return;
        }
    }
}
