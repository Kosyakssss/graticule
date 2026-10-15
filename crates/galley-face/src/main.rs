use std::collections::HashMap;
use std::sync::Arc;

use galley::{Doc, Font, Hyphenation, Mark, Marks, Replacement, Segment, Setting, Stale, Typeset};
use graticule::kernel::{Camera, Drawing, Weight};
use graticule::ledger::{Migration, Value};
use graticule::prelude::*;

#[derive(Facts)]
struct Galley {
    buffers: List<Buffer>,
    #[session]
    showing: Key<Buffer>,
    #[session]
    beside: Option<Key<Buffer>>,
    #[session]
    file: Text,
    #[session]
    find: Text,
    #[session]
    with: Text,
    font: Text,
    paper: Paper,
    #[session]
    reading: Reading,
    #[session]
    camera: Camera,
    #[derived(marked, from = [buffers, showing, file])]
    marks: Derived<Marks>,
    #[derived(Pages::of, from = [buffers, showing, font, paper, reading])]
    pages: Derived<Pages>,
    #[derived(Sheets::of, from = [pages, camera])]
    sheets: Derived<Sheets>,
}

/// What the main pane shows: the text to edit, or the text typeset on pages.
#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
enum Reading {
    #[default]
    Text,
    Pages,
}

/// The paper the pages are set on.
#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
enum Paper {
    #[default]
    A4,
    Letter,
}

impl Paper {
    fn galley(self) -> galley::Paper {
        match self {
            Paper::A4 => galley::Paper::A4,
            Paper::Letter => galley::Paper::Letter,
        }
    }
}

/// The shown buffer typeset in the chosen font, with the outline of every glyph it uses, worked out off the main thread.
#[derive(Default)]
struct Pages {
    set: Option<Typeset>,
    outlines: HashMap<u16, Vec<Segment>>,
    missing: Option<String>,
}

/// The space between pages on the canvas, in points.
const GAP: f32 = 24.0;

/// The most a canvas shows, in points: what is in sight through the camera on the largest screen.
const SIGHT: (f64, f64) = (3_840.0, 2_160.0);

impl Pages {
    fn of(buffers: &List<Buffer>, showing: &Key<Buffer>, font: &Text, paper: &Paper, reading: &Reading) -> Pages {
        if *reading == Reading::Text {
            return Pages::default();
        }
        let Some(buffer) = buffers.find(*showing).or(buffers.first()) else { return Pages::default() };
        let Some(set) = typeset(&buffer.text.to_string(), font.as_str(), *paper) else { return Pages { missing: Some(font.to_string()), ..Pages::default() } };
        let mut outlines = HashMap::new();
        for g in set.pages.iter().flat_map(|p| &p.lines).flat_map(|l| &l.glyphs) {
            outlines.entry(g.id).or_insert_with(|| set.font.outline(g.id));
        }
        Pages { set: Some(set), outlines, missing: None }
    }
}

/// Drawings of the pages in sight through the camera, one to a page.
#[derive(Default)]
struct Sheets {
    pages: Vec<Arc<Drawing>>,
}

impl Sheets {
    fn of(pages: &Derived<Pages>, camera: &Camera) -> Sheets {
        let Some(pages) = pages.get() else { return Sheets::default() };
        let Some(set) = &pages.set else { return Sheets::default() };
        let (width, height) = set.setting.paper.size();
        let [_, _, top, bottom] = camera.showing(SIGHT.0, SIGHT.1);
        let step = f64::from(height + GAP);
        let first = (top / step).floor().max(0.0) as usize;
        let last = ((bottom / step).ceil().max(0.0) as usize).min(set.pages.len());
        let size = set.setting.size;
        let sheets = (first..last)
            .map(|k| {
                let mut drawing = Drawing::new();
                let y0 = k as f32 * (height + GAP);
                drawing.rect((0.0, y0), (width, y0 + height)).fill().ink(Ink::Field);
                drawing.rect((0.0, y0), (width, y0 + height)).stroke(Weight::Hair).ink(Ink::Rule);
                for line in &set.pages[k].lines {
                    let base = y0 + line.baseline;
                    drawing
                        .path(|pen| {
                            for g in &line.glyphs {
                                let at = |x: f32, y: f32| (g.x + x * size, base - y * size);
                                for segment in pages.outlines.get(&g.id).into_iter().flatten() {
                                    match *segment {
                                        Segment::Move(x, y) => pen.move_to(at(x, y)),
                                        Segment::Line(x, y) => pen.line_to(at(x, y)),
                                        Segment::Quad(cx, cy, x, y) => pen.quad_to(at(cx, cy), at(x, y)),
                                        Segment::Cubic(ax, ay, bx, by, x, y) => pen.curve_to(at(ax, ay), at(bx, by), at(x, y)),
                                        Segment::Close => pen.close(),
                                    };
                                }
                            }
                        })
                        .fill()
                        .ink(Ink::Ink);
                }
                let _ = drawing.stamp();
                Arc::new(drawing)
            })
            .collect();
        Sheets { pages: sheets }
    }
}

