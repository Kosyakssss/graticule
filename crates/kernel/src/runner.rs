use std::fmt::Write as _;

use ledger::{Change, Choose, Edit, Transaction};

use crate::app::{App, Cx, Moment, Said};
use crate::command::{Available, Commands, Form};
use crate::components::Look;
use crate::hit::Hits;
use crate::kept::Kept;
use crate::layout::Layout;
use crate::metrics::Metrics;
use crate::paint::Painter;
use crate::shell::{Button, Face, Frame, Input, Seen};
use crate::tree::{Id, Kind, NONE, Tree};
use crate::ui::{Scroll, Scrolls, Ui};

/// Runs an app in a window: builds its tree from the facts, lays it out, keeps its quads and turns input into commands.
pub struct Runner<A: App> {
    pub facts: A,
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
}

impl<A: App> Runner<A> {
    pub fn new(facts: A) -> Self {
        Self {
            facts,
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
            said: None,
            status: NONE,
            stack: Vec::new(),
            widths: Vec::new(),
            replica: std::process::id() as u64 | 1 << 40,
            counter: 0,
            gesture: 0,
            minimum: None,
        }
    }

    /// Builds the tree from the facts: the app's view, then the window's own lines, then every label and availability filled in.
    pub fn build(&mut self) {
        let root = self.tree.root();
        A::view(&self.facts, &mut Ui::new(&mut self.tree, &self.facts, &self.scrolls, root));
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
        for (e, entry) in A::Cmd::entries().iter().enumerate() {
            if entry.internal {
                continue;
            }
            if let Some(cmd) = A::Cmd::make(e, None) {
                let a = self.tree.add(bar, Kind::Action, Some(Id::ROOT.mix(4).mix(e as u64)), std::panic::Location::caller());
                self.tree.nodes[a as usize].cmd = self.tree.commands.len() as u32;
                self.tree.commands.push(cmd);
            }
        }
        self.resolve();
        self.status();
        self.tree.reindex();
        self.stale = false;
        self.laid = false;
    }

    fn resolve(&mut self) {
        for i in 0..self.tree.nodes.len() {
            let n = self.tree.nodes[i];
            if n.cmd == NONE {
                continue;
            }
            let short = matches!(n.kind, Kind::Button | Kind::Link) && self.tree.in_row(i as u32);
            let cmd = &self.tree.commands[n.cmd as usize];
            let available = self.facts.available(cmd);
            let name = label(&mut self.tree.text, cmd, &self.facts, Form::Long);
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
        let at = self.hover.or(self.focus).and_then(|id| self.tree.find(id));
        let range = match at.map(|i| self.tree.nodes[i as usize]) {
            Some(n) if !n.name.is_empty() => {
                let (name, reason) = (n.name, n.reason);
                let start = self.tree.text.len() as u32;
                let label = self.tree.str(name).to_string();
                let why = self.tree.str(reason).to_string();
                let _ = if why.is_empty() { write!(self.tree.text, "{label}") } else { write!(self.tree.text, "{label}: {why}") };
                crate::tree::Range32 { start, end: self.tree.text.len() as u32 }
            }
            _ => crate::tree::Range32::default(),
        };
        self.tree.nodes[self.status as usize].text = range;
        self.tree.hash_node(self.status as usize);
    }

    fn layout(&mut self, p: &mut Painter) {
        for _ in 0..2 {
            let mut layout = Layout::new(p.text, p.m, std::mem::take(&mut self.stack), std::mem::take(&mut self.widths));
            layout.run(&mut self.tree);
            self.minimum = Some(layout.minimum(&self.tree));
            (self.stack, self.widths) = layout.finish();
            let mut moved = false;
            for n in &self.tree.nodes {
                if let Kind::List { .. } = n.kind {
                    let s = self.scrolls.entry(n.id).or_default();
                    if s.rows != n.area.rows {
                        s.rows = n.area.rows;
                        moved = true;
                    }
                }
            }
            if !moved {
                break;
            }
            self.build();
        }
        self.hits.fill(&self.tree, p.m.cols(), p.m.rows());
        self.laid = true;
    }

    fn look(&self) -> impl Fn(Id) -> Look + use<A> {
        let (hover, pressed, focus) = (self.hover, self.pressed, self.focus);
        move |id| Look { hovered: hover == Some(id), pressed: pressed == Some(id) && hover == Some(id), focused: focus == Some(id), caret: u32::MAX }
    }

    /// Runs a command the way every entry point does: only if it is available, inside a transaction.
    pub fn run(&mut self, cmd: A::Cmd) -> bool {
        if let Available::No(reason) = self.facts.available(&cmd) {
            self.said = Some(Said::Note(reason));
            self.stale = true;
            return true;
        }
        let mut cx = Cx::new(self.replica, self.counter, Moment::now(), self.counter ^ 0x9e37_79b9_7f4a_7c15);
        let tx = Transaction::begin();
        let _ = self.facts.update(cmd, &mut cx);
        let _ops = tx.end();
        self.counter = cx.counter;
        if let Some(said) = cx.said.pop() {
            self.said = Some(said);
        }
        self.stale = true;
        true
    }

    fn activate(&mut self, i: u32) -> bool {
        let n = self.tree.nodes[i as usize];
        match n.kind {
            Kind::Button | Kind::Link | Kind::Action | Kind::Check { .. } if n.cmd != NONE => {
                let cmd = self.tree.commands[n.cmd as usize].clone();
                self.run(cmd)
            }
            Kind::ListRow { key, .. } => {
                let Some(path) = n.path else { return false };
                self.gesture += 1;
                let edit = Edit::new(path, Change::Select(Choose::Only(key)), self.gesture);
                match A::Cmd::edit(edit) {
                    Some(cmd) => self.run(cmd),
                    None => false,
                }
            }
            _ => false,
        }
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
        if key.is_none() {
            s.anchor = None;
        }
        self.stale = true;
        true
    }

    /// Clicks whatever acts at this cell, as the mouse would.
    pub fn click(&mut self, col: i32, row: i32) -> bool {
        match self.hits.at(col, row) {
            Some(i) => self.activate(i),
            None => false,
        }
    }

    fn hovered(&self, frame: &Frame) -> Option<Id> {
        let (x, y) = frame.pointer?;
        let (col, row) = frame.metrics.cell_at(x, y);
        self.hits.at(col, row).map(|i| self.tree.nodes[i as usize].id)
    }
}

impl<A: App> Face for Runner<A> {
    fn paint(&mut self, p: &mut Painter, _: Seen) {
        if self.metrics != Some(p.m) {
            self.metrics = Some(p.m);
            self.kept.reset();
            self.laid = false;
        }
        if self.stale {
            self.build();
        }
        if !self.laid {
            self.layout(p);
        }
        let look = self.look();
        let mut kept = std::mem::take(&mut self.kept);
        kept.update(p, &self.tree, look);
        self.kept = kept;
    }

