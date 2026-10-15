use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::panic::Location;

use ledger::ErasedPath;

use crate::builtin::Builtin;
use crate::grid::Area;
use crate::ink::Ink;
use crate::picture::Picture;
use crate::value::{Calendar, Value};

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

    /// Mixed with words, for ids that follow a name rather than a place in the code.
    pub fn named(self, name: &str) -> Id {
        Id(name.bytes().fold(self.0, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3)))
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

/// What a field is for, which decides how it is drawn and what its keys do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Holds {
    /// One line of text.
    Line,
    /// Words to search for, after a magnifying glass; Escape empties it.
    Search,
    /// Several lines, wrapped, at least `rows` tall and growing with the text.
    Lines { rows: u16 },
    /// Text that reads as text and becomes a field when it is clicked or reached with Tab.
    Inline,
    /// A whole number, typed as an expression and stepped with the arrows.
    Number,
    /// A code of `length` characters, one to a box.
    Code { length: u8 },
    /// A tag to add to the tags before it in its row.
    Tags,
    /// A day, typed as a date or in words.
    Date,
    /// A time of day.
    Time,
    /// A color, typed as hex or OKLCH after a swatch of it.
    Color,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Root,
    Page,
    Row,
    Column,
    Columns,
    List { count: u32, first: u32, shown: u32, rows: i32 },
    ListRow { key: u128, index: u32, selected: bool, focused: bool },
    Field { holds: Holds },
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
    Picture { picture: &'static Picture, rows: i32 },
    Value { value: Value },
    Overlay { anchor: Anchor },
    Entry,
    Rule,
    Keys,
    Gauge { filled: u16, meter: bool },
    Placeholder { rows: i32 },
    Tag,
    Avatar,
    Callout { ink: Ink },
    Card,
    Toggle { on: bool },
    IconButton { picture: &'static Picture },
    Group,
    Radio { on: bool },
    Switch { on: bool },
    Fieldset,
    Labelled,
    Tabs,
    Strip,
    Tab { on: bool },
    Place { on: bool },
    Disclosure,
    Opener { open: bool },
    Crumbs,
    PageLink { on: bool },
    Overflow,
    Places,
    Slider { at: u16, label: u8 },
    More,
    Cycle { widest: u8 },
    Browse,
    Chip,
    Range { label: u8 },
    Thumb { at: u16, high: bool },
    Select { widest: u8, many: bool, open: bool },
    Pick { on: bool },
    MenuButton,
    Popover { open: bool },
    Confirm { open: bool },
    Calendar { month: i32 },
    Day { on: bool, today: bool, outside: bool },
    Turn { forward: bool },
    ColorArea { l: u16, c: u16, h: u16 },
    Canvas { rows: i32, pressable: bool },
    Image { rows: i32, width: u32, height: u32 },
    Header,
    Head { sorted: i8, sortable: bool, numeric: bool },
    Cell { numeric: bool, tone: Tone },
    Twig { depth: u16, branch: crate::table::Branch },
    Split,
    Pane,
    Divider,
    Picker,
    Matches { count: u32, first: u32, shown: u32, at: u32 },
    Match { chosen: bool },
    TimeAxis,
    Ruler,
}

/// What an anchored overlay hangs from: the node that opened it, or the cell the pointer was on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Anchor {
    Node(u32),
    Cell { col: i32, row: i32 },
}

impl Kind {
    pub fn acts(&self) -> bool {
        matches!(self, Kind::Field { .. } | Kind::Choice { .. } | Kind::Action | Kind::ListRow { .. } | Kind::Match { .. } | Kind::Entry) || self.control()
    }

    /// Whether it is a control the user presses: a button, a link, a box to tick or a switch to flip.
    pub fn control(&self) -> bool {
        matches!(
            self,
            Kind::Button
                | Kind::Link
                | Kind::Check { .. }
                | Kind::Toggle { .. }
                | Kind::IconButton { .. }
                | Kind::Radio { .. }
                | Kind::Switch { .. }
                | Kind::Tab { .. }
                | Kind::Place { .. }
                | Kind::Opener { .. }
                | Kind::PageLink { .. }
                | Kind::More
                | Kind::Slider { .. }
                | Kind::Cycle { .. }
                | Kind::Browse
                | Kind::Chip
                | Kind::Thumb { .. }
                | Kind::Select { .. }
                | Kind::Pick { .. }
                | Kind::MenuButton
                | Kind::Popover { .. }
                | Kind::Confirm { open: false }
                | Kind::Day { .. }
                | Kind::Turn { .. }
                | Kind::ColorArea { .. }
                | Kind::Canvas { pressable: true, .. }
                | Kind::Head { sortable: true, .. }
                | Kind::Twig { branch: crate::table::Branch::Closed | crate::table::Branch::Open, .. }
                | Kind::Divider
                | Kind::Ruler
        )
    }