fn typeset(text: &str, font: &str, paper: Paper) -> Option<Typeset> {
    let (data, index) = graticule::kernel::fonts::family(font)?;
    let font = Font::new(data, index)?;
    Some(galley::set(text, &font, Setting { paper: paper.galley(), ..Setting::default() }, Some(Hyphenation::english())))
}

#[derive(Facts)]
struct Buffer {
    #[key]
    id: Key<Buffer>,
    name: String,
    text: Doc,
}

#[derive(Commands)]
enum Cmd {
    #[label("Open")]
    Open,
    #[internal]
    Opened(Result<String, Failure>),
    #[label("Save")]
    Save,
    #[internal]
    Saved(Result<Path, Failure>),
    #[label("New buffer")]
    NewBuffer,
    #[label("Next buffer")]
    NextBuffer,
    #[label("Previous buffer")]
    PreviousBuffer,
    #[label("Close the buffer")]
    CloseBuffer,
    #[label("Split")]
    Split,
    #[label("Close the split")]
    Unsplit,
    #[label("Swap the panes")]
    SwapPanes,
    #[label("Select every match")]
    SelectMatches,
    #[label("Find the next")]
    FindNext,
    #[label("Find the previous")]
    FindPrevious,
    #[internal]
    Found(Option<galley::Next>),
    #[label("Add a cursor above")]
    AddCursorAbove,
    #[label("Add a cursor below")]
    AddCursorBelow,
    #[label("Add the next match")]
    AddNextMatch,
    #[internal]
    NextFound(Option<galley::Next>),
    #[label("Replace all")]
    ReplaceAll,
    #[internal]
    Replaced(Replacement),
    #[label("Fold the block")]
    Fold,
    #[internal]
    Folding(Option<galley::Block>),
    #[label("Unfold")]
    Unfold,
    #[label("Unfold everything")]
    UnfoldAll,
    #[label("Show the pages")]
    ShowPages,
    #[label("Show the text")]
    ShowText,
    #[label("Use A4 paper")]
    UseA4,
    #[label("Use US Letter paper")]
    UseLetter,
    #[label("Save as a PDF…")]
    SavePdf,
    #[internal]
    SavedPdf(Result<Path, Failure>),
    #[internal]
    Edit(Edit),
}

impl App for Galley {
    type Cmd = Cmd;

    fn available(&self, cmd: &Cmd) -> Available {
        let text = self.text();
        let line = text.line_of(text.selections().primary().head);
        match cmd {
            Cmd::Open | Cmd::Save => no_if(self.file.is_empty(), t!("Choose a file first")),
            Cmd::NextBuffer | Cmd::PreviousBuffer if self.beside.is_some() => no_if(self.buffers.len() < 3, t!("Every other buffer is showing beside this one")),
            Cmd::NextBuffer | Cmd::PreviousBuffer => no_if(self.buffers.len() < 2, t!("There is only one buffer")),
            Cmd::CloseBuffer => no_if(self.buffers.len() < 2, t!("The last buffer stays open")),
            Cmd::Split if self.beside.is_some() => no_if(true, t!("The window is split already")),
            Cmd::Split => no_if(self.buffers.len() < 2, t!("Open a second buffer to show beside this one")),
            Cmd::Unsplit | Cmd::SwapPanes => no_if(self.beside.is_none(), t!("The window isn't split")),
            Cmd::SelectMatches | Cmd::ReplaceAll | Cmd::FindNext | Cmd::FindPrevious => no_if(self.find.is_empty(), t!("Type what to find first")),
            Cmd::AddCursorAbove => no_if(line == 0, t!("The cursor is on the first line")),
            Cmd::AddCursorBelow => no_if(line + 1 >= text.lines(), t!("The cursor is on the last line")),
            Cmd::AddNextMatch => no_if(text.selections().primary().is_empty(), t!("Select some text first")),
            Cmd::Fold => no_if(text.lines() < 2, t!("There is only one line")),
            Cmd::Unfold => no_if(text.folded(line) == 0, t!("Nothing is folded under this line")),
            Cmd::UnfoldAll => no_if(text.folds().is_empty(), t!("Nothing is folded")),
            Cmd::ShowPages => no_if(self.reading == Reading::Pages, t!("The pages show already")),
            Cmd::ShowText => no_if(self.reading == Reading::Text, t!("The text shows already")),
            Cmd::UseA4 => no_if(self.paper == Paper::A4, t!("The pages are A4 already")),
            Cmd::UseLetter => no_if(self.paper == Paper::Letter, t!("The pages are US Letter already")),
            Cmd::SavePdf => match self.pages.get().and_then(|p| p.missing.as_deref()) {
                Some(font) => no_if(true, t!("There is no font called “{font}”")),
                None => no_if(self.font.is_empty(), t!("Name a font first")),
            },
            Cmd::SavedPdf(_) | Cmd::NewBuffer | Cmd::Folding(_) | Cmd::Opened(_) | Cmd::Saved(_) | Cmd::Replaced(_) | Cmd::NextFound(_) | Cmd::Found(_) | Cmd::Edit(_) => Yes,
        }
    }

