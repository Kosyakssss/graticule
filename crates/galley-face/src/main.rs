use galley::{Doc, Mark, Marks, Replacement, Stale};
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
    #[derived(marked, from = [buffers, showing, file])]
    marks: Derived<Marks>,
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
            Cmd::NewBuffer | Cmd::Folding(_) | Cmd::Opened(_) | Cmd::Saved(_) | Cmd::Replaced(_) | Cmd::NextFound(_) | Cmd::Found(_) | Cmd::Edit(_) => Yes,
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
        Galley { buffers, showing: Key::fixed(1), beside: None, file: Text::default(), find: Text::default(), with: Text::default(), marks: Derived::default() }
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