    pub fn container(&self) -> bool {
        matches!(
            self,
            Kind::Root
                | Kind::Page
                | Kind::Row
                | Kind::Column
                | Kind::Columns
                | Kind::List { .. }
                | Kind::ListRow { .. }
                | Kind::Overlay { .. }
                | Kind::Confirm { open: true }
                | Kind::Calendar { .. }
                | Kind::Callout { .. }
                | Kind::Card
                | Kind::Group
                | Kind::Fieldset
                | Kind::Labelled
                | Kind::Tabs
                | Kind::Strip
                | Kind::Disclosure
                | Kind::Crumbs
                | Kind::Overflow
                | Kind::Places
                | Kind::Header
                | Kind::Split
                | Kind::Pane
                | Kind::Picker
                | Kind::Matches { .. }
                | Kind::Match { .. }
                | Kind::TimeAxis
        )
    }

    pub fn role(&self) -> &'static str {
        match self {
            Kind::Root => "window",
            Kind::Page => "page",
            Kind::Row | Kind::Columns => "group",
            Kind::Column => "column",
            Kind::List { .. } => "list",
            Kind::Header => "column heads",
            Kind::Head { .. } => "column head",
            Kind::Cell { .. } => "cell",
            Kind::Twig { .. } => "branch",
            Kind::Split => "split",
            Kind::Pane => "pane",
            Kind::Divider => "divider",
            Kind::Picker => "picker",
            Kind::Matches { .. } => "matches",
            Kind::Match { .. } => "match",
            Kind::TimeAxis => "time axis",
            Kind::Ruler => "ruler",
            Kind::ListRow { .. } => "list item",
            Kind::Field { holds: Holds::Lines { .. } } => "text area",
            Kind::Field { holds: Holds::Search } => "search field",
            Kind::Field { holds: Holds::Inline } => "editable text",
            Kind::Field { holds: Holds::Number } => "number field",
            Kind::Field { holds: Holds::Date } => "date field",
            Kind::Field { holds: Holds::Time } => "time field",
            Kind::Calendar { .. } => "calendar",
            Kind::Day { .. } => "day",
            Kind::Turn { .. } => "button",
            Kind::Field { holds: Holds::Color } => "color field",
            Kind::ColorArea { .. } => "color area",
            Kind::Canvas { .. } => "canvas",
            Kind::Image { .. } => "image",
            Kind::Field { holds: Holds::Tags } => "tag field",
            Kind::Field { holds: Holds::Code { .. } } => "code field",
            Kind::Field { .. } => "text field",
            Kind::Check { .. } => "checkbox",
            Kind::Link => "link",
            Kind::Button => "button",
            Kind::Text { strong: true, .. } => "heading",
            Kind::Text { .. } => "text",
            Kind::Empty => "empty",
            Kind::Bar => "toolbar",
            Kind::Action => "button",
            Kind::Status => "status",
            Kind::Note => "alert",
            Kind::Palette => "dialog",
            Kind::Choice { .. } => "option",
            Kind::Swatch { .. } => "swatch",
            Kind::Picture { .. } => "image",
            Kind::Value { .. } => "value",
            Kind::Overlay { .. } => "menu",
            Kind::Entry => "menu item",
            Kind::Rule => "separator",
            Kind::Keys => "keys",
            Kind::Gauge { meter: false, .. } => "progress bar",
            Kind::Gauge { meter: true, .. } => "meter",
            Kind::Placeholder { .. } => "placeholder",
            Kind::Tag => "tag",
            Kind::Avatar => "avatar",
            Kind::Callout { .. } => "callout",
            Kind::Card => "card",
            Kind::Toggle { .. } => "toggle button",
            Kind::IconButton { .. } => "button",
            Kind::Group => "group",
            Kind::Radio { .. } => "radio button",
            Kind::Switch { .. } => "switch",
            Kind::Fieldset => "fieldset",
            Kind::Labelled => "labelled",
            Kind::Tabs => "tabs",
            Kind::Strip => "tab list",
            Kind::Tab { .. } => "tab",
            Kind::Place { .. } => "place",
            Kind::Disclosure => "disclosure",
            Kind::Opener { .. } => "opener",
            Kind::Crumbs => "breadcrumbs",
            Kind::PageLink { .. } => "page link",
            Kind::Overflow => "overflow",
            Kind::Places => "navigation",
            Kind::Slider { .. } => "slider",
            Kind::Cycle { .. } => "cycling choice",
            Kind::Browse => "button",
            Kind::Chip => "removable tag",
            Kind::Range { .. } => "range slider",
            Kind::Thumb { .. } => "slider handle",
            Kind::Select { many: false, .. } => "select",
            Kind::Select { many: true, .. } => "multi-select",
            Kind::Pick { .. } => "option",
            Kind::MenuButton => "menu button",
            Kind::Popover { .. } => "popover button",
            Kind::Confirm { open: false } => "button that asks first",
            Kind::Confirm { open: true } => "question",
            Kind::More => "more",
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
    pub asks: u32,
    pub set: u32,
    /// Whether the node is inside an anchored overlay, drawn and hit above everything else.
    pub over: bool,
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
            asks: NONE,
            set: NONE,
            over: false,
            at,
        }
    }
}