    fn update(&mut self, cmd: Cmd, cx: &mut Cx<Cmd>) -> Outcome {
        match cmd {
            Cmd::Open => {
                let file = self.file.to_string();
                cx.job(t!("Opening {file}"), move || std::fs::read_to_string(&file).map_err(|e| Failure::new(e.to_string())), Cmd::Opened);
                Outcome::Nothing
            }
            Cmd::Opened(Ok(text)) => {
                let name = std::path::Path::new(self.file.as_str()).file_name().map_or_else(|| self.file.to_string(), |n| n.to_string_lossy().into_owned());
                self.open(Buffer { id: cx.key(), name, text: Doc::new(&text) })
            }
            Cmd::Opened(Err(failure)) | Cmd::Saved(Err(failure)) => cx.say_failure(failure),
            Cmd::Save => {
                cx.write(Path(self.file.to_string().into()), |facts: &Galley| facts.text().to_string(), Cmd::Saved);
                Outcome::Nothing
            }
            Cmd::Saved(Ok(path)) => cx.say(t!("Saved to {path}")),
            Cmd::NewBuffer => {
                let count = self.buffers.len() + 1;
                let name = t!("Untitled {count}").to_string();
                self.open(Buffer { id: cx.key(), name, text: Doc::default() })
            }
            Cmd::NextBuffer | Cmd::PreviousBuffer => {
                let at = self.buffers.position(self.shown()).unwrap_or(0);
                let count = self.buffers.len().max(1);
                let step = if matches!(cmd, Cmd::NextBuffer) { 1 } else { count - 1 };
                let mut to = (at + step) % count;
                if self.buffers.get(to).map(|b| b.id) == self.beside {
                    to = (to + step) % count;
                }
                if let Some(buffer) = self.buffers.get(to) {
                    self.showing = buffer.id;
                }
                Outcome::Nothing
            }
            Cmd::CloseBuffer => {
                let gone = self.shown();
                let at = self.buffers.position(gone).unwrap_or(0);
                let outcome = self.buffers.remove(gone);
                if let Some(buffer) = self.buffers.get(at.min(self.buffers.len().saturating_sub(1))) {
                    self.showing = buffer.id;
                }
                if self.beside == Some(gone) || self.beside == Some(self.shown()) {
                    self.beside = None;
                }
                outcome
            }
            Cmd::Split => {
                let at = self.buffers.position(self.shown()).unwrap_or(0);
                self.beside = self.buffers.get((at + 1) % self.buffers.len().max(1)).map(|b| b.id);
                Outcome::Nothing
            }
            Cmd::Unsplit => {
                self.beside = None;
                Outcome::Nothing
            }
            Cmd::SwapPanes => {
                if let Some(other) = self.beside {
                    self.beside = Some(self.shown());
                    self.showing = other;
                }
                Outcome::Nothing
            }
            Cmd::SelectMatches => {
                let find_text = self.find.to_string();
                match self.change(|text| text.select_matches(&find_text)).unwrap_or(0) {
                    0 => cx.say(t!("Nothing matches")),
                    count => cx.say(t!("Selected {count} matches")),
                }
            }
            Cmd::FindNext | Cmd::FindPrevious => {
                let (text, find, back) = (self.text().clone(), self.find.to_string(), matches!(cmd, Cmd::FindPrevious));
                cx.job(t!("Finding “{find}”"), move || text.find_from(&find, back), Cmd::Found);
                Outcome::Nothing
            }
            Cmd::Found(None) => cx.say(t!("Nothing matches")),
            Cmd::Found(Some(next)) => match self.change(|text| text.go_to(&next).map(|()| text.line_of(text.selections().primary().head) + 1)).unwrap_or(Err(Stale)) {
                Ok(line) => cx.say(t!("Found on line {line}")),
                Err(_) => cx.say(t!("The text changed while it was being searched")),
            },
            Cmd::AddCursorAbove | Cmd::AddCursorBelow => {
                let down = matches!(cmd, Cmd::AddCursorBelow);
                let (before, count) = self
                    .change(|text| {
                        let before = text.selections().spans().len();
                        text.add_cursor_line(down);
                        (before, text.selections().spans().len())
                    })
                    .unwrap_or_default();
                match count {
                    count if count == before => cx.say(t!("There is a cursor there already")),
                    count => cx.say(t!("{count} cursors")),
                }
            }
            Cmd::AddNextMatch => {
                let text = self.text().clone();
                cx.job(t!("Finding the next match"), move || text.next_match(), Cmd::NextFound);
                Outcome::Nothing
            }
            Cmd::NextFound(None) => Outcome::Nothing,
            Cmd::NextFound(Some(next)) => match self.change(|text| text.take_next(&next).map(|added| (added, text.selections().spans().len()))).unwrap_or(Err(Stale)) {
                Ok((true, count)) => cx.say(t!("{count} selections")),
                Ok((false, _)) => cx.say(t!("Every match is selected already")),
                Err(_) => cx.say(t!("The text changed while the next match was being found")),
            },
            Cmd::ReplaceAll => {
                let (text, find, with) = (self.text().clone(), self.find.to_string(), self.with.to_string());
                cx.job(t!("Replacing “{find}”"), move || text.replacement(&find, &with), Cmd::Replaced);
                Outcome::Nothing
            }
            Cmd::Replaced(replacement) => {
                let count = replacement.count();
                match self.change(|text| text.take(replacement)).unwrap_or(Err(Stale)) {
                    Ok(_) if count == 0 => cx.say(t!("Nothing matches")),
                    Ok(outcome) => {
                        let _ = cx.say(t!("Replaced {count} matches"));
                        outcome
                    }
                    Err(_) => cx.say(t!("The text changed while the matches were being found, so nothing was replaced")),
                }
            }
            Cmd::Fold => {
                let text = self.text().clone();
                cx.job(t!("Finding the block"), move || text.block(), Cmd::Folding);
                Outcome::Nothing
            }
            Cmd::Folding(None) => cx.say(t!("Nothing here is indented under another line")),
            Cmd::Folding(Some(block)) => match self.change(|text| text.fold_found(&block)).unwrap_or(Err(Stale)) {
                Ok(count) => cx.say(t!("Folded {count} lines")),
                Err(_) => cx.say(t!("The text changed while the block was being found")),
            },
            Cmd::Unfold => {
                let count = self.change(Doc::unfold).unwrap_or(0);
                cx.say(t!("{count} lines show again"))
            }
            Cmd::UnfoldAll => {
                let count = self.change(Doc::unfold_all).unwrap_or(0);
                cx.say(t!("Unfolded {count} folds"))
            }
            Cmd::ShowPages | Cmd::ShowText => {
                self.reading = if matches!(cmd, Cmd::ShowPages) { Reading::Pages } else { Reading::Text };
                Outcome::Nothing
            }
            Cmd::UseA4 | Cmd::UseLetter => {
                self.paper = if matches!(cmd, Cmd::UseA4) { Paper::A4 } else { Paper::Letter };
                Outcome::Changed
            }
            Cmd::SavePdf => {
                let name = self.buffers.find(self.shown()).map_or("Untitled", |b| b.name.as_str());
                let stem = std::path::Path::new(name).file_stem().map_or_else(|| name.to_string(), |s| s.to_string_lossy().into_owned());
                cx.save_as(&format!("{stem}.pdf"), pdf, Cmd::SavedPdf);
                Outcome::Nothing
            }
            Cmd::SavedPdf(Ok(path)) => cx.say(t!("Saved the pages to {path}")),
            Cmd::SavedPdf(Err(failure)) => cx.say_failure(failure),
            Cmd::Edit(edit) => edit.apply(self),
        }
    }

