use galley::{Doc, Replacement};
use graticule::prelude::*;

#[derive(Facts, Default)]
struct Galley {
    text: Doc,
    #[session]
    file: Text,
    #[session]
    find: Text,
    #[session]
    with: Text,
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
    #[label("Select every match")]
    SelectMatches,
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
    #[internal]
    Edit(Edit),
}

impl App for Galley {
    type Cmd = Cmd;

    fn available(&self, cmd: &Cmd) -> Available {
        match cmd {
            Cmd::Open | Cmd::Save => no_if(self.file.is_empty(), t!("Choose a file first")),
            Cmd::SelectMatches | Cmd::ReplaceAll => no_if(self.find.is_empty(), t!("Type what to find first")),
            Cmd::AddCursorAbove => no_if(self.text.line_of(self.text.selections().primary().head) == 0, t!("The cursor is on the first line")),
            Cmd::AddCursorBelow => no_if(self.text.line_of(self.text.selections().primary().head) + 1 >= self.text.lines(), t!("The cursor is on the last line")),
            Cmd::AddNextMatch => no_if(self.text.selections().primary().is_empty(), t!("Select some text first")),
            Cmd::Opened(_) | Cmd::Saved(_) | Cmd::Replaced(_) | Cmd::NextFound(_) | Cmd::Edit(_) => Yes,
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
                let len = self.text.len();
                self.text.replace(0..len, &text)
            }
            Cmd::Opened(Err(failure)) | Cmd::Saved(Err(failure)) => cx.say_failure(failure),
            Cmd::Save => {
                cx.write(Path(self.file.to_string().into()), |facts: &Galley| facts.text.to_string(), Cmd::Saved);
                Outcome::Nothing
            }
            Cmd::Saved(Ok(path)) => cx.say(t!("Saved to {path}")),
            Cmd::SelectMatches => match self.text.select_matches(self.find.as_str()) {
                0 => cx.say(t!("Nothing matches")),
                count => cx.say(t!("Selected {count} matches")),
            },
            Cmd::AddCursorAbove | Cmd::AddCursorBelow => {
                let before = self.text.selections().spans().len();
                self.text.add_cursor_line(matches!(cmd, Cmd::AddCursorBelow));
                match self.text.selections().spans().len() {
                    count if count == before => cx.say(t!("There is a cursor there already")),
                    count => cx.say(t!("{count} cursors")),
                }
            }
            Cmd::AddNextMatch => {
                let text = self.text.clone();
                cx.job(t!("Finding the next match"), move || text.next_match(), Cmd::NextFound);
                Outcome::Nothing
            }
            Cmd::NextFound(None) => Outcome::Nothing,
            Cmd::NextFound(Some(next)) => match self.text.take_next(&next) {
                Ok(true) => {
                    let count = self.text.selections().spans().len();
                    cx.say(t!("{count} selections"))
                }
                Ok(false) => cx.say(t!("Every match is selected already")),
                Err(_) => cx.say(t!("The text changed while the next match was being found")),
            },
            Cmd::ReplaceAll => {
                let (text, find, with) = (self.text.clone(), self.find.to_string(), self.with.to_string());
                cx.job(t!("Replacing “{find}”"), move || text.replacement(&find, &with), Cmd::Replaced);
                Outcome::Nothing
            }
            Cmd::Replaced(replacement) => {
                let count = replacement.count();
                match self.text.take(replacement) {
                    Ok(_) if count == 0 => cx.say(t!("Nothing matches")),
                    Ok(outcome) => {
                        let _ = cx.say(t!("Replaced {count} matches"));
                        outcome
                    }
                    Err(_) => cx.say(t!("The text changed while the matches were being found, so nothing was replaced")),
                }
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
            ui.editor(t!("Text"), Galley::text);
            ui.row(|ui| {
                ui.field(Galley::find, t!("Find"));
                ui.field(Galley::with, t!("Replace with"));
                ui.button(Cmd::SelectMatches);
                ui.button(Cmd::ReplaceAll);
            });
        });
    }

    fn states() -> States<Self> {
        States::new()
            .add("Empty", Galley::default())
            .add("A letter", Galley::with_text(LETTER))
            .add("Every match selected", {
                let mut galley = Galley::with_text(LETTER);
                let _ = galley.text.select_matches("the");
                galley
            })
            .add("A long file", Galley::with_text(&(1..=100_000).map(|n| format!("Line {n}\n")).collect::<String>()))
    }
}

impl Galley {
    fn with_text(text: &str) -> Galley {
        Galley { text: Doc::new(text), ..Galley::default() }
    }
}

const LETTER: &str = "Dear reader,\n\nThe text you are reading sits in a rope, so typing in the middle of a long file costs the same as typing in a short one.\nEvery change is an operation the ledger keeps, and undo takes it back with the cursors where they were.\n\n    Indented lines keep their indent when Return starts the next one.\n\nWith thanks,\nthe galley\n";

fn main() {
    graticule::run::<Galley>(t!("Galley"));
}
