use galley::Doc;
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
    #[label("Replace all")]
    ReplaceAll,
    #[internal]
    Edit(Edit),
}

impl App for Galley {
    type Cmd = Cmd;

    fn available(&self, cmd: &Cmd) -> Available {
        match cmd {
            Cmd::Open | Cmd::Save => no_if(self.file.is_empty(), t!("Choose a file first")),
            Cmd::SelectMatches | Cmd::ReplaceAll => no_if(self.find.is_empty(), t!("Type what to find first")),
            Cmd::Opened(_) | Cmd::Saved(_) | Cmd::Edit(_) => Yes,
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
            Cmd::ReplaceAll => {
                let (count, outcome) = self.text.replace_all(self.find.as_str(), self.with.as_str());
                if count == 0 {
                    return cx.say(t!("Nothing matches"));
                }
                outcome
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
