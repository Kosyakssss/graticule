//! SC-01, Scope's window: attaches to a Graticule app running on this machine and shows what its frames cost and why they were drawn.

mod app;

use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};

use app::{Cmd, Frame, Running, Scope, Stage};
use kernel::accesskit;
use kernel::ink::{Appearance, Theme};
use kernel::kept::Kept;
use kernel::metrics::Metrics;
use kernel::paint::Painter;
use kernel::runner::Runner;
use kernel::scope::{Stages, Stamp};
use kernel::shell::{Face, Frame as ShellFrame, Input, Seen, Waker};
use ledger::Key;

enum Heard {
    Launch(String, f32),
    Frame(Stamp),
    Closed,
}

/// The connection to the attached app: a thread reads its recorder's lines and wakes the window, at most once until the window has caught up.
struct Line {
    to: Key<Running>,
    heard: Receiver<Heard>,
    pending: Arc<AtomicBool>,
}

struct Window {
    runner: Runner<Scope>,
    line: Option<Line>,
    waker: Option<Waker>,
    looks: u32,
}

impl Window {
    fn new() -> Window {
        let mut window = Window { runner: Runner::new(Scope::default()), line: None, waker: None, looks: 0 };
        window.look();
        window
    }

    fn look(&mut self) {
        let mut found = Vec::new();
        if let Some(entries) = kernel::scope::folder().and_then(|f| std::fs::read_dir(f).ok()) {
            for entry in entries.flatten() {
                let Some(pid) = entry.file_name().to_str().and_then(|n| n.parse::<u64>().ok()) else { continue };
                if pid == std::process::id() as u64 {
                    continue;
                }
                let Ok(said) = std::fs::read_to_string(entry.path()) else { continue };
                let mut lines = said.lines();
                let (Some(name), Some(port)) = (lines.next(), lines.next().and_then(|p| p.parse::<u16>().ok())) else { continue };
                if TcpStream::connect(("127.0.0.1", port)).is_err() {
                    let _ = std::fs::remove_file(entry.path());
                    continue;
                }
                found.push(Running::of(pid, name, port));
            }
        }
        found.sort_by(|a, b| a.name.cmp(&b.name));
        self.runner.run(Cmd::Found(found));
    }

    fn follow(&mut self) -> bool {
        if self.runner.facts.looks != self.looks {
            self.looks = self.runner.facts.looks;
            self.look();
        }
        let wanted = self.runner.facts.attached;
        if self.line.as_ref().map(|l| l.to) == wanted {
            return false;
        }
        self.line = None;
        let Some(to) = wanted else { return true };
        let Some(port) = self.runner.facts.apps.find(to).map(|a| a.port) else { return true };
        let (send, heard) = channel();
        let pending = Arc::new(AtomicBool::new(false));
        let (waker, waiting) = (self.waker.clone(), pending.clone());
        let started = std::thread::Builder::new().name("scope line".into()).spawn(move || {
            let wake = || {
                if !waiting.swap(true, Ordering::AcqRel)
                    && let Some(w) = &waker
                {
                    w.wake();
                }
            };
            if let Ok(stream) = TcpStream::connect(("127.0.0.1", port)) {
                for line in BufReader::new(stream).lines() {
                    let Ok(line) = line else { break };
                    let got = match line.strip_prefix("launch\t").and_then(|l| l.split_once('\t')) {
                        Some((stage, ms)) => Heard::Launch(stage.to_string(), ms.parse().unwrap_or(0.0)),
                        None => match Stamp::read(&line) {
                            Some(stamp) => Heard::Frame(stamp),
                            None => continue,
                        },
                    };
                    if send.send(got).is_err() {
                        return;
                    }
                    wake();
                }
            }
            let _ = send.send(Heard::Closed);
            wake();
        });
        if started.is_ok() {
            self.line = Some(Line { to, heard, pending });
        }
        true
    }

    fn collect(&mut self) -> bool {
        let Some(line) = &self.line else { return false };
        line.pending.store(false, Ordering::Release);
        let (mut stages, mut frames, mut closed) = (Vec::new(), Vec::new(), false);
        while let Ok(heard) = line.heard.try_recv() {
            match heard {
                Heard::Launch(name, ms) => stages.push(Stage::of(stages.len() as u64 + 1, &name, ms)),
                Heard::Frame(stamp) => frames.push(Frame::of(stamp)),
                Heard::Closed => closed = true,
            }
        }
        if frames.len() > app::KEPT {
            frames.drain(..frames.len() - app::KEPT);
        }
        let changed = !stages.is_empty() || !frames.is_empty() || closed;
        if !stages.is_empty() {
            self.runner.run(Cmd::Launched(stages));
        }
        if !frames.is_empty() {
            self.runner.run(Cmd::Arrived(frames));
        }
        if closed {
            self.line = None;
            self.runner.run(Cmd::Lost);
            self.look();
        }
        changed
    }
}

impl Face for Window {
    fn theme(&self, appearance: Appearance) -> Theme {
        self.runner.theme(appearance)
    }

    fn paint(&mut self, painter: &mut Painter, seen: Seen) {
        self.runner.paint(painter, seen);
    }

    fn input(&mut self, input: Input, frame: &mut ShellFrame) -> bool {
        let collected = matches!(input, Input::Woken) && self.collect();
        let redraw = self.runner.input(input, frame);
        self.follow() || redraw || collected
    }

    fn painted(&mut self, frame: &mut ShellFrame) {
        self.runner.painted(frame);
    }

    fn minimum(&self) -> Option<(i32, i32)> {
        self.runner.minimum()
    }

    fn kept(&mut self) -> Option<&mut Kept> {
        self.runner.kept()
    }

    fn quit(&self) -> bool {
        self.runner.quit()
    }

    fn access(&mut self, metrics: Metrics, title: &str) -> Option<accesskit::TreeUpdate> {
        self.runner.access(metrics, title)
    }

    fn act(&mut self, node: u64, action: accesskit::Action) -> bool {
        let acted = self.runner.act(node, action);
        self.follow() || acted
    }

    fn stages(&self) -> Stages {
        self.runner.stages()
    }
}

fn main() {
    let ran = specimen::offscreen::<Scope>(std::env::args().skip(1)).unwrap_or_else(|| {
        kernel::shell::run(kernel::shell::Config::new("Scope"), |waker| {
            let mut window = Window::new();
            window.runner = std::mem::replace(&mut window.runner, Runner::new(Scope::default())).wake_with(waker.clone());
            window.waker = Some(waker);
            window
        })
    });
    if let Err(e) = ran {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