    fn kept(&mut self) -> Option<&mut Kept> {
        Some(&mut self.kept)
    }

    fn minimum(&self) -> Option<(i32, i32)> {
        self.minimum
    }

    fn access(&mut self, m: Metrics, title: &str) -> Option<accesskit::TreeUpdate> {
        Some(crate::access::update(&self.tree, m, title, self.focus))
    }

    fn act(&mut self, node: u64, action: accesskit::Action) -> bool {
        let Some(i) = self.tree.find(Id(node)) else { return false };
        match action {
            accesskit::Action::Click => self.activate(i),
            accesskit::Action::Focus => {
                self.focus = Some(Id(node));
                self.status();
                true
            }
            _ => false,
        }
    }

    fn input(&mut self, input: Input, frame: &mut Frame) -> bool {
        match input {
            Input::Moved { .. } | Input::Left => {
                let hover = if input == Input::Left { None } else { self.hovered(frame) };
                if hover == self.hover {
                    return false;
                }
                self.hover = hover;
                self.status();
                true
            }
            Input::Press { button: Button::Left, .. } => {
                self.pressed = self.hovered(frame);
                if let Some(i) = self.pressed.and_then(|id| self.tree.find(id))
                    && matches!(self.tree.nodes[i as usize].kind, Kind::Field)
                {
                    self.focus = self.pressed;
                }
                self.pressed.is_some()
            }
            Input::Release { button: Button::Left } => {
                let Some(pressed) = self.pressed.take() else { return false };
                if self.hovered(frame) != Some(pressed) {
                    return true;
                }
                match self.tree.find(pressed) {
                    Some(i) => {
                        self.activate(i);
                        true
                    }
                    None => true,
                }
            }
            Input::Wheel { dy, .. } => {
                let Some((x, y)) = frame.pointer else { return false };
                let (col, row) = frame.metrics.cell_at(x, y);
                let by = -(dy / frame.metrics.cell_h).round() as i32;
                by != 0 && self.scroll(col, row, by)
            }
            Input::Appearance(_) => {
                self.kept.reset();
                true
            }
            Input::Resized | Input::Woken => true,
            _ => false,
        }
    }
}

fn label<F, C: Commands<F>>(text: &mut String, cmd: &C, facts: &F, form: Form) -> crate::tree::Range32 {
    let start = text.len() as u32;
    cmd.label(facts, form, text);
    crate::tree::Range32 { start, end: text.len() as u32 }
}