/// Reads keys out of the facts a list was built from: the chosen keys in list order, or the keys from one to another.
pub type Keys = fn(&dyn std::any::Any, ErasedPath, ErasedPath, Option<&crate::table::Ranks>, Option<(u128, u128)>) -> Vec<u128>;

/// Reads the key of the row at a place in a list, counting from the top.
pub type Nth = fn(&dyn std::any::Any, ErasedPath, Option<&crate::table::Ranks>, u32) -> Option<u128>;

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
    /// The order its rows are shown in, by its place in `Tree::orders`, or `NONE` for the list's own.
    pub order: u32,
}

/// The command a file dropped from the system makes.
pub type Takes<C> = fn(crate::app::Path) -> C;

/// The command a row dragged to a new place makes, from its key and the place.
pub type Mover<C> = Box<dyn Fn(u128, usize) -> C>;

/// The flat tree a view builds: every component on screen, in building order, with stable ids. Its vectors keep their capacity, so building it again allocates nothing in steady state.
pub struct Tree<C> {
    pub nodes: Vec<Node>,
    pub text: String,
    pub commands: Vec<C>,
    counts: HashMap<(u64, u32), u32>,
    pub index: HashMap<Id, u32>,
    pub lists: Vec<ListInfo>,
    /// How dates are written in this tree.
    pub calendar: Calendar,
    /// The first moment a date written relative to now would read differently.
    pub changes: Option<crate::app::Moment>,
    /// The values settings' controls set when pressed, by `Node::set`, each with how the change reads in undo and the log.
    pub sets: Vec<(ledger::Value, String)>,
    /// Whether forms show only the settings that differ from their defaults.
    pub only_changed: bool,
    /// The containers that take files dropped from the system, each with the command a file makes.
    pub files: Vec<(u32, Takes<C>)>,
    /// The lists whose rows can be dragged to a new place, each with the command a move makes from the row's key and the place it goes.
    pub moves: Vec<(u32, Mover<C>)>,
    /// The number fields and sliders, each with the least and most its number can be.
    pub numbers: Vec<(u32, i128, i128)>,
    /// The select, popover or menu that is open, by its id; the view builds what it shows only while it is.
    pub opened: Option<Id>,
    /// The focused component, for the parts of the view that show only around it, such as a combo box's completions.
    pub focus: Option<Id>,
    /// The month each calendar that was turned shows, by its id, counted as `Day::month_index` counts; the others show the month of their chosen day.
    pub months: HashMap<Id, i32>,
    /// The sliders that set a part of a color, each drawn with the colors it sets.
    pub shades: Vec<(u32, crate::color::Oklch, crate::color::Channel)>,
    /// The canvases, each with what it draws and the shapes that run a command when pressed.
    pub canvases: Vec<Board>,
    /// The shape the keyboard has reached in a canvas, by the canvas's id and the shape's key.
    pub aim: Option<(Id, u64)>,
    /// The raster images, each with the node that shows it.
    pub images: Vec<(u32, crate::image::Image)>,
    /// The orders tables show their rows in, by `ListInfo::order`.
    pub orders: Vec<std::sync::Arc<crate::table::Ranks>>,
    /// The widths of the columns the user has resized, by the id of each column's head.
    pub resized: HashMap<Id, i32>,
    /// Where each picker's highlight is, by the picker's id.
    pub picks: HashMap<Id, Picking>,
    /// The letters a picker's query matched in its rows' text, as byte ranges, by the text's node.
    pub marks: Vec<(u32, Vec<(u32, u32)>)>,
    /// The rulers of time axes, each with what it shows.
    pub rulers: Vec<Ruler>,
    /// The time the playhead is being dragged to, by the ruler's id, shown until it is let go.
    pub scrub: Option<(Id, i64)>,
    /// How many columns wide each ruler was last laid out, by its id, so its lanes know how much time they show.
    pub spans: HashMap<Id, i32>,
}

