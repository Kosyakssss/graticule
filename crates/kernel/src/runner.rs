use std::any::Any;
use std::collections::HashMap;
use std::fmt::Write as _;

use ledger::{Change, Choose, Edit, ErasedPath, Op, Transaction};
use winit::event::{Ime, KeyEvent};

use crate::app::{App, Cx, Moment, Said};
use crate::builtin::Builtin;
use crate::command::{Available, Commands, Form};
use crate::components::Look;
use crate::field::{self, Action, Caret};
use crate::hit::Hits;
use crate::kept::Kept;
use crate::keys::{self, Chord, Key, Keymap, Lookup};
use crate::layout::Layout;
use crate::message::Message;
use crate::metrics::Metrics;
use crate::paint::Painter;
use crate::shell::{Button, Face, Frame, Input, Mods, Seen};
use crate::text::Text;
use crate::tree::{Id, Kind, ListInfo, NONE, Range32, Tree};
use crate::ui::{Scroll, Scrolls, Ui};

const CHOICES: usize = 12;
const PALETTE: Id = Id(0x9a1e_77e5_0000_0001);

#[derive(Clone, Debug, Default)]
struct Palette {
    query: String,
    caret: Caret,
    chosen: usize,
    shown: usize,
}

/// A command named by a key, the palette or the bar, once its subjects are filled in.
enum Target<C> {
    Builtin(Builtin),
    One(C),
    Many(Vec<C>),
    Refused(Option<C>, Message),
}

/// The list commands act on right now, and the keys they act on.
type Here = Option<(ListInfo, Vec<u128>)>;

/// Runs an app in a window: builds its tree from the facts, lays it out, keeps its quads and turns input into commands.
pub struct Runner<A: App> {
    pub facts: A,
    /// Every transaction that changed more than a selection, oldest first, as the operations it made.
    pub steps: Vec<Vec<Op>>,
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
    composing: bool,
    drag: Option<Id>,
    pending: Vec<Chord>,
    keymap: Keymap,
    palette: Option<Palette>,
    clipboard: Option<arboard::Clipboard>,
    reveal: bool,
    ime: bool,
    repairing: bool,
}

impl<A: App> Runner<A> {
    pub fn new(mut facts: A) -> Self {
        ledger::Field::attach(&mut facts, ErasedPath::ROOT);
        let (keymap, said) = read_keymap::<A>();
        Self {
            facts,
            steps: Vec::new(),
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
            replica: std::process::id() as u64 | 1 << 40,
            counter: 0,
            gesture: 0,
            minimum: None,
            carets: HashMap::new(),
            kill: String::new(),
            typing: None,
            composing: false,
            drag: None,
            pending: Vec::new(),
            keymap,
            palette: None,
            clipboard: None,
            reveal: false,
            ime: false,
            repairing: false,
        }
    }