    fn view(&self, ui: &mut Ui<Cmd>) {
        ui.page(t!("Galley"), |ui| {
            ui.row(|ui| {
                ui.file_field(t!("File"), Galley::file);
                ui.button(Cmd::Open);
                ui.button(Cmd::Save);
            });
            let shown = self.shown();
            if self.reading == Reading::Pages {
                return self.typeset_view(ui);
            }
            match self.beside.and_then(|k| self.buffers.find(k)) {
                Some(other) => ui.split(|ui| {
                    ui.pane(t!("This buffer"), Fill, move |ui| self.buffer(ui, shown, true));
                    ui.pane(t!("Beside it"), Fill, move |ui| self.buffer(ui, other.id, false));
                }),
                None => self.buffer(ui, shown, true),
            }
            ui.row(|ui| {
                ui.field(Galley::find, t!("Find"));
                ui.field(Galley::with, t!("Replace with"));
                ui.button(Cmd::FindPrevious);
                ui.button(Cmd::FindNext);
                ui.button(Cmd::SelectMatches);
                ui.button(Cmd::ReplaceAll);
            });
        });
    }

    fn migrations() -> &'static [Migration] {
        &[into_buffers]
    }

    fn states() -> States<Self> {
        States::new()
            .add("Empty", Galley::default())
            .add("A letter", Galley::with_text(LETTER))
            .add("Every match selected", {
                let mut galley = Galley::with_text(LETTER);
                let _ = galley.change(|text| text.select_matches("the"));
                galley
            })
            .add("Finding a word", Galley { find: Text::from("the"), ..Galley::with_text(LETTER) })
            .add("A folded block", {
                let mut galley = Galley { file: Text::from("main.rs"), ..Galley::with_text(CODE) };
                galley.change(|text| {
                    let _ = text.fold(4..8);
                    let at = text.line(3).start;
                    text.place(at, false);
                });
                galley
            })
            .add("Two buffers side by side", {
                let mut galley = Galley { file: Text::from("main.rs"), ..Galley::with_text(CODE) };
                let _ = galley.buffers.push(Buffer { id: Key::fixed(2), name: "Letter".into(), text: Doc::new(LETTER) });
                galley.beside = Some(Key::fixed(2));
                galley
            })
            .add("The letter typeset", Galley { reading: Reading::Pages, ..Galley::with_text(LETTER) })
            .add("A long file", Galley::with_text(&(1..=100_000).map(|n| format!("Line {n}\n")).collect::<String>()))
    }
}

