use std::any::Any;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::panic::Location;

use ledger::{FactPath, Field, Key, List, Record, Selection, Text};

use crate::message::Message;
use crate::tree::{Id, Kind, ListInfo, NONE, Span, Tone, Tree};

const CHECK_MIN: i32 = 12;

/// Where a list is scrolled to: the row at its top, by key so rows arriving above don’t move it, and how many rows it showed last frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Scroll {
    pub anchor: Option<u128>,
    pub index: u32,
    pub rows: i32,
}

pub type Scrolls = HashMap<Id, Scroll>;

/// A command bound in full, like `Cmd::Add`.
pub struct Whole;

/// A command waiting for the key of the row it is in, like `Cmd::Toggle`.
pub struct Of<T>(PhantomData<T>);

/// What a control can be given: a whole command, or one that takes the row's key. `S` is the key type of the row the control is in, so binding a command that wants another kind of key doesn't compile.
#[diagnostic::on_unimplemented(
    message = "`{Self}` can't be bound here",
    note = "outside a list row, give a whole command like `Cmd::Add`; inside a row, a command that takes the row's key, like `Cmd::Remove`"
)]
pub trait Bind<C, S, M> {
    fn bind(self, subject: Option<u128>) -> Option<C>;
}

impl<C, S> Bind<C, S, Whole> for C {
    fn bind(self, _: Option<u128>) -> Option<C> {
        Some(self)
    }
}

impl<C, T, F: Fn(Key<T>) -> C> Bind<C, Key<T>, Of<T>> for F {
    fn bind(self, subject: Option<u128>) -> Option<C> {
        subject.map(|raw| self(Key::from_raw(raw)))
    }
}

/// What a view is given to describe the screen. Every method places a component in cells under the current one; nothing here speaks in pixels.
pub struct Ui<'a, C, S = ()> {
    tree: &'a mut Tree<C>,
    facts: &'a dyn Any,
    scrolls: &'a Scrolls,
    parent: u32,
    subject: Option<u128>,
    keys: PhantomData<fn() -> S>,
}

impl<'a, C> Ui<'a, C, ()> {
    pub fn new(tree: &'a mut Tree<C>, facts: &'a dyn Any, scrolls: &'a Scrolls, parent: u32) -> Self {
        Self { tree, facts, scrolls, parent, subject: None, keys: PhantomData }
    }
}

/// A component just placed, to finish describing: `ui.field(…).on_submit(Cmd::Add)`.
pub struct Placed<'u, 'a, C, S> {
    ui: &'u mut Ui<'a, C, S>,
    node: u32,
}

impl<C, S> Placed<'_, '_, C, S> {
    /// The command Return runs in this field.
    pub fn on_submit<M>(self, cmd: impl Bind<C, S, M>) -> Self {
        if let Some(cmd) = cmd.bind(self.ui.subject) {
            self.ui.tree.nodes[self.node as usize].submit = self.ui.tree.commands.len() as u32;
            self.ui.tree.commands.push(cmd);
        }
        self
    }

    /// How wide it is, in columns: `.width(28.min(20))`, `.width(Fill)`.
    pub fn width(self, span: impl Into<Span>) -> Self {
        self.ui.tree.nodes[self.node as usize].span = span.into();
        self
    }
}

