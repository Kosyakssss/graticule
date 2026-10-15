use std::any::Any;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::panic::Location;
use std::sync::Arc;

use ledger::{FactPath, Field, Key, Record, Selection, Text};

use crate::app::{Moment, Path};
use crate::canvas::Drawing;
use crate::color::{CHROMA, Channel, HUE, Oklch};
use crate::days::{Day, Time};
use crate::form::{Control, Item, Setting, Settings};
use crate::ink::Ink;
use crate::message::Message;
use crate::picture::Picture;
use crate::table::{Branch, Cell, Rows, Sort};
use crate::tree::{Board, Holds, Id, Kind, ListInfo, NONE, Span, Tone, Tree};
use crate::value::{Integer, Value, grouped};

const CHECK_MIN: i32 = 12;

/// Where a list is scrolled to: the row at its top, by key so rows arriving above don’t move it, and how many rows it showed last frame. A list shown in an order of its own is scrolled by place, so sorting it again starts from the same place.
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

    /// How tall a picture is, in rows; its width follows from its shape.
    pub fn height(self, rows: i32) -> Self {
        if let Kind::Picture { rows: r, .. } = &mut self.ui.tree.nodes[self.node as usize].kind {
            *r = rows.max(1);
        }
        self
    }

    /// How wide it is, in columns: `.width(28.min(20))`, `.width(Fill)`.
    pub fn width(self, span: impl Into<Span>) -> Self {
        self.ui.tree.nodes[self.node as usize].span = span.into();
        self
    }
}

/// A list just placed, to finish describing: `ui.list(…).on_move(Cmd::Move)`.
pub struct Listed<'u, 'a, C, S, T> {
    ui: &'u mut Ui<'a, C, S>,
    node: u32,
    items: PhantomData<fn() -> T>,
}

impl<C: 'static, S, T: 'static> Listed<'_, '_, C, S, T> {
    /// Rows can be dragged to a new place, which runs `cmd` with the row's key and the place, counted from the top of the list before the move: `Cmd::Move(#[subject] Key<Item>, usize)`. Give it the command itself, so it costs nothing to keep.
    pub fn on_move(self, cmd: impl Fn(Key<T>, usize) -> C + 'static) -> Self {
        self.ui.tree.moves.push((self.node, Box::new(move |key, to| cmd(Key::from_raw(key), to))));
        self
    }

    /// How many rows tall it is when it sits among other things rather than filling the page.
    pub fn height(self, rows: i32) -> Self {
        if let Kind::List { rows: r, .. } = &mut self.ui.tree.nodes[self.node as usize].kind {
            *r = rows.max(3);
        }
        self
    }
}

/// A picker being described, to say what choosing one of its items does.
pub struct Picked<'u, 'a, C, S, T> {
    ui: &'u mut Ui<'a, C, S>,
    node: u32,
    items: PhantomData<fn() -> T>,
}

impl<C, S, T> Picked<'_, '_, C, S, T> {
    /// Runs `cmd` with the key of the item chosen, by Return on the highlighted one or by a click: `.on_choose(Cmd::Open)`.
    pub fn on_choose<M>(self, cmd: impl Bind<C, Key<T>, M> + Clone) -> Self {
        let rows: Vec<u32> = self.ui.tree.children(self.node).collect();
        for r in rows {
            if let Some(c) = cmd.clone().bind(self.ui.tree.nodes[r as usize].subject) {
                self.ui.tree.nodes[r as usize].cmd = self.ui.tree.commands.len() as u32;
                self.ui.tree.commands.push(c);
            }
        }
        self
    }
}