impl Default for Galley {
    fn default() -> Self {
        Galley::with_text("")
    }
}

impl Galley {
    fn with_text(text: &str) -> Galley {
        let mut buffers = List::new();
        let _ = buffers.push(Buffer { id: Key::fixed(1), name: "Untitled".into(), text: Doc::new(text) });
        Galley {
            buffers,
            showing: Key::fixed(1),
            beside: None,
            file: Text::default(),
            find: Text::default(),
            with: Text::default(),
            font: Text::from("IBM Plex Sans"),
            paper: Paper::A4,
            reading: Reading::Text,
            camera: Camera { x: -f64::from(GAP), y: -f64::from(GAP), zoom: 0.75 },
            marks: Derived::default(),
            pages: Derived::default(),
            sheets: Derived::default(),
        }
    }

    fn shown(&self) -> Key<Buffer> {
        match self.buffers.find(self.showing).or(self.buffers.first()) {
            Some(buffer) => buffer.id,
            None => self.showing,
        }
    }

    fn text(&self) -> &Doc {
        static EMPTY: std::sync::OnceLock<Doc> = std::sync::OnceLock::new();
        self.buffers.find(self.shown()).map_or_else(|| EMPTY.get_or_init(Doc::default), |b| &b.text)
    }

    fn change<R>(&mut self, f: impl FnOnce(&mut Doc) -> R) -> Option<R> {
        let mut out = None;
        let _ = self.buffers.change(self.shown(), |buffer| out = Some(f(&mut buffer.text)));
        out
    }

    fn open(&mut self, buffer: Buffer) -> Outcome {
        let id = buffer.id;
        let outcome = self.buffers.insert_after(Some(self.shown()), buffer);
        self.showing = id;
        outcome
    }