/// A time axis's ruler: how its time is counted, what it shows, where the playhead is and the facts it sets.
#[derive(Clone, Copy, Debug)]
pub struct Ruler {
    pub node: u32,
    pub clock: crate::time::Clock,
    pub view: crate::time::TimeView,
    pub playhead: i64,
    pub view_path: ErasedPath,
    pub playhead_path: ErasedPath,
}

/// Which row a picker highlights and which it shows at the top, for the query it was moved under. A new query starts from the top.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Picking {
    pub query: u64,
    pub at: u32,
    pub first: u32,
}

/// What a canvas node draws: what it paints with its own GPU commands, its drawings in order over that, and the keys of its shapes that run a command when pressed, each with the command.
#[derive(Clone)]
pub struct Board {
    pub node: u32,
    pub paint: Option<std::sync::Arc<dyn crate::canvas::Paint>>,
    pub drawings: Vec<std::sync::Arc<crate::canvas::Drawing>>,
    pub presses: Vec<(u64, u32)>,
}

impl<C> Default for Tree<C> {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            text: String::new(),
            commands: Vec::new(),
            counts: HashMap::new(),
            index: HashMap::new(),
            lists: Vec::new(),
            calendar: Calendar::default(),
            changes: None,
            sets: Vec::new(),
            only_changed: false,
            files: Vec::new(),
            moves: Vec::new(),
            numbers: Vec::new(),
            opened: None,
            focus: None,
            months: HashMap::new(),
            shades: Vec::new(),
            canvases: Vec::new(),
            aim: None,
            images: Vec::new(),
            orders: Vec::new(),
            resized: HashMap::new(),
            picks: HashMap::new(),
            marks: Vec::new(),
            rulers: Vec::new(),
            scrub: None,
            spans: HashMap::new(),
        }
    }
}

impl<C> Tree<C> {
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.lists.clear();
        self.text.clear();
        self.commands.clear();
        self.sets.clear();
        self.files.clear();
        self.moves.clear();
        self.numbers.clear();
        self.shades.clear();
        self.canvases.clear();
        self.images.clear();
        self.orders.clear();
        self.marks.clear();
        self.rulers.clear();
        self.counts.clear();
        self.index.clear();
        self.changes = None;
    }

    pub fn root(&mut self) -> u32 {
        self.clear();
        self.nodes.push(Node::new(Id::ROOT, NONE, Kind::Root, Location::caller()));
        0
    }

    /// The least and most the number of a number field or slider can be.
    pub fn bounds(&self, node: u32) -> Option<(i128, i128)> {
        self.numbers.iter().find(|(n, ..)| *n == node).map(|&(_, least, most)| (least, most))
    }

    /// The order a list shows its rows in, if it isn't the list's own.
    pub fn ranks(&self, list: &ListInfo) -> Option<&crate::table::Ranks> {
        self.orders.get(list.order as usize).map(|r| &**r)
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
        let mut node = Node::new(id, parent, kind, at);
        let p = &self.nodes[parent as usize];
        node.over = p.over || matches!(p.kind, Kind::Overlay { .. }) || matches!(kind, Kind::Overlay { .. });
        self.nodes.push(node);
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

    /// Whether a container has hidden this node for want of room, or because it is an entry of a menu that opens.
    pub fn hidden(&self, i: u32) -> bool {
        let n = &self.nodes[i as usize];
        n.area.is_empty() && n.parent != NONE && matches!(self.nodes[n.parent as usize].kind, Kind::Overflow | Kind::Places | Kind::MenuButton)
    }

    /// Whether the node with id `inner` is the node with id `outer` or inside it.
    pub fn within(&self, inner: Id, outer: Id) -> bool {
        let Some(i) = self.find(inner) else { return false };
        self.ancestor(i, |n| n.id == outer).is_some()
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
        if let Some(&(_, color, channel)) = self.shades.iter().find(|s| s.0 == i as u32) {
            (color, channel).hash(&mut h);
        }
        if let Some(b) = self.canvases.iter().find(|b| b.node == i as u32) {
            for d in &b.drawings {
                d.stamp().hash(&mut h);
            }
            b.presses.hash(&mut h);
            b.paint.as_ref().map(|p| p.stamp()).hash(&mut h);
            self.aim.filter(|a| a.0 == n.id).hash(&mut h);
        }
        if let Some(r) = self.rulers.iter().find(|r| r.node == i as u32) {
            (r.clock, r.view, r.playhead).hash(&mut h);
        }
        if let Some((_, marks)) = self.marks.iter().find(|m| m.0 == i as u32) {
            marks.hash(&mut h);
        }
        if let Some((_, image)) = self.images.iter().find(|m| m.0 == i as u32) {
            image.hash(&mut h);
        }
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
