use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::panic::Location;

use ledger::ErasedPath;

use crate::builtin::Builtin;
use crate::grid::Area;
use crate::ink::Ink;

pub const NONE: u32 = u32::MAX;

/// A component's stable id: from the line of code that made it, the key of its row, and a count when one line makes several.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(pub u64);

impl Id {
    pub const ROOT: Id = Id(0xcbf2_9ce4_8422_2325);

    pub fn mix(self, n: u64) -> Id {
        let mut h = self.0;
        for b in n.to_le_bytes() {
            h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
        }
        Id(h ^ (h >> 29))
    }

    pub fn at(self, location: &Location<'_>, count: u32) -> Id {
        let file = location.file().bytes().fold(0u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
        self.mix(file).mix((location.line() as u64) << 32 | location.column() as u64).mix(count as u64)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Width {
    Auto,
    Cols(i32),
    Fill(u16),
}

/// A width in columns with a minimum: `Cols(28).min(20)`, `Fill.min(40)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    pub width: Width,
    pub min: i32,
    pub max: i32,
}

#[allow(non_upper_case_globals)]
pub const Fill: Span = Span { width: Width::Fill(1), min: 0, max: i32::MAX };

pub const fn cols(n: i32) -> Span {
    Span { width: Width::Cols(n), min: n, max: n }
}

impl Span {
    pub const fn min(self, min: i32) -> Span {
        Span { min, ..self }
    }

    pub const fn max(self, max: i32) -> Span {
        Span { max, ..self }
    }

    pub const fn weight(self, weight: u16) -> Span {
        Span { width: Width::Fill(weight), ..self }
    }
}

impl From<i32> for Span {
    fn from(n: i32) -> Span {
        cols(n)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tone {
    Ink,
    Muted,
    Faint,
    Danger,
    Ok,
    Warn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Root,
    Page,
    Row,
    Column,
    Columns,
    List { count: u32, first: u32, shown: u32 },
    ListRow { key: u128, index: u32, selected: bool, focused: bool },
    Field,
    Check { on: bool },
    Link,
    Button,
    Text { mono: bool, tone: Tone, strong: bool },
    Empty,
    Bar,
    Action,
    Status,
    Note,
    Palette,
    Choice { chosen: bool },
    Swatch { ink: Ink },
}

impl Kind {
    pub fn acts(&self) -> bool {
        matches!(self, Kind::Field | Kind::Check { .. } | Kind::Link | Kind::Button | Kind::Action | Kind::Choice { .. } | Kind::ListRow { .. })
    }

    pub fn container(&self) -> bool {
        matches!(self, Kind::Root | Kind::Page | Kind::Row | Kind::Column | Kind::Columns | Kind::List { .. } | Kind::ListRow { .. })
    }

    pub fn role(&self) -> &'static str {
        match self {
            Kind::Root => "window",
            Kind::Page => "page",
            Kind::Row | Kind::Columns => "group",
            Kind::Column => "column",
            Kind::List { .. } => "list",
            Kind::ListRow { .. } => "list item",
            Kind::Field => "text field",
            Kind::Check { .. } => "checkbox",
            Kind::Link => "link",
            Kind::Button => "button",
            Kind::Text { .. } => "text",
            Kind::Empty => "empty",
            Kind::Bar => "toolbar",
            Kind::Action => "button",
            Kind::Status => "status",
            Kind::Note => "alert",
            Kind::Palette => "dialog",
            Kind::Choice { .. } => "option",
            Kind::Swatch { .. } => "swatch",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Range32 {
    pub start: u32,
    pub end: u32,
}

impl Range32 {
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Node {
    pub id: Id,
    pub parent: u32,
    pub first: u32,
    pub last: u32,
    pub next: u32,
    pub kind: Kind,
    pub span: Span,
    pub area: Area,
    pub text: Range32,
    pub name: Range32,
    pub reason: Range32,
    pub cmd: u32,
    pub submit: u32,
    pub enabled: bool,
    pub subject: Option<u128>,
    pub path: Option<ErasedPath>,
    pub hash: u64,
    pub keys: Range32,
    pub builtin: Option<Builtin>,
    pub host: u32,
    pub list: u32,
    pub many: u32,
    pub at: &'static Location<'static>,
}

impl Node {
    fn new(id: Id, parent: u32, kind: Kind, at: &'static Location<'static>) -> Self {
        Node {
            id,
            parent,
            first: NONE,
            last: NONE,
            next: NONE,
            kind,
            span: Span { width: Width::Auto, min: 0, max: i32::MAX },
            area: Area::default(),
            text: Range32::default(),
            name: Range32::default(),
            reason: Range32::default(),
            cmd: NONE,
            submit: NONE,
            enabled: true,
            subject: None,
            path: None,
            hash: 0,
            keys: Range32::default(),
            builtin: None,
            host: NONE,
            list: NONE,
            many: 0,
            at,
        }
    }
}

/// Reads keys out of the facts a list was built from: the chosen keys in list order, or the keys from one to another.
pub type Keys = fn(&dyn std::any::Any, ErasedPath, ErasedPath, Option<(u128, u128)>) -> Vec<u128>;

/// Reads the key of the row at a place in a list, counting from the top.
pub type Nth = fn(&dyn std::any::Any, ErasedPath, u32) -> Option<u128>;

/// What the kernel knows about a list it can move around in and choose from, without knowing the type of its items.
#[derive(Clone, Copy, Debug)]
pub struct ListInfo {
    pub node: u32,
    pub item: std::any::TypeId,
    pub items: ErasedPath,
    pub selection: ErasedPath,
    pub count: u32,
    pub chosen: u32,
    pub focused: Option<u128>,
    pub anchor: Option<u128>,
    pub focus_index: Option<u32>,
    pub before: Option<u128>,
    pub after: Option<u128>,
    pub first: Option<u128>,
    pub last: Option<u128>,
    pub keys: Keys,
    pub nth: Nth,
}

/// The flat tree a view builds: every component on screen, in building order, with stable ids. Its vectors keep their capacity, so building it again allocates nothing in steady state.
pub struct Tree<C> {
    pub nodes: Vec<Node>,
    pub text: String,
    pub commands: Vec<C>,
    counts: HashMap<(u64, u32), u32>,
    pub index: HashMap<Id, u32>,
    pub lists: Vec<ListInfo>,
}

impl<C> Default for Tree<C> {
    fn default() -> Self {
        Self { nodes: Vec::new(), text: String::new(), commands: Vec::new(), counts: HashMap::new(), index: HashMap::new(), lists: Vec::new() }
    }
}

impl<C> Tree<C> {
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.lists.clear();
        self.text.clear();
        self.commands.clear();
        self.counts.clear();
        self.index.clear();
    }

    pub fn root(&mut self) -> u32 {
        self.clear();
        self.nodes.push(Node::new(Id::ROOT, NONE, Kind::Root, Location::caller()));
        0
    }

    pub fn str(&self, r: Range32) -> &str {
        &self.text[r.start as usize..r.end as usize]
    }

    pub fn put(&mut self, s: &str) -> Range32 {
        let start = self.text.len() as u32;
        self.text.push_str(s);
        Range32 { start, end: self.text.len() as u32 }
    }

    pub fn write(&mut self, f: impl FnOnce(&mut String)) -> Range32 {
        let start = self.text.len() as u32;
        f(&mut self.text);
        Range32 { start, end: self.text.len() as u32 }
    }

    pub fn add(&mut self, parent: u32, kind: Kind, id: Option<Id>, at: &'static Location<'static>) -> u32 {
        let id = id.unwrap_or_else(|| {
            let parent_id = self.nodes[parent as usize].id;
            let site = parent_id.at(at, 0).0;
            let count = self.counts.entry((site, parent)).or_insert(0);
            let n = *count;
            *count += 1;
            parent_id.at(at, n)
        });
        let index = self.nodes.len() as u32;
        self.nodes.push(Node::new(id, parent, kind, at));
        let p = &mut self.nodes[parent as usize];
        if p.first == NONE {
            p.first = index;
        } else {
            let last = p.last;
            self.nodes[last as usize].next = index;
        }
        self.nodes[parent as usize].last = index;
        index
    }

    pub fn children(&self, parent: u32) -> Children<'_, C> {
        Children { tree: self, at: self.nodes[parent as usize].first }
    }

    pub fn reindex(&mut self) {
        self.index.clear();
        for (i, n) in self.nodes.iter().enumerate() {
            self.index.insert(n.id, i as u32);
        }
    }

    pub fn find(&self, id: Id) -> Option<u32> {
        self.index.get(&id).copied()
    }

    /// The nearest node at or above `i` that acts when clicked.
    pub fn actor(&self, mut i: u32) -> Option<u32> {
        while i != NONE {
            if self.nodes[i as usize].kind.acts() {
                return Some(i);
            }
            i = self.nodes[i as usize].parent;
        }
        None
    }

    pub fn ancestor(&self, mut i: u32, pred: impl Fn(&Node) -> bool) -> Option<u32> {
        while i != NONE {
            if pred(&self.nodes[i as usize]) {
                return Some(i);
            }
            i = self.nodes[i as usize].parent;
        }
        None
    }

    pub fn hash_node(&mut self, i: usize) {
        let n = &self.nodes[i];
        let mut h = std::collections::hash_map::DefaultHasher::new();
        n.kind.hash(&mut h);
        n.span.hash(&mut h);
        n.enabled.hash(&mut h);
        n.builtin.hash(&mut h);
        n.host.hash(&mut h);
        n.many.hash(&mut h);
        self.text[n.keys.start as usize..n.keys.end as usize].hash(&mut h);
        self.text[n.reason.start as usize..n.reason.end as usize].hash(&mut h);
        self.text[n.text.start as usize..n.text.end as usize].hash(&mut h);
        self.text[n.name.start as usize..n.name.end as usize].hash(&mut h);
        let hash = h.finish();
        self.nodes[i].hash = hash;
    }
}

pub struct Children<'a, C> {
    tree: &'a Tree<C>,
    at: u32,
}

impl<C> Iterator for Children<'_, C> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        if self.at == NONE {
            return None;
        }
        let i = self.at;
        self.at = self.tree.nodes[i as usize].next;
        Some(i)
    }
}