    fn typeset_view(&self, ui: &mut Ui<Cmd>) {
        ui.row(|ui| {
            ui.field(Galley::font, t!("Font"));
            ui.button(match self.paper {
                Paper::A4 => Cmd::UseLetter,
                Paper::Letter => Cmd::UseA4,
            });
            ui.button(Cmd::SavePdf);
            ui.button(Cmd::ShowText);
        });
        let pages = self.pages.get();
        let count = pages.and_then(|p| p.set.as_ref()).map_or(0, |s| s.pages.len());
        if let Some(font) = pages.and_then(|p| p.missing.as_deref()) {
            ui.muted(t!("There is no font called “{font}” on this computer"));
        }
        ui.canvas(t!("The pages typeset, {count} of them"), 0, |canvas| {
            canvas.camera(Galley::camera);
            for drawing in self.sheets.get().into_iter().flat_map(|s| &s.pages) {
                canvas.drawing(drawing);
            }
        });
    }

    fn buffer(&self, ui: &mut Ui<Cmd>, key: Key<Buffer>, main: bool) {
        let Some(buffer) = self.buffers.find(key) else { return };
        let at = self.buffers.position(key).unwrap_or(0) + 1;
        let count = self.buffers.len();
        let name = buffer.name.as_str();
        ui.muted(t!("{name}, buffer {at} of {count}"));
        let path = Galley::buffers.at(key).then(Buffer::text);
        let editor = if main { ui.editor_with_find(buffer.name.as_str(), path, Galley::find) } else { ui.editor(buffer.name.as_str(), path) };
        editor.marked(self.marks.get().filter(|_| main));
    }
}

fn pdf(facts: &Galley) -> Vec<u8> {
    let name = facts.buffers.find(facts.shown()).map_or("Untitled", |b| b.name.as_str());
    typeset(&facts.text().to_string(), facts.font.as_str(), facts.paper).map(|set| set.pdf(name)).unwrap_or_default()
}

fn into_buffers(facts: Value) -> Value {
    let Value::Map(mut fields) = facts else { return facts };
    let text = fields.iter().position(|(name, _)| name == "text").map_or(Value::Str(String::new()), |at| fields.remove(at).1);
    let buffer = Value::Map(vec![("id".into(), Value::Key(Key::<Buffer>::fixed(1).raw())), ("name".into(), Value::Str("Untitled".into())), ("text".into(), text)]);
    fields.insert(0, ("buffers".into(), Value::List(vec![buffer])));
    Value::Map(fields)
}

const LETTER: &str = "Dear reader,\n\nThe text you are reading sits in a rope, so typing in the middle of a long file costs the same as typing in a short one.\nEvery change is an operation the ledger keeps, and undo takes it back with the cursors where they were.\n\n    Indented lines keep their indent when Return starts the next one.\n\nWith thanks,\nthe galley\n";

const KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "else", "enum", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static",
    "struct", "trait", "true", "type", "use", "where", "while",
];

fn marked(buffers: &List<Buffer>, showing: &Key<Buffer>, file: &Text) -> Marks {
    let mut marks = Marks::new();
    let Some(text) = buffers.find(*showing).or(buffers.first()).map(|b| &b.text) else { return marks };
    if !file.as_str().ends_with(".rs") {
        return marks;
    }
    let chars: Vec<char> = text.to_string().chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        let start = i;
        let c = chars[i];
        let mark = if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            Some(Mark::Comment)
        } else if c == '"' {
            i += 1;
            while i < len && chars[i] != '"' {
                i += if chars[i] == '\\' { 2 } else { 1 };
            }
            i = (i + 1).min(len);
            Some(Mark::Literal)
        } else if c.is_ascii_digit() {
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.') {
                i += 1;
            }
            Some(Mark::Literal)
        } else if c.is_alphabetic() || c == '_' {
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            if KEYWORDS.contains(&word.as_str()) {
                Some(Mark::Keyword)
            } else if c.is_uppercase() {
                Some(Mark::Type)
            } else {
                None
            }
        } else {
            i += 1;
            None
        };
        if let Some(mark) = mark {
            marks.add(start..i, mark);
        }
    }
    marks
}

const CODE: &str = "fn main() {\n    // Print every line that has something on it\n    let lines: Vec<String> = read(\"notes.txt\");\n    for line in lines {\n        if line.is_empty() {\n            continue;\n        }\n        print(line);\n    }\n}\n";

fn main() {
    graticule::run::<Galley>(t!("Galley"));
}