    /// Builds the tree from the facts: the app's view, the palette when it is open, then the window's own lines, then every label and availability filled in.
    pub fn build(&mut self) {
        let root = self.tree.root();
        A::view(&self.facts, &mut Ui::new(&mut self.tree, &self.facts, &self.scrolls, root));
        self.tree.reindex();
        for list in &self.tree.lists {
            if let Some(at) = list.focus_index {
                self.scrolls.entry(self.tree.nodes[list.node as usize].id).or_default().focus = Some(at);
            }
        }
        self.follow();
        let here = self.here();
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
        if self.pending.is_empty() {
            self.build_bar(bar, &here);
        } else {
            self.build_which(bar, &here);
        }
        self.tree.reindex();
        self.resolve();
        self.status();
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
            let keys = self.keymap.keys_for(entry.name).map(keys::label).unwrap_or_default();
            self.tree.nodes[a as usize].keys = self.tree.put(&keys);
            self.put_target(a, target, entry.name);
        }
        for b in [Builtin::Undo, Builtin::Redo, Builtin::Palette] {
            let a = self.tree.add(bar, Kind::Action, Some(Id::ROOT.mix(5).mix(b as u64)), std::panic::Location::caller());
            let keys = self.builtin_keys(b);
            self.tree.nodes[a as usize].keys = self.tree.put(&keys);
            self.tree.nodes[a as usize].builtin = Some(b);
        }
    }

    fn build_which(&mut self, bar: u32, here: &Here) {
        let mut next: Vec<(Chord, Option<String>)> = Vec::new();
        for (chord, name) in self.keymap.next(&self.pending) {
            if !next.iter().any(|(c, _)| *c == chord) {
                next.push((chord, name.map(str::to_string)));
            }
        }
        let typed = keys::label(&self.pending);
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

    fn build_palette(&mut self, root: u32, here: &Here) {
        let Some(palette) = self.palette.clone() else { return };
        let p = self.tree.add(root, Kind::Palette, Some(Id::ROOT.mix(7)), std::panic::Location::caller());
        let f = self.tree.add(p, Kind::Field, Some(PALETTE), std::panic::Location::caller());
        self.tree.nodes[f as usize].text = self.tree.put(&palette.query);
        self.tree.nodes[f as usize].name = self.tree.put("Type to find a command");
        self.tree.nodes[f as usize].span = crate::tree::Fill;
        let mut found: Vec<(String, Target<A::Cmd>, String)> = Vec::new();
        for (e, entry) in A::Cmd::entries().iter().enumerate() {
            if entry.internal {
                continue;
            }
            let target = self.target(e, here);
            let label = match &target {
                Target::One(c) | Target::Refused(Some(c), _) => c.label_string(&self.facts, Form::Long),
                Target::Many(cs) => cs.first().map(|c| c.label_string(&self.facts, Form::Many(cs.len()))).unwrap_or_default(),
                Target::Refused(None, _) | Target::Builtin(_) => title(entry.name),
            };
            let keys = self.keymap.keys_for(entry.name).map(keys::label).unwrap_or_default();
            found.push((label, target, keys));
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
            Target::One(cmd) => self.push_command(node, cmd),
            Target::Many(mut cmds) => {
                self.tree.nodes[node as usize].many = cmds.len() as u32;
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
                let r = self.tree.put(b.label());
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
            }
        }
    }

    fn here(&self) -> Here {
        let list = self.current_list()?;
        let keys = if list.chosen > 1 { (list.keys)(&self.facts as &dyn Any, list.items, list.selection, None) } else { list.focused.into_iter().collect() };
        Some((list, keys))
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
        let entry = A::Cmd::entries()[e];
        if !entry.subject {
            return match A::Cmd::make(e, None) {
                Some(cmd) => Target::One(cmd),
                None => Target::Refused(None, Message::from("It needs arguments, which only the app can give it")),
            };
        }
        let Some((list, subjects)) = here.as_ref().filter(|(l, _)| entry.subject_type.is_none_or(|t| t() == l.item)) else {
            return Target::Refused(None, Message::from("There is nothing here for it to act on"));
        };
        match subjects.as_slice() {
            [] => Target::Refused(None, Message::from("Nothing is chosen")),
            [one] => A::Cmd::make(e, Some(*one)).map_or(Target::Refused(None, Message::from("Nothing is chosen")), Target::One),
            many if entry.many => Target::Many(many.iter().filter_map(|k| A::Cmd::make(e, Some(*k))).collect()),
            _ => list.focused.and_then(|k| A::Cmd::make(e, Some(k))).map_or(Target::Refused(None, Message::from("Nothing is chosen")), Target::One),
        }
    }

    fn named(&self, name: &str, here: &Here) -> Target<A::Cmd> {
        if let Some(b) = Builtin::find(name) {
            return Target::Builtin(b);
        }
        match A::Cmd::entries().iter().position(|e| e.name == name && !e.internal) {
            Some(e) => self.target(e, here),
            None => Target::Refused(None, Message::plain(format!("No command is called “{name}”"))),
        }
    }

    fn builtin_keys(&self, b: Builtin) -> String {
        match self.keymap.keys_for(b.name()) {
            Some(k) => keys::label(k),
            None => b.convention().first().map(Chord::to_string).unwrap_or_default(),
        }
    }

    fn builtin_available(&self, b: Builtin) -> Available {
        match b {
            Builtin::Undo => Available::No(Message::from("Nothing to undo")),
            Builtin::Redo => Available::No(Message::from("Nothing to redo")),
            Builtin::Up | Builtin::Down | Builtin::ExtendUp | Builtin::ExtendDown | Builtin::First | Builtin::Last | Builtin::SelectAll if self.tree.lists.is_empty() => {
                Available::No(Message::from("There is no list here"))
            }
            _ => Available::Yes,
        }
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
        let mut cx = Cx::new(self.replica, self.counter, Moment::now(), self.counter ^ 0x9e37_79b9_7f4a_7c15);
        let tx = Transaction::begin();
        let _ = self.facts.update(cmd, &mut cx);
        self.commit(tx.end());
        self.counter = cx.counter;
        self.said = cx.said.pop();
        self.stale = true;
        self.repair();
        true
    }

    /// Runs one command over several subjects as one transaction. Subjects it can't take are skipped, and the message line says which and why.
    pub fn run_many(&mut self, cmds: Vec<A::Cmd>) -> bool {
        let mut cx = Cx::new(self.replica, self.counter, Moment::now(), self.counter ^ 0x9e37_79b9_7f4a_7c15);
        let mut skipped: Vec<(String, Message)> = Vec::new();
        let tx = Transaction::begin();
        for cmd in cmds {
            match self.facts.available(&cmd) {
                Available::No(reason) => skipped.push((cmd.label_string(&self.facts, Form::Long), reason)),
                Available::Yes => {
                    let _ = self.facts.update(cmd, &mut cx);
                }
            }
        }
        self.commit(tx.end());
        self.counter = cx.counter;
        self.said = match skipped.first() {
            Some((what, why)) if skipped.len() == 1 => Some(Said::Note(Message::plain(format!("{what} was skipped: {}", why.as_str())))),
            Some((what, why)) => Some(Said::Note(Message::plain(format!("{} were skipped, among them {what}: {}", skipped.len(), why.as_str())))),
            None => cx.said.pop(),
        };
        self.stale = true;
        self.repair();
        true
    }

    fn commit(&mut self, ops: Vec<Op>) {
        if ops.iter().any(|op| !matches!(op, Op::Select { .. })) {
            self.steps.push(ops);
        }
    }

    fn repair(&mut self) {
        if self.repairing {
            return;
        }
        self.repairing = true;
        self.build();
        let gone: Vec<(ListInfo, u128)> = self.tree.lists.iter().filter_map(|l| l.gone.map(|k| (*l, k))).collect();
        for (list, key) in gone {
            self.gesture += 1;
            if let Some(cmd) = A::Cmd::edit(Edit::new(list.selection, Change::Select(Choose::Only(key)), self.gesture)) {
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
        match self.named(name, &here) {
            Target::Builtin(b) => self.builtin(b),
            Target::One(cmd) => {
                self.run(cmd);
                true
            }
            Target::Many(cmds) => self.run_many(cmds),
            Target::Refused(_, reason) => self.say(reason),
        }
    }

    fn fire(&mut self, i: u32) -> bool {
        let n = self.tree.nodes[i as usize];
        if !n.enabled && n.cmd == NONE {
            let why = self.tree.str(n.reason).to_string();
            return !why.is_empty() && self.say(Message::plain(why));
        }
        if let Some(b) = n.builtin {
            return self.builtin(b);
        }
        if n.cmd == NONE {
            return false;
        }
        let cmd = self.tree.commands[n.cmd as usize].clone();
        if n.many > 0
            && let Some(e) = cmd.entry()
            && let Target::Many(cmds) = self.target(e, &self.here())
        {
            return self.run_many(cmds);
        }
        self.run(cmd);
        true
    }

    fn builtin(&mut self, b: Builtin) -> bool {
        if let Available::No(reason) = self.builtin_available(b) {
            return self.say(reason);
        }
        match b {
            Builtin::Palette => {
                self.palette = Some(Palette::default());
                self.pending.clear();
                self.stale = true;
                true
            }
            Builtin::Undo | Builtin::Redo => false,
            Builtin::Next | Builtin::Previous => self.cycle(b == Builtin::Next),
            Builtin::Cancel => self.cancel(),
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
        match A::Cmd::edit(Edit::new(list.selection, Change::Select(choose), self.gesture)) {
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
        if self.palette.take().is_some() || !self.pending.is_empty() {
            self.pending.clear();
            self.stale = true;
            return true;
        }
        if self.focused_field().is_some() {
            self.focus = None;
            self.stale = true;
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
        let text = event.text.as_deref().filter(|_| !self.composing);
        self.press(chord, text)
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
        if self.pending.is_empty()
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
        if self.pending.is_empty()
            && chord.mods == Mods::default()
            && matches!(chord.key, Key::Enter | Key::Char(' '))
            && let Some(i) = self.focused_control()
        {
            return self.fire(i);
        }
        if !self.pending.is_empty() && chord.key == Key::Escape {
            self.pending.clear();
            self.stale = true;
            return true;
        }
        match self.keymap.lookup(&self.pending, chord) {
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
            Lookup::Nothing if !self.pending.is_empty() => {
                self.pending.push(chord);
                let typed = keys::label(&self.pending);
                self.pending.clear();
                return self.say(Message::plain(format!("{typed} does nothing")));
            }
            Lookup::Nothing => {}
        }
        match Builtin::ALL.into_iter().find(|b| b.convention().contains(&chord)) {
            Some(b) => self.builtin(b),
            None => false,
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
        let Some(cmd) = A::Cmd::edit(Edit::new(path, Change::Splice { range, text }, self.gesture)) else { return false };
        if self.run(cmd) {
            self.carets.insert(n.id, Caret::at(caret));
        }
        true
    }

    fn palette_key(&mut self, chord: Chord, text: Option<String>) -> bool {
        let Some(mut palette) = self.palette.take() else { return false };
        self.stale = true;
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
        let before = text.measure(&value[..self.caret(i).caret.min(value.len())], mono, m.cell_h);
        let shift = if self.focus == Some(n.id) { (before - (r.w - m.cell_w * 1.5)).max(0.0) } else { 0.0 };
        let local = x - (r.x + m.cell_w * 0.5) + shift;
        let mut best = (0, f32::MAX);
        for at in value.char_indices().map(|(i, _)| i).chain([value.len()]) {
            let d = (text.measure(&value[..at], mono, m.cell_h) - local).abs();
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

    fn painted(&mut self, frame: &mut Frame) {
        let field = if self.palette.is_some() { self.tree.find(PALETTE) } else { self.focused_field() };
        let want = field.is_some();
        if want != self.ime {
            frame.window.set_ime_allowed(want);
            self.ime = want;
        }
        if let Some(i) = field {
            let r = frame.metrics.rect(self.tree.nodes[i as usize].area);
            frame.window.set_ime_cursor_area(winit::dpi::PhysicalPosition::new(r.x as f64, r.y as f64), winit::dpi::PhysicalSize::new(r.w as f64, r.h as f64));
        }
    }

    fn kept(&mut self) -> Option<&mut Kept> {
        Some(&mut self.kept)
    }

    fn minimum(&self) -> Option<(i32, i32)> {
        self.minimum
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
                self.pressed = self.hovered(frame);
                let Some(i) = self.pressed.and_then(|id| self.tree.find(id)) else { return false };
                let n = self.tree.nodes[i as usize];
                if n.kind == Kind::Field && n.path.is_some() {
                    let (x, _) = frame.pointer.unwrap_or_default();
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
                by != 0 && self.scroll(col, row, by)
            }
            Input::Key(event) => self.key(&event, frame.mods),
            Input::Ime(Ime::Preedit(text, _)) => {
                self.composing = !text.is_empty();
                false
            }
            Input::Ime(Ime::Commit(text)) => {
                self.composing = false;
                self.typed(&text)
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

fn read_keymap<A: App>() -> (Keymap, Option<Said>) {
    match Keymap::parse(A::Cmd::keymap()) {
        Ok(keymap) => {
            let known = |name: &str| Builtin::find(name).is_some() || A::Cmd::entries().iter().any(|e| e.name == name && !e.internal);
            let unknown = keymap.bindings.iter().find(|(_, c)| !known(c)).map(|(_, c)| c.clone());
            let said = unknown.map(|c| Said::Failure(format!("The keymap names “{c}”, which is not a command")));
            (keymap, said)
        }
        Err(e) => (Keymap::default(), Some(Said::Failure(format!("The keymap has a mistake on {e}")))),
    }
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