impl<'a, C, S> Ui<'a, C, S> {
    fn child<S2>(&mut self, parent: u32, subject: Option<u128>) -> Ui<'_, C, S2> {
        Ui { tree: &mut *self.tree, facts: self.facts, scrolls: self.scrolls, parent, subject, keys: PhantomData }
    }

    fn add(&mut self, kind: Kind, at: &'static Location<'static>) -> u32 {
        let i = self.tree.add(self.parent, kind, None, at);
        self.tree.nodes[i as usize].subject = self.subject;
        i
    }

    fn command<M>(&mut self, node: u32, cmd: impl Bind<C, S, M>) {
        if let Some(cmd) = cmd.bind(self.subject) {
            self.tree.nodes[node as usize].cmd = self.tree.commands.len() as u32;
            self.tree.commands.push(cmd);
        } else {
            self.tree.nodes[node as usize].enabled = false;
        }
    }

    fn facts<F: 'static>(&self) -> Option<&'a F> {
        self.facts.downcast_ref::<F>()
    }

    /// The app's page: its title at the top, then its components top to bottom.
    #[track_caller]
    pub fn page(&mut self, title: Message, f: impl FnOnce(&mut Ui<'_, C, S>)) {
        let i = self.add(Kind::Page, Location::caller());
        let r = self.tree.put(title.as_str());
        self.tree.nodes[i as usize].text = r;
        let subject = self.subject;
        f(&mut self.child(i, subject));
    }

    /// Components left to right. When their minimum widths don't fit, they stack.
    #[track_caller]
    pub fn row(&mut self, f: impl FnOnce(&mut Ui<'_, C, S>)) {
        let i = self.add(Kind::Row, Location::caller());
        let subject = self.subject;
        f(&mut self.child(i, subject));
    }

    /// Columns side by side, each made with `column`. When their minimum widths don't fit, they stack.
    #[track_caller]
    pub fn columns(&mut self, f: impl FnOnce(&mut Ui<'_, C, S>)) {
        let i = self.add(Kind::Columns, Location::caller());
        let subject = self.subject;
        f(&mut self.child(i, subject));
    }

    /// One column of `columns`, `width` columns wide: `28.min(20)` or `Fill.min(40)`.
    #[track_caller]
    pub fn column(&mut self, width: impl Into<Span>, f: impl FnOnce(&mut Ui<'_, C, S>)) {
        let i = self.add(Kind::Column, Location::caller());
        self.tree.nodes[i as usize].span = width.into();
        let subject = self.subject;
        f(&mut self.child(i, subject));
    }

    /// Text, in the proportional face.
    #[track_caller]
    pub fn text(&mut self, text: impl AsRef<str>) {
        self.text_as(text.as_ref(), false, Tone::Ink, Location::caller());
    }

    /// Text in the monospace face, for data.
    #[track_caller]
    pub fn mono(&mut self, text: impl AsRef<str>) {
        self.text_as(text.as_ref(), true, Tone::Ink, Location::caller());
    }

    /// Text that says less than the text around it.
    #[track_caller]
    pub fn muted(&mut self, text: impl AsRef<str>) {
        self.text_as(text.as_ref(), false, Tone::Muted, Location::caller());
    }

    /// Text that heads what follows it, in the strong weight.
    #[track_caller]
    pub fn strong(&mut self, text: impl AsRef<str>) {
        let i = self.add(Kind::Text { mono: false, tone: Tone::Ink, strong: true }, Location::caller());
        let r = self.tree.put(text.as_ref());
        self.tree.nodes[i as usize].text = r;
    }

    fn text_as(&mut self, text: &str, mono: bool, tone: Tone, at: &'static Location<'static>) {
        let i = self.add(Kind::Text { mono, tone, strong: false }, at);
        let r = self.tree.put(text);
        self.tree.nodes[i as usize].text = r;
    }

    /// A button for a command. Its label comes from the command and says what it will do; when the command can't run, the button says so and the status line says why.
    #[track_caller]
    pub fn button<M>(&mut self, cmd: impl Bind<C, S, M>) -> Placed<'_, 'a, C, S> {
        let i = self.add(Kind::Button, Location::caller());
        self.command(i, cmd);
        Placed { ui: self, node: i }
    }

    /// A link for a command: a lighter way to offer it, for commands that are one of many on a row.
    #[track_caller]
    pub fn link<M>(&mut self, cmd: impl Bind<C, S, M>) -> Placed<'_, 'a, C, S> {
        let i = self.add(Kind::Link, Location::caller());
        self.command(i, cmd);
        Placed { ui: self, node: i }
    }

    /// A checkbox showing `on`, labelled with `label`; clicking it runs `cmd`.
    #[track_caller]
    pub fn check<M>(&mut self, label: impl AsRef<str>, on: bool, cmd: impl Bind<C, S, M>) -> Placed<'_, 'a, C, S> {
        let i = self.add(Kind::Check { on }, Location::caller());
        let r = self.tree.put(label.as_ref());
        self.tree.nodes[i as usize].text = r;
        self.tree.nodes[i as usize].span = crate::tree::Fill.min(CHECK_MIN);
        self.command(i, cmd);
        Placed { ui: self, node: i }
    }

    /// A one-line field that types into the `Text` at `path`. Its caret and selection are the field's own; each change arrives in `update` as an `Edit`.
    #[track_caller]
    pub fn field<F: Field>(&mut self, path: FactPath<F, Text>, placeholder: Message) -> Placed<'_, 'a, C, S> {
        let erased = path.erased();
        let id = Id::ROOT.mix(erased.hash64());
        let i = self.tree.add(self.parent, Kind::Field, Some(id), Location::caller());
        let node = &mut self.tree.nodes[i as usize];
        node.subject = self.subject;
        node.path = Some(erased);
        let value = self.facts::<F>().and_then(|f| path.get(f)).map_or("", |t| t.as_str());
        let r = self.tree.put(value);
        self.tree.nodes[i as usize].text = r;
        let r = self.tree.put(placeholder.as_str());
        self.tree.nodes[i as usize].name = r;
        self.tree.nodes[i as usize].span = crate::tree::Fill.min(16);
        Placed { ui: self, node: i }
    }

    /// A list of `items`, scrolled by whole rows, whose selection lives at `selection`. `row` describes one item; it is called only for the rows that show, so a list of fifty million costs what a screenful does.
    #[track_caller]
    pub fn list<T: Record, F: Field>(&mut self, items: &List<T>, selection: FactPath<F, Selection<T>>, mut row: impl FnMut(&mut Ui<'_, C, Key<T>>, &T)) {
        let at = Location::caller();
        let i = self.add(Kind::List { count: items.len() as u32, first: 0, shown: 0 }, at);
        let erased = selection.erased();
        self.tree.nodes[i as usize].path = Some(erased);
        let id = self.tree.nodes[i as usize].id;
        let scroll = self.scrolls.get(&id).copied().unwrap_or_default();
        let len = items.len();
        let first = scroll.anchor.and_then(|k| items.position(Key::from_raw(k))).unwrap_or_else(|| (scroll.index as usize).min(len.saturating_sub(1)));
        let want = (scroll.rows.max(1) + 1) as usize;
        let chosen = self.facts::<F>().and_then(|f| selection.get(f));
        let mut shown = 0u32;
        for (n, index) in (first..len).take(want).enumerate() {
            let Some(item) = items.get(index) else { break };
            let key = item.key();
            let raw = key.raw();
            let (selected, focused) = chosen.map_or((false, false), |s| (s.contains(key), s.focused() == Some(key)));
            let row_id = id.mix(raw as u64).mix((raw >> 64) as u64);
            let r = self.tree.add(i, Kind::ListRow { key: raw, index: index as u32, selected, focused }, Some(row_id), at);
            self.tree.nodes[r as usize].subject = Some(raw);
            self.tree.nodes[r as usize].path = Some(erased);
            row(&mut self.child(r, Some(raw)), item);
            shown = n as u32 + 1;
        }
        self.tree.nodes[i as usize].kind = Kind::List { count: len as u32, first: first as u32, shown };
        let focused = chosen.and_then(|s| s.focused());
        let focus_index = focused.and_then(|k| items.position(k));
        let near = |d: isize| focus_index.and_then(|p| p.checked_add_signed(d)).and_then(|p| items.get(p)).map(|t| t.key().raw());
        self.tree.nodes[i as usize].list = self.tree.lists.len() as u32;
        self.tree.lists.push(ListInfo {
            node: i,
            item: std::any::TypeId::of::<T>(),
            items: items.path(),
            selection: erased,
            count: len as u32,
            chosen: chosen.map_or(0, |s| s.len() as u32),
            focused: focused.map(Key::raw),
            anchor: chosen.and_then(|s| s.anchor()).map(Key::raw),
            focus_index: focus_index.map(|p| p as u32),
            before: near(-1),
            after: near(1),
            first: items.first().map(|t| t.key().raw()),
            last: items.last().map(|t| t.key().raw()),
            keys: keys::<F, T>,
            nth: nth::<F, T>,
        });
    }

    /// What the page shows when there is nothing yet: a sentence, and the command that makes something.
    #[track_caller]
    pub fn empty<M>(&mut self, message: Message, cmd: impl Bind<C, S, M>) {
        let i = self.add(Kind::Empty, Location::caller());
        let r = self.tree.put(message.as_str());
        self.tree.nodes[i as usize].text = r;
        let subject = self.subject;
        self.child::<S>(i, subject).button(cmd);
    }
}

impl<C> Tree<C> {
    /// The row a node sits in, if any, and its key.
    pub fn row_of(&self, i: u32) -> Option<u32> {
        self.ancestor(i, |n| matches!(n.kind, Kind::ListRow { .. }))
    }

    pub fn in_row(&self, i: u32) -> bool {
        let parent = self.nodes[i as usize].parent;
        parent != NONE && self.row_of(parent).is_some()
    }
}

fn keys<F: Field, T: Record>(facts: &dyn Any, items: ledger::ErasedPath, selection: ledger::ErasedPath, span: Option<(u128, u128)>) -> Vec<u128> {
    let Some(facts) = facts.downcast_ref::<F>() else { return Vec::new() };
    let Some(list) = facts.reach(items.steps()).and_then(|l| l.downcast_ref::<List<T>>()) else { return Vec::new() };
    match span {
        Some((from, to)) => {
            let (Some(a), Some(b)) = (list.position(Key::from_raw(from)), list.position(Key::from_raw(to))) else { return Vec::new() };
            (a.min(b)..=a.max(b)).filter_map(|i| list.get(i)).map(|t| t.key().raw()).collect()
        }
        None => {
            let Some(chosen) = facts.reach(selection.steps()).and_then(|s| s.downcast_ref::<Selection<T>>()) else { return Vec::new() };
            let mut keys: Vec<(usize, u128)> = chosen.iter().filter_map(|k| list.position(k).map(|p| (p, k.raw()))).collect();
            keys.sort_unstable();
            keys.into_iter().map(|(_, k)| k).collect()
        }
    }
}

fn nth<F: Field, T: Record>(facts: &dyn Any, items: ledger::ErasedPath, index: u32) -> Option<u128> {
    let list = facts.downcast_ref::<F>()?.reach(items.steps())?.downcast_ref::<List<T>>()?;
    list.get(index as usize).map(|t| t.key().raw())
}