/// How many items a picker shows at a time.
pub(crate) const PICKED: u32 = 8;

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
        self.tree.nodes[i as usize].span = crate::tree::Fill;
        let subject = self.subject;
        f(&mut self.child(i, subject));
    }

    /// Panes side by side, with a divider between each two that the user drags, or moves with the arrows once it is reached, to make one wider and the other narrower. A split takes the rows the page has to spare, and in a window too narrow for its panes it stacks them.
    #[track_caller]
    pub fn split(&mut self, f: impl FnOnce(&mut Ui<'_, C, S>)) {
        let i = self.add(Kind::Split, Location::caller());
        self.tree.nodes[i as usize].span = crate::tree::Fill;
        let subject = self.subject;
        f(&mut self.child(i, subject));
    }

    /// One pane of a `split`, named for its divider and the screen reader, starting `width` columns wide: `30.min(20)` or `Fill`. The width the user gives it is kept with the window.
    #[track_caller]
    pub fn pane(&mut self, name: impl Into<Message>, width: impl Into<Span>, f: impl FnOnce(&mut Ui<'_, C, S>)) {
        let (name, at) = (name.into(), Location::caller());
        let parent = self.parent;
        let id = self.tree.nodes[parent as usize].id.named(name.as_str());
        let before = self.tree.nodes[parent as usize].last;
        if self.tree.nodes[parent as usize].kind == Kind::Split && before != NONE && self.tree.nodes[before as usize].kind == Kind::Pane {
            let d = self.tree.add(parent, Kind::Divider, Some(id.mix(0xd1)), at);
            let between = format!("Between “{}” and “{}”", self.tree.str(self.tree.nodes[before as usize].name), name.as_str());
            let r = self.tree.put(&between);
            self.tree.nodes[d as usize].name = r;
        }
        let i = self.tree.add(parent, Kind::Pane, Some(id), at);
        let span: Span = width.into();
        self.tree.nodes[i as usize].span = match self.tree.resized.get(&id) {
            Some(&w) => crate::tree::cols(w).min(span.min.min(w)),
            None => span,
        };
        let r = self.tree.put(name.as_str());
        self.tree.nodes[i as usize].name = r;
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

    /// A rule between what comes before and what comes after, on a cell edge, with the same room above and below it.
    #[track_caller]
    pub fn separator(&mut self) {
        self.add(Kind::Rule, Location::caller());
    }

    /// Text that heads what follows it, in the strong weight, which a screen reader announces as a heading.
    #[track_caller]
    pub fn heading(&mut self, text: impl AsRef<str>) {
        let i = self.add(Kind::Text { mono: false, tone: Tone::Ink, strong: true }, Location::caller());
        let r = self.tree.put(text.as_ref());
        self.tree.nodes[i as usize].text = r;
    }

    /// The keys that run `cmd`, read from the keymap as it is now, so they never go stale; "no key" when nothing is bound to it.
    #[track_caller]
    pub fn keys<M>(&mut self, cmd: impl Bind<C, S, M>) {
        let i = self.add(Kind::Keys, Location::caller());
        self.command(i, cmd);
    }

    /// How far a piece of work has come, `done` of `total`, as a bar with the count beside it.
    #[track_caller]
    pub fn progress(&mut self, done: u64, total: u64) {
        let filled = if total == 0 { 0 } else { (done.min(total) as u128 * 1000 / total as u128) as u16 };
        let i = self.add(Kind::Gauge { filled, meter: false }, Location::caller());
        let r = self.tree.put(&format!("{} of {}", grouped(done.min(total)), grouped(total)));
        self.tree.nodes[i as usize].text = r;
    }

    /// A level within its range, such as how full a disk is, as a bar with the percentage beside it. `level` runs from 0 to 1.
    #[track_caller]
    pub fn meter(&mut self, level: f32) {
        let level = if level.is_nan() { 0.0 } else { level.clamp(0.0, 1.0) };
        let filled = (level * 1000.0).round() as u16;
        let i = self.add(Kind::Gauge { filled, meter: true }, Location::caller());
        let r = self.tree.put(&format!("{} %", (level * 100.0).round()));
        self.tree.nodes[i as usize].text = r;
    }

    /// `rows` rows kept for content that hasn't arrived yet, drawn as faint lines, so the page doesn't jump when it comes.
    #[track_caller]
    pub fn placeholder(&mut self, rows: i32) {
        self.add(Kind::Placeholder { rows: rows.max(1) }, Location::caller());
    }

    /// A short word that sorts something into a group, such as `design` or `urgent`, in a thin box.
    #[track_caller]
    pub fn tag(&mut self, text: impl AsRef<str>) {
        let i = self.add(Kind::Tag, Location::caller());
        let r = self.tree.put(text.as_ref());
        self.tree.nodes[i as usize].text = r;
    }

    /// A person, as the initials of their name in a box. A screen reader reads the whole name.
    #[track_caller]
    pub fn avatar(&mut self, name: impl AsRef<str>) {
        let name = name.as_ref();
        let initials: String = name.split_whitespace().filter_map(|w| w.chars().next()).flat_map(char::to_uppercase).take(2).collect();
        let i = self.add(Kind::Avatar, Location::caller());
        let r = self.tree.put(if initials.is_empty() { "?" } else { &initials });
        self.tree.nodes[i as usize].text = r;
        let r = self.tree.put(name);
        self.tree.nodes[i as usize].name = r;
    }

    /// A message boxed into the page with its ink's icon: `Ink::Link` for information, `Warn`, `Danger` or `Ok`. What goes inside is described in `f`.
    #[track_caller]
    pub fn callout(&mut self, ink: Ink, f: impl FnOnce(&mut Ui<'_, C, S>)) {
        let i = self.add(Kind::Callout { ink }, Location::caller());
        let subject = self.subject;
        f(&mut self.child(i, subject));
    }

    /// A group of components in a box under its title, for things that belong together, such as one account's details.
    #[track_caller]
    pub fn card(&mut self, title: impl AsRef<str>, f: impl FnOnce(&mut Ui<'_, C, S>)) {
        let i = self.add(Kind::Card, Location::caller());
        let r = self.tree.put(title.as_ref());
        self.tree.nodes[i as usize].text = r;
        let subject = self.subject;
        f(&mut self.child(i, subject));
    }

    /// A small box filled with one of the theme's inks, which follows the theme and the appearance. A screen reader reads the ink's name.
    #[track_caller]
    pub fn swatch(&mut self, ink: Ink) {
        let i = self.add(Kind::Swatch { ink }, Location::caller());
        let r = self.tree.put(ink.name());
        self.tree.nodes[i as usize].text = r;
    }

    /// A picture compiled by Engraver, one row tall unless `.height(rows)` says otherwise, as wide as its shape needs. A screen reader reads its label.
    #[track_caller]
    pub fn picture(&mut self, picture: &'static Picture) -> Placed<'_, 'a, C, S> {
        let i = self.add(Kind::Picture { picture, rows: 1 }, Location::caller());
        let r = self.tree.put(picture.label);
        self.tree.nodes[i as usize].text = r;
        Placed { ui: self, node: i }
    }

    /// A raster image `rows` tall and as wide as its shape asks, narrower when the room is, with `label` saying what it shows for screen readers. An image not loaded yet holds its place, blank.
    #[track_caller]
    pub fn image(&mut self, label: impl AsRef<str>, image: &crate::image::Image, rows: i32) {
        let (width, height) = if image.is_empty() { (1, 1) } else { (image.width(), image.height()) };
        let i = self.add(Kind::Image { rows, width, height }, Location::caller());
        let r = self.tree.put(label.as_ref());
        self.tree.nodes[i as usize].name = r;
        self.tree.images.push((i, image.clone()));
    }

    /// A count, in the monospace face with its thousands grouped: 1,284.
    #[track_caller]
    pub fn count(&mut self, n: impl Integer) {
        self.value(Value::Count, &grouped(n), Location::caller());
    }

    /// A moment, written the one way every date is written: as the day and time, or as how long ago, by the user's one setting.
    #[track_caller]
    pub fn date(&mut self, at: Moment) {
        let (text, changes) = self.tree.calendar.write(at);
        if let Some(c) = changes {
            self.tree.changes = Some(self.tree.changes.map_or(c, |t| t.min(c)));
        }
        self.value(Value::Date, &text, Location::caller());
    }

    /// A version, like `0.4.1`, in a dotted box.
    #[track_caller]
    pub fn version(&mut self, version: impl AsRef<str>) {
        self.value(Value::Version, version.as_ref(), Location::caller());
    }

    /// What state something is in, like `draft` or `offline`, filled with `ink`: one of the inks text is written in, such as `Ink::Ok`, `Ink::Warn` or `Ink::Danger`. A badge: it says what something is, never what to do.
    #[track_caller]
    pub fn state(&mut self, state: impl AsRef<str>, ink: Ink) {
        let ink = if ink.needs().is_some() { ink } else { Ink::Ink };
        self.value(Value::State(ink), &state.as_ref().to_uppercase(), Location::caller());
    }

    /// A product's code, like `DK-01`, in a heavy box.
    #[track_caller]
    pub fn product(&mut self, code: impl AsRef<str>) {
        self.value(Value::Product, &code.as_ref().to_uppercase(), Location::caller());
    }

    /// A piece of code or a query, like `is:unread`, on the field's surface.
    #[track_caller]
    pub fn code(&mut self, code: impl AsRef<str>) {
        self.value(Value::Code, code.as_ref(), Location::caller());
    }

    /// The mode something is in, like `rows`, muted and underlined.
    #[track_caller]
    pub fn mode(&mut self, mode: impl AsRef<str>) {
        self.value(Value::Mode, mode.as_ref(), Location::caller());
    }

    /// What went wrong, in the danger ink.
    #[track_caller]
    pub fn error(&mut self, text: impl AsRef<str>) {
        self.value(Value::Error, text.as_ref(), Location::caller());
    }

    fn value(&mut self, value: Value, text: &str, at: &'static Location<'static>) {
        let i = self.add(Kind::Value { value }, at);
        let r = self.tree.put(text);
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

    /// A button that stays pressed in while `on`. Clicking it runs `cmd`, which turns the thing on or off: a mode, such as bold or wrapping lines.
    #[track_caller]
    pub fn toggle<M>(&mut self, on: bool, cmd: impl Bind<C, S, M>) -> Placed<'_, 'a, C, S> {
        let i = self.add(Kind::Toggle { on }, Location::caller());
        self.command(i, cmd);
        Placed { ui: self, node: i }
    }

    /// A button that shows a picture instead of words, such as `icons::CLOSE`. The command's label is what the status line and a screen reader say.
    #[track_caller]
    pub fn icon_button<M>(&mut self, picture: &'static Picture, cmd: impl Bind<C, S, M>) -> Placed<'_, 'a, C, S> {
        let i = self.add(Kind::IconButton { picture }, Location::caller());
        self.command(i, cmd);
        Placed { ui: self, node: i }
    }

    /// Buttons and toggles joined side by side, with no room between them, read as one control.
    #[track_caller]
    pub fn group(&mut self, f: impl FnOnce(&mut Ui<'_, C, S>)) {
        let i = self.add(Kind::Group, Location::caller());
        let subject = self.subject;
        f(&mut self.child(i, subject));
    }

    /// An on or off fact at `path`, after `label`, changed the moment it is clicked. Each change is an `Edit` and one undo step.
    #[track_caller]
    pub fn switch<F: Field>(&mut self, label: impl AsRef<str>, path: FactPath<F, bool>) {
        let on = self.facts::<F>().and_then(|f| path.get(f)).copied().unwrap_or(false);
        self.switch_at(label.as_ref(), path.erased(), on, Location::caller());
    }

    /// One of a few named options, as joined toggles after `label`: the fact at `path`, an enum with `derive(Choice)`. Choosing one is an `Edit` and one undo step.
    #[track_caller]
    pub fn segments<F: Field, T: Setting>(&mut self, label: impl AsRef<str>, path: FactPath<F, T>) {
        let (Control::Choice(options), Some(now)) = (T::control(), self.facts::<F>().and_then(|f| path.get(f)).map(Field::value)) else { return };
        self.segments_at(label.as_ref(), path.erased(), &options, &now, Location::caller());
    }

    /// One of a few named options, as radio buttons one under another under `label`: the fact at `path`, an enum with `derive(Choice)`. Choosing one is an `Edit` and one undo step.
    #[track_caller]
    pub fn radio<F: Field, T: Setting>(&mut self, label: impl AsRef<str>, path: FactPath<F, T>) {
        let (Control::Choice(options), Some(now)) = (T::control(), self.facts::<F>().and_then(|f| path.get(f)).map(Field::value)) else { return };
        let (label, at) = (label.as_ref(), Location::caller());
        let path = path.erased();
        let id = Id::ROOT.mix(path.hash64());
        let set = self.tree.add(self.parent, Kind::Fieldset, Some(id), at);
        let r = self.tree.put(label);
        self.tree.nodes[set as usize].text = r;
        for (k, (option, value)) in options.iter().enumerate() {
            let c = self.tree.add(set, Kind::Radio { on: *value == now }, Some(id.mix(3).mix(k as u64)), at);
            let r = self.tree.put(option);
            self.tree.nodes[c as usize].text = r;
            let said = format!("{label}: {option}");
            let r = self.tree.put(&said);
            self.tree.nodes[c as usize].name = r;
            self.set(c, path, value.clone(), said);
        }
    }

    /// Controls grouped under a heading, such as the settings of one kind. A screen reader names the group with it.
    #[track_caller]
    pub fn fieldset(&mut self, legend: impl AsRef<str>, f: impl FnOnce(&mut Ui<'_, C, S>)) {
        let i = self.add(Kind::Fieldset, Location::caller());
        let r = self.tree.put(legend.as_ref());
        self.tree.nodes[i as usize].text = r;
        let subject = self.subject;
        f(&mut self.child(i, subject));
    }

    /// A control with its label above it. Muted text and an error described after the control say more about it, and a screen reader reads them with it.
    #[track_caller]
    pub fn labelled(&mut self, label: impl AsRef<str>, f: impl FnOnce(&mut Ui<'_, C, S>)) {
        let i = self.add(Kind::Labelled, Location::caller());
        let r = self.tree.put(label.as_ref());
        self.tree.nodes[i as usize].text = r;
        let subject = self.subject;
        f(&mut self.child(i, subject));
    }

    /// Pages of one panel: a tab for each option of the enum at `path`, an enum with `derive(Choice)`, and under them the page `f` describes for the chosen one. Switching tabs is an `Edit` and one undo step.
    #[track_caller]
    pub fn tabs<F: Field, T: Setting>(&mut self, path: FactPath<F, T>, f: impl FnOnce(&mut Ui<'_, C, S>, T)) {
        let (Control::Choice(options), Some(now)) = (T::control(), self.facts::<F>().and_then(|f| path.get(f)).cloned()) else { return };
        let at = Location::caller();
        let path = path.erased();
        let id = Id::ROOT.mix(path.hash64());
        let tabs = self.tree.add(self.parent, Kind::Tabs, Some(id), at);
        self.tree.nodes[tabs as usize].span = crate::tree::Fill;
        let strip = self.tree.add(tabs, Kind::Strip, Some(id.mix(1)), at);
        let value = now.value();
        for (k, (option, v)) in options.iter().enumerate() {
            let c = self.tree.add(strip, Kind::Tab { on: *v == value }, Some(id.mix(3).mix(k as u64)), at);
            self.option(c, option, path, v.clone());
        }
        let page = self.tree.add(tabs, Kind::Column, Some(id.mix(2)), at);
        let subject = self.subject;
        f(&mut self.child(page, subject), now);
    }

    /// The places of an app along the top, one for each option of the enum at `path`, an enum with `derive(Choice)`, with the current one marked. Places that don't fit go into a "More…" choice. Going to one is an `Edit` and one undo step.
    #[track_caller]
    pub fn places<F: Field, T: Setting>(&mut self, path: FactPath<F, T>) {
        let (Control::Choice(options), Some(now)) = (T::control(), self.facts::<F>().and_then(|f| path.get(f)).map(Field::value)) else { return };
        let at = Location::caller();
        let path = path.erased();
        let id = Id::ROOT.mix(path.hash64());
        let places = self.tree.add(self.parent, Kind::Places, Some(id), at);
        for (k, (option, v)) in options.iter().enumerate() {
            let c = self.tree.add(places, Kind::Place { on: *v == now }, Some(id.mix(3).mix(k as u64)), at);
            self.option(c, option, path, v.clone());
        }
        self.more(places, id, at);
    }

    /// A section that opens in place: `label` with a chevron, and under it what `f` describes while the fact at `path` is true. Opening and closing it is an `Edit` and one undo step.
    #[track_caller]
    pub fn disclosure<F: Field>(&mut self, label: impl AsRef<str>, path: FactPath<F, bool>, f: impl FnOnce(&mut Ui<'_, C, S>)) {
        let open = self.facts::<F>().and_then(|f| path.get(f)).copied().unwrap_or(false);
        let (label, at) = (label.as_ref(), Location::caller());
        let path = path.erased();
        let id = Id::ROOT.mix(path.hash64());
        let section = self.tree.add(self.parent, Kind::Disclosure, Some(id), at);
        self.tree.nodes[section as usize].subject = self.subject;
        let opener = self.tree.add(section, Kind::Opener { open }, Some(id.mix(1)), at);
        let r = self.tree.put(label);
        self.tree.nodes[opener as usize].text = r;
        self.set(opener, path, ledger::Value::Bool(!open), format!("{label}: {}", if open { "close" } else { "open" }));
        if open {
            let subject = self.subject;
            f(&mut self.child(section, subject));
        }
    }

    /// Where something sits, as the levels that lead to it: `names` from the top down, with the fact at `path` saying how deep the current one is, counting from 0. The levels above it are links back up, and going to one is an `Edit` and one undo step. A long trail is shortened in the middle.
    #[track_caller]
    pub fn breadcrumbs<F: Field>(&mut self, path: FactPath<F, u32>, names: &[&str]) {
        let depth = self.facts::<F>().and_then(|f| path.get(f)).copied().unwrap_or(0) as usize;
        let at = Location::caller();
        let path = path.erased();
        let id = Id::ROOT.mix(path.hash64());
        let crumbs = self.tree.add(self.parent, Kind::Crumbs, Some(id), at);
        for (k, name) in names.iter().enumerate().take(depth + 1) {
            let kind = if k < depth { Kind::Link } else { Kind::Text { mono: false, tone: Tone::Ink, strong: true } };
            let c = self.tree.add(crumbs, kind, Some(id.mix(3).mix(k as u64)), at);
            let r = self.tree.put(name);
            self.tree.nodes[c as usize].text = r;
            if k < depth {
                self.set(c, path, (k as u32).value(), format!("Back to {name}"));
            }
        }
    }

    /// Links to the pages of something that comes in `count` pages, with the page at `path` marked: the first, the last, the ones beside it, and the previous and next. Pages count from 0 in the facts and from 1 on screen. Going to one is an `Edit` and one undo step.
    #[track_caller]
    pub fn pages<F: Field>(&mut self, path: FactPath<F, u32>, count: u32) {
        let now = self.facts::<F>().and_then(|f| path.get(f)).copied().unwrap_or(0);
        let at = Location::caller();
        let path = path.erased();
        let id = Id::ROOT.mix(path.hash64());
        let strip = self.tree.add(self.parent, Kind::Strip, Some(id), at);
        let last = count.saturating_sub(1);
        let link = |ui: &mut Self, label: &str, page: u32, on: bool, enabled: bool, key: u64| {
            let c = ui.tree.add(strip, Kind::PageLink { on }, Some(id.mix(key)), at);
            let r = ui.tree.put(label);
            ui.tree.nodes[c as usize].text = r;
            if enabled {
                ui.set(c, path, page.value(), format!("Page {}", page + 1));
            } else {
                ui.tree.nodes[c as usize].enabled = false;
            }
        };
        link(self, "Previous", now.saturating_sub(1), false, now > 0, 1);
        let mut gap = false;
        for page in 0..count {
            if page == 0 || page == last || page.abs_diff(now) <= 1 {
                link(self, &(page + 1).to_string(), page, page == now, page != now, 4 + page as u64);
                gap = false;
            } else if !gap {
                let c = self.tree.add(strip, Kind::Text { mono: true, tone: Tone::Muted, strong: false }, Some(id.mix(3).mix(page as u64)), at);
                let r = self.tree.put("…");
                self.tree.nodes[c as usize].text = r;
                gap = true;
            }
        }
        link(self, "Next", (now + 1).min(last), false, now < last, 2);
    }

    /// Buttons and links on one row; what doesn't fit goes into a "More…" choice at the end, which lists it.
    #[track_caller]
    pub fn overflow(&mut self, f: impl FnOnce(&mut Ui<'_, C, S>)) {
        let at = Location::caller();
        let i = self.add(Kind::Overflow, at);
        let subject = self.subject;
        f(&mut self.child(i, subject));
        let id = self.tree.nodes[i as usize].id;
        self.more(i, id, at);
    }

    fn option(&mut self, c: u32, option: &str, path: ledger::ErasedPath, value: ledger::Value) {
        let r = self.tree.put(option);
        self.tree.nodes[c as usize].text = r;
        self.set(c, path, value, option.to_owned());
    }

    fn more(&mut self, parent: u32, id: Id, at: &'static Location<'static>) {
        let c = self.tree.add(parent, Kind::More, Some(id.mix(9)), at);
        let r = self.tree.put("More…");
        self.tree.nodes[c as usize].text = r;
    }

    fn switch_at(&mut self, label: &str, path: ledger::ErasedPath, on: bool, at: &'static Location<'static>) {
        let i = self.tree.add(self.parent, Kind::Switch { on }, Some(Id::ROOT.mix(path.hash64())), at);
        let r = self.tree.put(label);
        self.tree.nodes[i as usize].text = r;
        self.tree.nodes[i as usize].span = crate::tree::Fill.min(CHECK_MIN);
        self.set(i, path, ledger::Value::Bool(!on), format!("{label}: {}", if on { "off" } else { "on" }));
    }

    /// One of a few named options, as a button that shows the chosen one after `label`: the fact at `path`, an enum with `derive(Choice)`. Pressing it chooses the next option, after the last the first; each choice is an `Edit` and one undo step.
    #[track_caller]
    pub fn choice<F: Field, T: Setting>(&mut self, label: impl AsRef<str>, path: FactPath<F, T>) {
        let (Control::Choice(options), Some(now)) = (T::control(), self.facts::<F>().and_then(|f| path.get(f)).map(Field::value)) else { return };
        let (label, path, at) = (label.as_ref(), path.erased(), Location::caller());
        let Some(k) = options.iter().position(|(_, value)| *value == now) else { return };
        let id = Id::ROOT.mix(path.hash64());
        let row = self.tree.add(self.parent, Kind::Row, Some(id.mix(1)), at);
        let l = self.tree.add(row, Kind::Text { mono: false, tone: Tone::Ink, strong: false }, Some(id.mix(2)), at);
        let r = self.tree.put(label);
        self.tree.nodes[l as usize].text = r;
        let widest = options.iter().map(|(option, _)| option.chars().count()).max().unwrap_or(0).min(u8::MAX as usize) as u8;
        let c = self.tree.add(row, Kind::Cycle { widest }, Some(id), at);
        let (option, (next, value)) = (options[k].0, &options[(k + 1) % options.len()]);
        let (r, named) = (self.tree.put(option), self.tree.put(&format!("{label}: {option}")));
        let n = &mut self.tree.nodes[c as usize];
        (n.text, n.name, n.subject) = (r, named, self.subject);
        self.set(c, path, value.clone(), format!("{label}: {next}"));
    }

    /// A button named `label` that opens a menu of the controls `f` describes, anchored under it. Each entry runs what its control would, shows its key, and says why when it can't run.
    #[track_caller]
    pub fn menu(&mut self, label: impl AsRef<str>, f: impl FnOnce(&mut Ui<'_, C, S>)) {
        let i = self.add(Kind::MenuButton, Location::caller());
        let r = self.tree.put(label.as_ref());
        self.tree.nodes[i as usize].text = r;
        let subject = self.subject;
        f(&mut self.child::<S>(i, subject));
    }

    /// A button named `label` that opens what `f` describes in an overlay anchored under it, for a few controls that belong to the button: sharing, a filter's details, a quick setting. Escape, a click elsewhere or moving the focus out closes it. What `f` describes is built only while it is open.
    #[track_caller]
    pub fn popover(&mut self, label: impl AsRef<str>, f: impl FnOnce(&mut Ui<'_, C, S>)) {
        let at = Location::caller();
        let i = self.add(Kind::Popover { open: false }, at);
        let id = self.tree.nodes[i as usize].id;
        let open = self.tree.opened == Some(id);
        let r = self.tree.put(label.as_ref());
        (self.tree.nodes[i as usize].text, self.tree.nodes[i as usize].kind) = (r, Kind::Popover { open });
        if open {
            let o = self.tree.add(i, Kind::Overlay { anchor: crate::tree::Anchor::Node(i) }, Some(id.mix(1)), at);
            let subject = self.subject;
            f(&mut self.child::<S>(o, subject));
        }
    }

    /// One of a few named options, as a button that shows the chosen one after `label` and opens the options in place under it: the fact at `path`, an enum with `derive(Choice)`. Choosing one closes them again; each choice is an `Edit` and one undo step.
    #[track_caller]
    pub fn select<F: Field, T: Setting>(&mut self, label: impl AsRef<str>, path: FactPath<F, T>) {
        let (Control::Choice(options), Some(now)) = (T::control(), self.facts::<F>().and_then(|f| path.get(f)).map(Field::value)) else { return };
        let label = label.as_ref();
        let on: Vec<bool> = options.iter().map(|(_, value)| *value == now).collect();
        let shown = options.iter().find(|(_, value)| *value == now).map_or("", |(option, _)| option).to_string();
        let picks = options.iter().zip(on).map(|(&(option, ref value), on)| (option, on, value.clone(), format!("{label}: {option}"))).collect();
        self.select_at(label, path.erased(), picks, shown, false, Location::caller());
    }

    /// Any of a few named options, as a button that shows the chosen ones after `label` and opens the options in place under it, each ticked or not: the list at `path`, of an enum with `derive(Choice)`. Ticking or unticking one is an `Edit` and one undo step, and the options stay open until Escape or a click elsewhere.
    #[track_caller]
    pub fn multi_select<F: Field, T: Setting>(&mut self, label: impl AsRef<str>, path: FactPath<F, Vec<T>>) {
        let (Control::Choice(options), Some(ledger::Value::List(now))) = (T::control(), self.facts::<F>().and_then(|f| path.get(f)).map(Field::value)) else { return };
        let label = label.as_ref();
        let on: Vec<bool> = options.iter().map(|(_, value)| now.contains(value)).collect();
        let names: Vec<&str> = options.iter().zip(&on).filter(|(_, on)| **on).map(|((option, _), _)| *option).collect();
        let widest = options.iter().map(|(option, _)| option.chars().count()).max().unwrap_or(0) * 2 + 2;
        let joined = names.join(", ");
        let shown = match names.len() {
            0 => "None".to_string(),
            _ if joined.chars().count() <= widest => joined,
            n => format!("{n} of {}", options.len()),
        };
        let picks = options
            .iter()
            .zip(&on)
            .enumerate()
            .map(|(k, (&(option, _), &was))| {
                let next = options.iter().zip(&on).enumerate().filter(|&(j, (_, on))| if j == k { !was } else { *on }).map(|(_, ((_, value), _))| value.clone()).collect();
                (option, was, ledger::Value::List(next), format!("{label}: {} “{option}”", if was { "untick" } else { "tick" }))
            })
            .collect();
        self.select_at(label, path.erased(), picks, shown, true, Location::caller());
    }

    fn select_at(&mut self, label: &str, path: ledger::ErasedPath, picks: Vec<(&'static str, bool, ledger::Value, String)>, shown: String, many: bool, at: &'static Location<'static>) {
        let id = Id::ROOT.mix(path.hash64());
        let row = self.tree.add(self.parent, Kind::Row, Some(id.mix(1)), at);
        let l = self.tree.add(row, Kind::Text { mono: false, tone: Tone::Ink, strong: false }, Some(id.mix(2)), at);
        let r = self.tree.put(label);
        self.tree.nodes[l as usize].text = r;
        let longest = picks.iter().map(|(option, ..)| option.chars().count()).max().unwrap_or(0);
        let widest = if many { longest * 2 + 2 } else { longest }.min(u8::MAX as usize) as u8;
        let open = self.tree.opened == Some(id);
        let s = self.tree.add(row, Kind::Select { widest, many, open }, Some(id), at);
        let (r, named) = (self.tree.put(&shown), self.tree.put(label));
        let n = &mut self.tree.nodes[s as usize];
        (n.text, n.name, n.subject) = (r, named, self.subject);
        if !open {
            return;
        }
        for (k, (option, on, value, said)) in picks.into_iter().enumerate() {
            let o = self.tree.add(s, Kind::Pick { on }, Some(id.mix(k as u64 + 3)), at);
            let r = self.tree.put(option);
            self.tree.nodes[o as usize].text = r;
            self.set(o, path, value, said);
        }
    }

    fn segments_at(&mut self, label: &str, path: ledger::ErasedPath, options: &[(&'static str, ledger::Value)], now: &ledger::Value, at: &'static Location<'static>) {
        let id = Id::ROOT.mix(path.hash64());
        let row = self.tree.add(self.parent, Kind::Row, Some(id), at);
        let l = self.tree.add(row, Kind::Text { mono: false, tone: Tone::Ink, strong: false }, Some(id.mix(1)), at);
        let r = self.tree.put(label);
        self.tree.nodes[l as usize].text = r;
        let group = self.tree.add(row, Kind::Group, Some(id.mix(2)), at);
        for (k, (option, value)) in options.iter().enumerate() {
            let c = self.tree.add(group, Kind::Toggle { on: value == now }, Some(id.mix(3).mix(k as u64)), at);
            let r = self.tree.put(option);
            self.tree.nodes[c as usize].text = r;
            let said = format!("{label}: {option}");
            let r = self.tree.put(&said);
            self.tree.nodes[c as usize].name = r;
            self.set(c, path, value.clone(), said);
        }
    }

    /// Every setting of the form at `path`, each with the control its type asks for: a switch, a row of options, a stepper or a field. Each change arrives in `update` as an `Edit` and is one undo step.
    #[track_caller]
    pub fn form<F: Field, T: Settings>(&mut self, path: FactPath<F, T>) {
        let at = Location::caller();
        let Some(form) = self.facts::<F>().and_then(|f| path.get(f)) else { return };
        let mut items = Vec::new();
        form.items(&mut items);
        let base = path.erased();
        let only = self.tree.only_changed;
        let column = self.add(Kind::Column, at);
        self.tree.nodes[column as usize].span = crate::tree::Fill;
        let subject = self.subject;
        let mut ui = self.child::<S>(column, subject);
        let shown: Vec<Item> = items.into_iter().filter(|item| !only || item.changed()).collect();
        if shown.is_empty() {
            ui.muted("Every setting is as it was at first");
        }
        for item in shown {
            let mut path = base;
            for step in item.path.steps() {
                path = path.push(*step, item.path.kind());
            }
            ui.setting(path, item, at);
        }
    }

    fn setting(&mut self, path: ledger::ErasedPath, item: Item, at: &'static Location<'static>) {
        let id = Id::ROOT.mix(path.hash64());
        match &item.control {
            Control::Switch => self.switch_at(item.label, path, item.value == ledger::Value::Bool(true), at),
            Control::Text => {
                let around = self.tree.add(self.parent, Kind::Labelled, Some(id.mix(1)), at);
                let r = self.tree.put(item.label);
                self.tree.nodes[around as usize].text = r;
                let i = self.tree.add(around, Kind::Field { holds: Holds::Line }, Some(id), at);
                self.tree.nodes[i as usize].path = Some(path);
                let r = self.tree.put(&item.shown());
                self.tree.nodes[i as usize].text = r;
                self.tree.nodes[i as usize].span = crate::tree::Fill.min(16);
            }
            Control::Choice(options) => self.segments_at(item.label, path, options, &item.value, at),
            Control::Whole { least, most } => {
                let row = self.tree.add(self.parent, Kind::Row, Some(id.mix(4)), at);
                let label = self.tree.add(row, Kind::Text { mono: false, tone: Tone::Ink, strong: false }, Some(id.mix(1)), at);
                let r = self.tree.put(item.label);
                self.tree.nodes[label as usize].text = r;
                let count = self.tree.add(row, Kind::Field { holds: Holds::Number }, Some(id), at);
                let (r, named) = (self.tree.put(&item.shown()), self.tree.put(item.label));
                let n = &mut self.tree.nodes[count as usize];
                (n.path, n.text, n.name, n.subject) = (Some(path), r, named, self.subject);
                self.tree.numbers.push((count, *least, *most));
                for (k, (sign, by, verb, end)) in [("−", -1, "Lower", "least"), ("+", 1, "Raise", "most")].into_iter().enumerate() {
                    let b = self.tree.add(row, Kind::Button, Some(id.mix(3).mix(k as u64)), at);
                    let r = self.tree.put(sign);
                    self.tree.nodes[b as usize].text = r;
                    let r = self.tree.put(&format!("{verb} {}", item.label));
                    self.tree.nodes[b as usize].name = r;
                    match crate::form::step(&item.value, by, *least, *most) {
                        Some(next) => {
                            let said = format!("{}: {}", item.label, crate::form::shown(&item.control, &next));
                            self.set(b, path, next, said);
                        }
                        None => {
                            let r = self.tree.put(&format!("{} is at its {end}, {}", item.label, item.shown()));
                            let n = &mut self.tree.nodes[b as usize];
                            (n.reason, n.enabled) = (r, false);
                        }
                    }
                }
            }
        }
    }

    fn set(&mut self, node: u32, path: ledger::ErasedPath, value: ledger::Value, said: String) {
        let n = &mut self.tree.nodes[node as usize];
        (n.path, n.set) = (Some(path), self.tree.sets.len() as u32);
        self.tree.sets.push((value, said));
    }

    /// A one-line field that types into the `Text` at `path`. Its caret and selection are the field's own; each change arrives in `update` as an `Edit`.
    #[track_caller]
    pub fn field<F: Field>(&mut self, path: FactPath<F, Text>, placeholder: Message) -> Placed<'_, 'a, C, S> {
        self.field_as(path, placeholder, Holds::Line, Location::caller())
    }

    /// A field for words to search for, after a magnifying glass. The app filters by the `Text` at `path` as it changes; Escape empties it.
    #[track_caller]
    pub fn search<F: Field>(&mut self, path: FactPath<F, Text>, placeholder: Message) -> Placed<'_, 'a, C, S> {
        self.field_as(path, placeholder, Holds::Search, Location::caller())
    }

    /// A field of several lines that types into the `Text` at `path`: at least `rows` tall, growing with what is typed. Return starts a new line; the primary key with Return submits it.
    #[track_caller]
    pub fn text_area<F: Field>(&mut self, path: FactPath<F, Text>, placeholder: Message, rows: u16) -> Placed<'_, 'a, C, S> {
        self.field_as(path, placeholder, Holds::Lines { rows }, Location::caller())
    }

    /// The `Text` at `path`, reading as text, which becomes a field when it is clicked or reached with Tab. Return finishes the edit.
    #[track_caller]
    pub fn editable<F: Field>(&mut self, path: FactPath<F, Text>, placeholder: Message) -> Placed<'_, 'a, C, S> {
        self.field_as(path, placeholder, Holds::Inline, Location::caller())
    }

    /// A whole number at `path`, typed as a sum such as `12*4+3` and stepped by one with the arrows up and down. What is typed is worked out when Return is pressed or another field is reached, and the number is set by one `Edit`; Escape puts back what was there.
    #[track_caller]
    pub fn number<F: Field, T: Setting>(&mut self, path: FactPath<F, T>) {
        let Some((now, least, most)) = self.whole(path) else { return };
        let erased = path.erased();
        let i = self.tree.add(self.parent, Kind::Field { holds: Holds::Number }, Some(Id::ROOT.mix(erased.hash64())), Location::caller());
        let r = self.tree.put(&grouped(now));
        let node = &mut self.tree.nodes[i as usize];
        (node.subject, node.path, node.text) = (self.subject, Some(erased), r);
        self.tree.numbers.push((i, least, most));
    }

    /// A whole number at `path` within `range`, as a handle on a track with the number after it. Dragging the handle sets the number when it is let go, one `Edit`; the arrows step it by one, Page Up and Page Down by a tenth of the range, and Home and End go to its ends.
    #[track_caller]
    pub fn slider<F: Field, T: Setting>(&mut self, path: FactPath<F, T>, range: std::ops::RangeInclusive<i64>) {
        let Some((now, least, most)) = self.whole(path) else { return };
        let (least, most) = (least.max(*range.start() as i128), most.min(*range.end() as i128));
        if least >= most {
            return;
        }
        let at = ((now.clamp(least, most) - least) * 1000 / (most - least)) as u16;
        let label = grouped(least).chars().count().max(grouped(most).chars().count()) as u8;
        let erased = path.erased();
        let i = self.tree.add(self.parent, Kind::Slider { at, label }, Some(Id::ROOT.mix(erased.hash64())), Location::caller());
        let r = self.tree.put(&grouped(now));
        let node = &mut self.tree.nodes[i as usize];
        (node.subject, node.path, node.text, node.span) = (self.subject, Some(erased), r, crate::tree::Fill.min(13 + label as i32));
        self.tree.numbers.push((i, least, most));
    }

    /// Two whole numbers at `path`, the ends of a range within `range`, as two handles on one track with the range after it. Each handle is reached with Tab and moved with the same keys as a slider's, and neither passes the other; letting go of a handle sets the range by one `Edit`.
    #[track_caller]
    pub fn range_slider<F: Field, T: Setting>(&mut self, path: FactPath<F, (T, T)>, range: std::ops::RangeInclusive<i64>) {
        let Control::Whole { least, most } = T::control() else { return };
        let Some(ledger::Value::List(ends)) = self.facts::<F>().and_then(|f| path.get(f)).map(Field::value) else { return };
        let whole = |v: &ledger::Value| match v {
            ledger::Value::Int(n) => Some(*n as i128),
            ledger::Value::UInt(n) => Some(*n as i128),
            _ => None,
        };
        let (Some(low), Some(high)) = (ends.first().and_then(whole), ends.get(1).and_then(whole)) else { return };
        let (least, most) = (least.max(*range.start() as i128), most.min(*range.end() as i128));
        if least >= most {
            return;
        }
        let at = Location::caller();
        let widest = grouped(least).chars().count().max(grouped(most).chars().count());
        let label = (widest * 2 + 1) as u8;
        let erased = path.erased();
        let id = Id::ROOT.mix(erased.hash64());
        let i = self.tree.add(self.parent, Kind::Range { label }, Some(id), at);
        let r = self.tree.put(&format!("{}–{}", grouped(low), grouped(high)));
        let node = &mut self.tree.nodes[i as usize];
        (node.subject, node.path, node.text, node.span) = (self.subject, Some(erased), r, crate::tree::Fill.min(13 + label as i32));
        self.tree.numbers.push((i, least, most));
        for (k, (now, upper, name)) in [(low, false, "From"), (high, true, "To")].into_iter().enumerate() {
            let t = self.tree.add(i, Kind::Thumb { at: ((now.clamp(least, most) - least) * 1000 / (most - least)) as u16, high: upper }, Some(id.mix(k as u64 + 1)), at);
            let (r, named) = (self.tree.put(&grouped(now)), self.tree.put(name));
            let node = &mut self.tree.nodes[t as usize];
            (node.text, node.name, node.subject) = (r, named, self.subject);
        }
    }

    /// A code of `length` characters that types into the `Text` at `path`, one character to a box, such as the six digits a sign-in sends. Typing past the last box does nothing.
    #[track_caller]
    pub fn code_field<F: Field>(&mut self, path: FactPath<F, Text>, length: u8) -> Placed<'_, 'a, C, S> {
        self.field_as(path, Message::plain(format!("A code of {} characters", length.max(1))), Holds::Code { length: length.max(1) }, Location::caller())
    }

    /// A file's path in the `Text` at `path`: a field to type or paste it into, and a button that opens the system's file panel to choose one. Choosing sets the path by one `Edit`, named after `label`.
    #[track_caller]
    pub fn file_field<F: Field>(&mut self, label: impl AsRef<str>, path: FactPath<F, Text>) {
        let (label, at) = (label.as_ref(), Location::caller());
        let erased = path.erased();
        let row = self.tree.add(self.parent, Kind::Row, Some(Id::ROOT.mix(erased.hash64()).mix(1)), at);
        self.tree.nodes[row as usize].span = crate::tree::Fill;
        let subject = self.subject;
        let mut ui = self.child::<S>(row, subject);
        ui.field_as(path, Message::plain(label.to_string()), Holds::Line, at);
        let b = ui.tree.add(row, Kind::Browse, Some(Id::ROOT.mix(erased.hash64()).mix(2)), at);
        let (r, named) = (ui.tree.put("Choose…"), ui.tree.put(&format!("Choose a file for {label}")));
        let n = &mut ui.tree.nodes[b as usize];
        (n.text, n.name, n.path, n.subject) = (r, named, Some(erased), subject);
    }

    /// A button for `cmd` that asks `question` in its own place before running it: pressing it puts the question there, with a button that runs `cmd` and one that cancels. Escape, a click elsewhere or moving the focus away cancels too. For what can't be undone or would lose work; anything that can be undone should just run.
    #[track_caller]
    pub fn confirm<M>(&mut self, cmd: impl Bind<C, S, M>, question: Message) {
        let at = Location::caller();
        let i = self.add(Kind::Confirm { open: false }, at);
        let id = self.tree.nodes[i as usize].id;
        self.command(i, cmd);
        if self.tree.opened != Some(id) {
            return;
        }
        let (r, run) = (self.tree.put(question.as_str()), self.tree.nodes[i as usize].cmd);
        let n = &mut self.tree.nodes[i as usize];
        (n.kind, n.cmd, n.name) = (Kind::Confirm { open: true }, NONE, r);
        let q = self.tree.add(i, Kind::Text { mono: false, tone: Tone::Ink, strong: false }, Some(id.mix(1)), at);
        self.tree.nodes[q as usize].text = r;
        let yes = self.tree.add(i, Kind::Button, Some(id.mix(2)), at);
        let no = self.tree.add(i, Kind::Button, Some(id.mix(3)), at);
        (self.tree.nodes[yes as usize].cmd, self.tree.nodes[no as usize].builtin) = (run, Some(crate::builtin::Builtin::Cancel));
        self.tree.nodes[yes as usize].enabled = run != NONE;
    }

    /// A one-line field that types into the `Text` at `path`, with the `completions` that contain what is typed listed under it while it has the focus. Down reaches them, and Return or a click takes one, setting the text by one `Edit`.
    #[track_caller]
    pub fn combo_box<F: Field>(&mut self, path: FactPath<F, Text>, placeholder: Message, completions: &[impl AsRef<str>]) {
        let (at, erased) = (Location::caller(), path.erased());
        let id = Id::ROOT.mix(erased.hash64());
        let typed = self.facts::<F>().and_then(|f| path.get(f)).map_or(String::new(), |t| t.as_str().to_string());
        let name = placeholder.as_str().to_string();
        let i = self.field_as(path, placeholder, Holds::Line, at).node;
        let pick = |k: usize| id.mix(k as u64 + 2);
        if !self.tree.focus.is_some_and(|f| f == id || (0..COMPLETIONS).any(|k| pick(k) == f)) {
            return;
        }
        let wanted = typed.to_lowercase();
        let shown: Vec<&str> = completions.iter().map(AsRef::as_ref).filter(|c| *c != typed && c.to_lowercase().contains(&wanted)).take(COMPLETIONS).collect();
        self.completions(i, erased, &name, &shown, at);
    }

    fn completions(&mut self, i: u32, erased: ledger::ErasedPath, name: &str, shown: &[&str], at: &'static Location<'static>) {
        if shown.is_empty() {
            return;
        }
        let id = self.tree.nodes[i as usize].id;
        let o = self.tree.add(i, Kind::Overlay { anchor: crate::tree::Anchor::Node(i) }, Some(id.mix(1)), at);
        for (k, c) in shown.iter().enumerate() {
            let p = self.tree.add(o, Kind::Pick { on: false }, Some(id.mix(k as u64 + 2)), at);
            let r = self.tree.put(c);
            self.tree.nodes[p as usize].text = r;
            self.set(p, erased, ledger::Value::Str(c.to_string()), format!("{name}: {c}"));
        }
    }

    /// A field for a typed filter over the records of `items`, such as `people>100000 country:spain`, at the `Text` at `path`. While a word is being typed, the record's fields that start with it are offered; a query that can't be read says why under the field. Filter the rows by it with [`Order::matching`](crate::table::Order::matching) in a derived order.
    #[track_caller]
    pub fn filter<T: Record, F: Field>(&mut self, _items: &ledger::List<T>, path: FactPath<F, Text>) {
        let (at, erased) = (Location::caller(), path.erased());
        let id = Id::ROOT.mix(erased.hash64());
        let typed = self.facts::<F>().and_then(|f| path.get(f)).map_or(String::new(), |t| t.as_str().to_string());
        let names = crate::filter::names::<T>();
        let placeholder = format!("Filter by {}", names.join(", "));
        let i = self.field_as(path, Message::plain(placeholder), Holds::Search, at).node;
        if let Err(why) = crate::filter::Filter::parse::<T>(&typed) {
            let w = self.tree.add(self.parent, Kind::Text { mono: false, tone: Tone::Danger, strong: false }, Some(id.mix(0xe7)), at);
            let r = self.tree.put(why.as_str());
            self.tree.nodes[w as usize].text = r;
        }
        let pick = |k: usize| id.mix(k as u64 + 2);
        if !self.tree.focus.is_some_and(|f| f == id || (0..COMPLETIONS).any(|k| pick(k) == f)) || typed.ends_with(char::is_whitespace) {
            return;
        }
        let start = typed.rfind(char::is_whitespace).map_or(0, |s| s + 1);
        let word = typed[start..].trim_start_matches('-');
        if word.is_empty() || word.contains([':', '=', '<', '>', '"']) {
            return;
        }
        let shown: Vec<String> = names.iter().filter(|n| n.starts_with(&word.to_lowercase())).map(|n| format!("{}{n}:", &typed[..typed.len() - word.len()])).take(COMPLETIONS).collect();
        let shown: Vec<&str> = shown.iter().map(String::as_str).collect();
        self.completions(i, erased, "Filter", &shown, at);
    }

    /// A field for the name of a time zone at `path`, such as `Europe/Lisbon`, with the zones whose names contain what is typed listed under it.
    #[track_caller]
    pub fn time_zone<F: Field>(&mut self, path: FactPath<F, Text>, placeholder: Message) {
        static ZONES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        let zones = ZONES.get_or_init(|| {
            let mut zones: Vec<String> = jiff::tz::db().available().map(|name| name.to_string()).filter(|name| name.contains('/')).collect();
            zones.sort();
            zones
        });
        self.combo_box(path, placeholder, zones);
    }

    /// A month of days for the `Day` at `path`, a week to a row from Monday, with the chosen day filled and today underlined. Clicking a day, or Return or Space on it, sets it by one `Edit` named after `label`. The arrows move between days, Home and End to the ends of the week, Page Up and Page Down between months, and the buttons beside the month's name turn it.
    #[track_caller]
    pub fn calendar<F: Field>(&mut self, label: impl AsRef<str>, path: FactPath<F, Day>) {
        let Some(chosen) = self.facts::<F>().and_then(|f| path.get(f)).copied() else { return };
        let (label, at, erased) = (label.as_ref(), Location::caller(), path.erased());
        let id = Id::ROOT.mix(erased.hash64()).mix(7);
        let month = self.tree.months.get(&id).copied().unwrap_or(chosen.month_index());
        let today = self.tree.calendar.today();
        let first = Day::first_of(month);
        let c = self.tree.add(self.parent, Kind::Calendar { month }, Some(id), at);
        let named = self.tree.put(label);
        (self.tree.nodes[c as usize].name, self.tree.nodes[c as usize].subject) = (named, self.subject);
        let h = self.tree.add(c, Kind::Text { mono: false, tone: Tone::Ink, strong: true }, Some(id.mix(1)), at);
        let r = self.tree.put(&first.month_name());
        self.tree.nodes[h as usize].text = r;
        for (k, (forward, name)) in [(false, "Previous month"), (true, "Next month")].into_iter().enumerate() {
            let t = self.tree.add(c, Kind::Turn { forward }, Some(id.mix(k as u64 + 2)), at);
            let r = self.tree.put(name);
            self.tree.nodes[t as usize].name = r;
        }
        for (k, weekday) in ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"].into_iter().enumerate() {
            let w = self.tree.add(c, Kind::Text { mono: true, tone: Tone::Muted, strong: false }, Some(id.mix(4).mix(k as u64)), at);
            let r = self.tree.put(weekday);
            self.tree.nodes[w as usize].text = r;
        }
        let start = first.0 - first.weekday() as i32;
        for day in (start..start + 42).map(Day) {
            let kind = Kind::Day { on: day == chosen, today: day == today, outside: day.month_index() != month };
            let d = self.tree.add(c, kind, Some(day_id(id, day)), at);
            let (r, spoken) = (self.tree.put(&day.parts().2.to_string()), self.tree.put(&day.spoken()));
            (self.tree.nodes[d as usize].text, self.tree.nodes[d as usize].name) = (r, spoken);
            self.set(d, erased, ledger::Value::Int(day.0 as i64), format!("{label}: {day}"));
        }
    }

    /// A day at `path`, typed as `2026-06-03` or in words: `today`, `tomorrow`, `next tue`, `in 3 days`, `jun 3`. What is typed is read when Return is pressed or another field is reached, and the day is set by one `Edit`; the arrows up and down step it by a day, and Escape puts back what was there.
    #[track_caller]
    pub fn date_field<F: Field>(&mut self, path: FactPath<F, Day>) {
        let Some(day) = self.facts::<F>().and_then(|f| path.get(f)).copied() else { return };
        self.when_field(path.erased(), Holds::Date, &day.to_string(), Location::caller());
    }

    /// A time of day at `path`, typed as `15:30`, `3pm` or `noon`. What is typed is read when Return is pressed or another field is reached, and the time is set by one `Edit`; the arrows up and down step the hour or the minute the caret is in, and Escape puts back what was there.
    #[track_caller]
    pub fn time_field<F: Field>(&mut self, path: FactPath<F, Time>) {
        let Some(time) = self.facts::<F>().and_then(|f| path.get(f)).copied() else { return };
        self.when_field(path.erased(), Holds::Time, &time.to_string(), Location::caller());
    }

    /// A date field for the `Day` at `path`, with a button after it that opens a calendar under it. Taking a day in the calendar sets it by one `Edit`, named after `label`, and closes the calendar.
    #[track_caller]
    pub fn date_picker<F: Field>(&mut self, label: impl AsRef<str>, path: FactPath<F, Day>) {
        let (label, at, erased) = (label.as_ref(), Location::caller(), path.erased());
        let row = self.tree.add(self.parent, Kind::Row, Some(Id::ROOT.mix(erased.hash64()).mix(8)), at);
        let subject = self.subject;
        let mut ui = self.child::<S>(row, subject);
        ui.date_field(path);
        ui.popover("Choose…", |ui| ui.calendar(label, path));
    }

    /// A color at `path`, after a swatch of it, typed as hex (`#3366cc`) or OKLCH (`oklch(0.63 0.26 290)`) and shown as hex. What is typed is read when Return is pressed or another field is reached, and the color is set by one `Edit`; Escape puts back what was there.
    #[track_caller]
    pub fn color_field<F: Field>(&mut self, path: FactPath<F, Oklch>) {
        let Some(color) = self.facts::<F>().and_then(|f| path.get(f)).copied() else { return };
        self.when_field(path.erased(), Holds::Color, &color.hex(), Location::caller());
    }

    /// One part of the color at `path`, its lightness, chroma or hue, as a slider whose track shows the colors it sets. It is a slider like any other, over that part's whole numbers.
    #[track_caller]
    pub fn color_slider<F: Field>(&mut self, path: FactPath<F, Oklch>, channel: Channel) {
        let Some(color) = self.facts::<F>().and_then(|f| path.get(f)).copied() else { return };
        let before = self.tree.nodes.len();
        match channel {
            Channel::Lightness => self.slider(path.then(Oklch::l), 0..=1000),
            Channel::Chroma => self.slider(path.then(Oklch::c), 0..=CHROMA as i64),
            Channel::Hue => self.slider(path.then(Oklch::h), 0..=HUE as i64 - 1),
        }
        if let Some(i) = (before..self.tree.nodes.len()).find(|&i| matches!(self.tree.nodes[i].kind, Kind::Slider { .. })) {
            let r = self.tree.put(channel.name());
            self.tree.nodes[i].name = r;
            let r = self.tree.put(&channel.show(i128::from(channel.of(color))));
            self.tree.nodes[i].text = r;
            self.tree.shades.push((i as u32, color, channel));
        }
    }

    /// A box `rows` tall and as wide as it may be, with a space of its own, holding what `f` draws. `label` says what it shows, for screen readers. It is drawn off the main thread, and again only when what it holds changes.
    #[track_caller]
    pub fn canvas(&mut self, label: impl AsRef<str>, rows: i32, f: impl FnOnce(&mut Canvas<'_, 'a, C, S>)) {
        let i = self.add(Kind::Canvas { rows, pressable: false }, Location::caller());
        let r = self.tree.put(label.as_ref());
        let n = &mut self.tree.nodes[i as usize];
        (n.name, n.span) = (r, crate::tree::Fill);
        let mut canvas = Canvas { ui: self, drawing: Drawing::new(), board: Board { node: i, paint: None, drawings: Vec::new(), presses: Vec::new() } };
        f(&mut canvas);
        let Canvas { ui, drawing, mut board } = canvas;
        if !drawing.is_empty() {
            board.drawings.push(Arc::new(drawing));
        }
        ui.tree.nodes[i as usize].kind = Kind::Canvas { rows, pressable: !board.presses.is_empty() };
        ui.tree.canvases.push(board);
    }

    /// The lightness and chroma of the color at `path`, at its hue, as an area to pick them from: chroma grows to the right and lightness upward. Dragging shows where the mark would land and sets both when it is let go, by one `Edit`; the arrows step them.
    #[track_caller]
    pub fn color_area<F: Field>(&mut self, path: FactPath<F, Oklch>) {
        let Some(color) = self.facts::<F>().and_then(|f| path.get(f)).copied() else { return };
        let erased = path.erased();
        let i = self.tree.add(self.parent, Kind::ColorArea { l: color.l, c: color.c, h: color.h }, Some(Id::ROOT.mix(erased.hash64()).mix(10)), Location::caller());
        let r = self.tree.put("Lightness and chroma");
        let n = &mut self.tree.nodes[i as usize];
        (n.subject, n.path, n.name) = (self.subject, Some(erased), r);
    }

    /// A color field for the color at `path` with a button after it that opens an area of lightness and chroma and a slider of hues under it.
    #[track_caller]
    pub fn color_picker<F: Field>(&mut self, path: FactPath<F, Oklch>) {
        let (at, erased) = (Location::caller(), path.erased());
        let row = self.tree.add(self.parent, Kind::Row, Some(Id::ROOT.mix(erased.hash64()).mix(11)), at);
        let subject = self.subject;
        let mut ui = self.child::<S>(row, subject);
        ui.color_field(path);
        ui.popover("Choose…", |ui| {
            ui.color_area(path);
            ui.color_slider(path, Channel::Hue);
        });
    }

    fn when_field(&mut self, path: ledger::ErasedPath, holds: Holds, now: &str, at: &'static Location<'static>) {
        let i = self.tree.add(self.parent, Kind::Field { holds }, Some(Id::ROOT.mix(path.hash64())), at);
        let r = self.tree.put(now);
        let node = &mut self.tree.nodes[i as usize];
        (node.subject, node.path, node.text) = (self.subject, Some(path), r);
    }

    /// Words that tag something, in the list at `path`: each a tag that removes itself when pressed, then a field to type another. Return adds what was typed, and so does moving to another field; Backspace in the empty field removes the last tag. Each change is one `Edit`, named after `label`.
    #[track_caller]
    pub fn tag_field<F: Field>(&mut self, label: impl AsRef<str>, path: FactPath<F, Vec<String>>) {
        let Some(tags) = self.facts::<F>().and_then(|f| path.get(f)).cloned() else { return };
        let (label, at, erased) = (label.as_ref(), Location::caller(), path.erased());
        let id = Id::ROOT.mix(erased.hash64());
        let row = self.tree.add(self.parent, Kind::Row, Some(id.mix(1)), at);
        self.tree.nodes[row as usize].span = crate::tree::Fill;
        for (k, tag) in tags.iter().enumerate() {
            let b = self.tree.add(row, Kind::Chip, Some(id.mix(2).mix(k as u64)), at);
            let (r, named) = (self.tree.put(tag), self.tree.put(&format!("Remove “{tag}”")));
            (self.tree.nodes[b as usize].text, self.tree.nodes[b as usize].name) = (r, named);
            let rest = tags.iter().enumerate().filter(|&(j, _)| j != k).map(|(_, t)| ledger::Value::Str(t.clone())).collect();
            self.set(b, erased, ledger::Value::List(rest), format!("{label}: remove “{tag}”"));
        }
        let f = self.tree.add(row, Kind::Field { holds: Holds::Tags }, Some(id), at);
        let r = self.tree.put(label);
        let n = &mut self.tree.nodes[f as usize];
        (n.path, n.name, n.subject, n.span) = (Some(erased), r, self.subject, crate::tree::Fill.min(12));
    }

    fn whole<F: Field, T: Setting>(&self, path: FactPath<F, T>) -> Option<(i128, i128, i128)> {
        let Control::Whole { least, most } = T::control() else { return None };
        let now = match self.facts::<F>().and_then(|f| path.get(f)).map(Field::value)? {
            ledger::Value::Int(n) => n as i128,
            ledger::Value::UInt(n) => n as i128,
            _ => return None,
        };
        Some((now, least, most))
    }

    fn field_as<F: Field>(&mut self, path: FactPath<F, Text>, placeholder: Message, holds: Holds, at: &'static Location<'static>) -> Placed<'_, 'a, C, S> {
        let erased = path.erased();
        let id = Id::ROOT.mix(erased.hash64());
        let i = self.tree.add(self.parent, Kind::Field { holds }, Some(id), at);
        let node = &mut self.tree.nodes[i as usize];
        node.subject = self.subject;
        node.path = Some(erased);
        let value = self.facts::<F>().and_then(|f| path.get(f)).map_or("", |t| t.as_str());
        let r = self.tree.put(value);
        self.tree.nodes[i as usize].text = r;
        let r = self.tree.put(placeholder.as_str());
        self.tree.nodes[i as usize].name = r;
        if !matches!(holds, Holds::Inline | Holds::Code { .. }) {
            self.tree.nodes[i as usize].span = crate::tree::Fill.min(16);
        }
        Placed { ui: self, node: i }
    }

    /// Files dropped from the system onto what is being described here run `cmd`, one command per file: `ui.on_files(Cmd::Import)`. While files are dragged over the window, the status line says what dropping them would do.
    pub fn on_files(&mut self, cmd: fn(Path) -> C) {
        self.tree.files.push((self.parent, cmd));
    }

    /// A list of `items`, scrolled by whole rows, whose selection lives at `selection`. `row` describes one item; it is called only for the rows that show, so a list of fifty million costs what a screenful does. `items` is a list, or a list in an order worked out in the background: `self.order.of(&self.files)`.
    #[track_caller]
    pub fn list<'r, T: Record, F: Field>(&mut self, items: impl Into<Rows<'r, T>>, selection: FactPath<F, Selection<T>>, mut row: impl FnMut(&mut Ui<'_, C, Key<T>>, &T)) -> Listed<'_, 'a, C, S, T> {
        let i = self.rows(&items.into(), selection, Location::caller(), false, |_, _, _, _| {}, |ui, item, _| row(ui, item));
        Listed { ui: self, node: i, items: PhantomData }
    }

    /// A tree of `items`: a list whose rows sit inside one another, each branch opened and closed by the keys in the `Vec` at `open`. Give it the rows in an order made by [`Order::tree`](crate::table::Order::tree) from the list and `open`, worked out in the background; `row` describes one item, after its twig. A click on a twig, or Right and Left on the focused row, opens and closes it; Right on an open branch goes into it and Left on a row inside one goes up to it.
    #[track_caller]
    pub fn tree<'r, T: Record, F: Field>(
        &mut self,
        items: impl Into<Rows<'r, T>>,
        selection: FactPath<F, Selection<T>>,
        open: FactPath<F, Vec<Key<T>>>,
        mut row: impl FnMut(&mut Ui<'_, C, Key<T>>, &T),
    ) -> Listed<'_, 'a, C, S, T> {
        let at = Location::caller();
        let items = items.into();
        let now: Vec<Key<T>> = self.facts::<F>().and_then(|f| open.get(f)).cloned().unwrap_or_default();
        let path = open.erased();
        let twig = |ui: &mut Self, r: u32, index: usize, key: u128| {
            let Some(nest) = items.nest(index) else { return };
            let t = ui.tree.add(r, Kind::Twig { depth: nest.depth, branch: nest.branch }, Some(ui.tree.nodes[r as usize].id.mix(0x7419)), at);
            ui.tree.nodes[t as usize].span = crate::tree::cols(i32::from(nest.depth) * 2 + 2);
            let next: Vec<Key<T>> = match nest.branch {
                Branch::Leaf => return,
                Branch::Open => now.iter().copied().filter(|k| k.raw() != key).collect(),
                Branch::Closed => now.iter().copied().chain([Key::from_raw(key)]).collect(),
            };
            let said = if nest.branch == Branch::Open { "Close the branch" } else { "Open the branch" };
            ui.set(t, path, Field::value(&next), said.to_string());
        };
        let i = self.rows(&items, selection, at, false, twig, |ui, item, _| row(ui, item));
        Listed { ui: self, node: i, items: PhantomData }
    }

    /// A table of `items`: a list whose rows are cells in named columns under a row of heads. `row` gives an item's cells, `row.cell(t!("Size"), file.size)`, the same columns in the same order for every item; it is called only for the rows that show. Columns share the width, and the user can drag the gap after a head to make its column wider or narrower; in a narrow window the columns that don't fit are left out from the right. With `.sort(Files::sort)` pressing a head sorts by its column.
    #[track_caller]
    pub fn table<'r, T: Record, F: Field>(&mut self, items: impl Into<Rows<'r, T>>, selection: FactPath<F, Selection<T>>, mut row: impl FnMut(&mut Row<'_, '_, C, T>, &T)) -> Tabled<'_, 'a, C, S, T> {
        let i = self.rows(&items.into(), selection, Location::caller(), true, |_, _, _, _| {}, |ui, item, heads| row(&mut Row { ui, heads, column: 0 }, item));
        Tabled { ui: self, node: i, items: PhantomData }
    }

    /// A field to type in over the items whose words match it, best first, eight at a time with one highlighted. Give it the items in an order made by [`Order::found`](crate::table::Order::found) from the list and the `Text` at `filter`, worked out in the background; `row` describes one item, and the letters typed are marked in its first text. Up and Down in the field move the highlight, and Return or a click runs the command given to `on_choose` for the item.
    #[track_caller]
    pub fn picker<'r, T: Record, F: Field>(&mut self, items: impl Into<Rows<'r, T>>, filter: FactPath<F, Text>, mut row: impl FnMut(&mut Ui<'_, C, Key<T>>, &T)) -> Picked<'_, 'a, C, S, T> {
        let (at, items, subject) = (Location::caller(), items.into(), self.subject);
        let id = Id::ROOT.mix(filter.erased().hash64()).mix(0x91c);
        let i = self.tree.add(self.parent, Kind::Picker, Some(id), at);
        let query = self.facts::<F>().and_then(|f| filter.get(f)).map_or(String::new(), |t| t.as_str().to_string());
        self.child::<S>(i, subject).field_as(filter, Message::from("Find"), Holds::Search, at);
        let len = items.len() as u32;
        let picking = self.tree.picks.get(&id).copied().filter(|p| p.query == crate::search::hashed(&query)).unwrap_or_default();
        let chosen = picking.at.min(len.saturating_sub(1));
        let first = picking.first.min(chosen).max((chosen + 1).saturating_sub(PICKED));
        let m = self.tree.add(i, Kind::Matches { count: len, first, shown: 0, at: chosen }, Some(id.mix(1)), at);
        let mut shown = 0;
        for index in first..(first + PICKED).min(len) {
            let Some(item) = items.get(index as usize) else { break };
            let raw = item.key().raw();
            let r = self.tree.add(m, Kind::Match { chosen: index == chosen }, Some(id.mix(raw as u64).mix((raw >> 64) as u64)), at);
            self.tree.nodes[r as usize].subject = Some(raw);
            row(&mut self.child(r, Some(raw)), item);
            if let Some(t) = self.tree.children(r).find(|&c| matches!(self.tree.nodes[c as usize].kind, Kind::Text { .. })) {
                let marks = crate::search::marked(&query, self.tree.str(self.tree.nodes[t as usize].text));
                if !marks.is_empty() {
                    self.tree.marks.push((t, marks));
                }
            }
            shown += 1;
        }
        if let Kind::Matches { shown: s, .. } = &mut self.tree.nodes[m as usize].kind {
            *s = shown;
        }
        if len == 0 && !query.trim().is_empty() {
            let said = if items.coming() { "Searching…".to_string() } else { format!("Nothing matches “{}”", query.trim()) };
            self.child::<S>(i, subject).muted(said);
        }
        Picked { ui: self, node: m, items: PhantomData }
    }

    fn rows<T: Record, F: Field>(
        &mut self,
        items: &Rows<'_, T>,
        selection: FactPath<F, Selection<T>>,
        at: &'static Location<'static>,
        table: bool,
        mut twig: impl FnMut(&mut Self, u32, usize, u128),
        mut row: impl FnMut(&mut Ui<'_, C, Key<T>>, &T, bool),
    ) -> u32 {
        let i = self.add(Kind::List { count: items.len() as u32, first: 0, shown: 0, rows: 0 }, at);
        let erased = selection.erased();
        self.tree.nodes[i as usize].path = Some(erased);
        let id = self.tree.nodes[i as usize].id;
        let scroll = self.scrolls.get(&id).copied().unwrap_or_default();
        let len = items.len();
        let anchor = scroll.anchor.filter(|_| items.ranks().is_none());
        let first = anchor.and_then(|k| items.position(Key::from_raw(k))).unwrap_or_else(|| (scroll.index as usize).min(len.saturating_sub(1)));
        let want = (scroll.rows.max(1) + 1) as usize;
        let chosen = self.facts::<F>().and_then(|f| selection.get(f));
        if table && let Some(item) = items.get(first) {
            let header = self.tree.add(i, Kind::Header, Some(id.mix(0x4ead)), at);
            row(&mut self.child(header, None), item, true);
        }
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
            twig(self, r, index, raw);
            row(&mut self.child(r, Some(raw)), item, false);
            shown = n as u32 + 1;
        }
        self.tree.nodes[i as usize].kind = Kind::List { count: len as u32, first: first as u32, shown, rows: 0 };
        let focused = chosen.and_then(|s| s.focused());
        let focus_index = focused.and_then(|k| items.position(k));
        let near = |d: isize| focus_index.and_then(|p| p.checked_add_signed(d)).and_then(|p| items.get(p)).map(|t| t.key().raw());
        self.tree.nodes[i as usize].list = self.tree.lists.len() as u32;
        self.tree.lists.push(ListInfo {
            node: i,
            item: std::any::TypeId::of::<T>(),
            items: items.list().path(),
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
            order: match items.ranks() {
                Some(r) => {
                    self.tree.orders.push(r.clone());
                    self.tree.orders.len() as u32 - 1
                }
                None => NONE,
            },
        });
        i
    }

    /// What the page shows when there is nothing yet: a sentence, and the command that makes something.
    #[track_caller]
    pub fn empty<M>(&mut self, message: Message, cmd: impl Bind<C, S, M>) {
        let i = self.add(Kind::Empty, Location::caller());
        let subject = self.subject;
        let mut inside = self.child::<S>(i, subject);
        inside.muted(message.as_str());
        inside.button(cmd);
    }
}

impl<C> Tree<C> {
    /// The row a node sits in, if any, and its key.
    pub fn row_of(&self, i: u32) -> Option<u32> {
        self.ancestor(i, |n| matches!(n.kind, Kind::ListRow { .. } | Kind::Overlay { .. } | Kind::Confirm { open: true })).filter(|&r| matches!(self.nodes[r as usize].kind, Kind::ListRow { .. }))
    }

    pub fn in_row(&self, i: u32) -> bool {
        let parent = self.nodes[i as usize].parent;
        parent != NONE && self.row_of(parent).is_some()
    }
}

const COMPLETIONS: usize = 8;

/// A canvas being drawn: draw on it as on a [`Drawing`], hand it drawings made elsewhere, and name the shapes that run a command when pressed.
pub struct Canvas<'u, 'a, C, S> {
    ui: &'u mut Ui<'a, C, S>,
    drawing: Drawing,
    board: Board,
}

impl<C, S> Canvas<'_, '_, C, S> {
    /// Paints the canvas with the app's own GPU commands first, under everything drawn on it.
    pub fn paint(&mut self, hook: &crate::canvas::PaintHook) {
        self.board.paint = hook.get().cloned();
    }

    /// Draws `drawing` too, under what is drawn on the canvas itself. A drawing made in a job is handed over this way, so the view never builds its shapes.
    pub fn drawing(&mut self, drawing: &Arc<Drawing>) {
        self.board.drawings.push(drawing.clone());
    }

    /// Runs `cmd` when the shape named `key` is pressed: by a click, or by the arrows and Return once the canvas is reached.
    pub fn press<M>(&mut self, key: u64, cmd: impl Bind<C, S, M>) {
        if let Some(cmd) = cmd.bind(self.ui.subject) {
            self.board.presses.push((key, self.ui.tree.commands.len() as u32));
            self.ui.tree.commands.push(cmd);
        }
    }
}

impl<C, S> Deref for Canvas<'_, '_, C, S> {
    type Target = Drawing;

    fn deref(&self) -> &Drawing {
        &self.drawing
    }
}

impl<C, S> DerefMut for Canvas<'_, '_, C, S> {
    fn deref_mut(&mut self) -> &mut Drawing {
        &mut self.drawing
    }
}

/// The id of `day`'s cell in the calendar with id `calendar`, so moving between days can name the one to focus before it is built.
pub(crate) fn day_id(calendar: Id, day: Day) -> Id {
    calendar.mix(9).mix(day.0 as u32 as u64)
}

fn keys<F: Field, T: Record>(facts: &dyn Any, items: ledger::ErasedPath, selection: ledger::ErasedPath, ranks: Option<&crate::table::Ranks>, span: Option<(u128, u128)>) -> Vec<u128> {
    let Some(facts) = facts.downcast_ref::<F>() else { return Vec::new() };
    let Some(list) = facts.reach(items.steps()).and_then(|l| l.downcast_ref::<ledger::List<T>>()) else { return Vec::new() };
    if let Some(r) = ranks {
        return match span {
            Some((from, to)) => {
                let (Some(a), Some(b)) = (r.position(from), r.position(to)) else { return Vec::new() };
                (a.min(b)..=a.max(b)).filter_map(|i| r.key(i)).filter(|&k| list.has(Key::from_raw(k))).collect()
            }
            None => {
                let Some(chosen) = facts.reach(selection.steps()).and_then(|s| s.downcast_ref::<Selection<T>>()) else { return Vec::new() };
                let mut keys: Vec<(usize, u128)> = chosen.iter().filter_map(|k| r.position(k.raw()).map(|p| (p, k.raw()))).collect();
                keys.sort_unstable();
                keys.into_iter().map(|(_, k)| k).collect()
            }
        };
    }
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

fn nth<F: Field, T: Record>(facts: &dyn Any, items: ledger::ErasedPath, ranks: Option<&crate::table::Ranks>, index: u32) -> Option<u128> {
    let list = facts.downcast_ref::<F>()?.reach(items.steps())?.downcast_ref::<ledger::List<T>>()?;
    match ranks {
        Some(r) => r.key(index as usize).filter(|&k| list.has(Key::from_raw(k))),
        None => list.get(index as usize).map(|t| t.key().raw()),
    }
}

/// One row of a table being described: its cells, left to right.
pub struct Row<'u, 'a, C, T> {
    ui: &'u mut Ui<'a, C, Key<T>>,
    heads: bool,
    column: u16,
}

impl<C, T> Row<'_, '_, C, T> {
    /// A cell in the column named `label`, showing `value`: words from the left, numbers lined up on the right.
    #[track_caller]
    pub fn cell(&mut self, label: impl Into<Message>, value: impl Cell) -> Celled<'_, C> {
        let label = label.into();
        let at = Location::caller();
        let column = self.column;
        self.column += 1;
        let tree = &mut *self.ui.tree;
        let parent = self.ui.parent;
        if self.heads {
            let id = tree.nodes[parent as usize].id.named(label.as_str());
            let i = tree.add(parent, Kind::Head { sorted: 0, sortable: false, numeric: value.numeric() }, Some(id), at);
            let r = tree.put(label.as_str());
            tree.nodes[i as usize].text = r;
            let wide = label.as_str().chars().count() as i32 + 2;
            tree.nodes[i as usize].span = match tree.resized.get(&id) {
                Some(&w) => crate::tree::cols(w),
                None if value.numeric() => crate::tree::cols(wide.max(10)).min(wide.min(6)),
                None => crate::tree::Fill.min(wide.min(10)),
            };
            return Celled { tree, node: i, sign: 0, heads: true };
        }
        let i = tree.add(parent, Kind::Cell { numeric: value.numeric(), tone: Tone::Ink }, Some(tree.nodes[parent as usize].id.mix(u64::from(column))), at);
        let r = tree.put(&value.shown());
        tree.nodes[i as usize].text = r;
        let r = tree.put(label.as_str());
        tree.nodes[i as usize].name = r;
        Celled { tree, node: i, sign: value.sign(), heads: false }
    }
}

/// A cell just placed, to finish describing: `row.cell(t!("Change"), quote.change).tone_by_sign()`.
pub struct Celled<'t, C> {
    tree: &'t mut Tree<C>,
    node: u32,
    sign: i8,
    heads: bool,
}

impl<C> Celled<'_, C> {
    /// Its number in the ok ink above zero and the danger ink below.
    pub fn tone_by_sign(self) -> Self {
        if !self.heads
            && let Kind::Cell { tone, .. } = &mut self.tree.nodes[self.node as usize].kind
        {
            *tone = crate::table::toned(self.sign, true);
        }
        self
    }

    /// How wide its column starts, in columns: `.width(12)`, `.width(Fill.min(20))`. The user can still drag it wider or narrower.
    pub fn width(self, span: impl Into<Span>) -> Self {
        if self.heads && !self.tree.resized.contains_key(&self.tree.nodes[self.node as usize].id) {
            self.tree.nodes[self.node as usize].span = span.into();
        }
        self
    }
}

/// A table just placed, to finish describing: `ui.table(…).sort(Files::sort)`.
pub struct Tabled<'u, 'a, C, S, T> {
    ui: &'u mut Ui<'a, C, S>,
    node: u32,
    items: PhantomData<fn() -> T>,
}

impl<C, S, T> Tabled<'_, '_, C, S, T> {
    /// Pressing a column's head sorts by it, then the other way, then not at all, by setting the [`Sort`] at `path`. Give the table the rows in the order worked out from it.
    pub fn sort<F: Field>(self, path: FactPath<F, Sort>) -> Self {
        let Some(now) = self.ui.facts::<F>().and_then(|f| path.get(f)).cloned() else { return self };
        let erased = path.erased();
        let Some(header) = self.ui.tree.children(self.node).find(|&c| self.ui.tree.nodes[c as usize].kind == Kind::Header) else { return self };
        let heads: Vec<u32> = self.ui.tree.children(header).collect();
        for h in heads {
            let label = self.ui.tree.str(self.ui.tree.nodes[h as usize].text).to_string();
            let next = now.pressed(&label);
            let sorted = match (now.column() == label, now.descending()) {
                (false, _) => 0,
                (true, false) => 1,
                (true, true) => -1,
            };
            if let Kind::Head { numeric, .. } = self.ui.tree.nodes[h as usize].kind {
                self.ui.tree.nodes[h as usize].kind = Kind::Head { sorted, sortable: true, numeric };
            }
            let said = next.said();
            self.ui.set(h, erased, next.value(), said);
        }
        self
    }

    /// How many rows tall it is, its heads included, when it sits among other things rather than filling the page.
    pub fn height(self, rows: i32) -> Self {
        if let Kind::List { rows: r, .. } = &mut self.ui.tree.nodes[self.node as usize].kind {
            *r = rows.max(3);
        }
        self
    }
}
