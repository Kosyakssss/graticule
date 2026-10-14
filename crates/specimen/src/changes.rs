use std::path::{Path as FilePath, PathBuf};

use graticule_macros::{Commands, Facts};
use kernel::ink::Appearance;
use kernel::offscreen::Offscreen;
use kernel::proof;
use kernel::runner::Runner;
use kernel::shell::Face;
use kernel::{App, Available, Cx, Failure, Message, Path, Ui};
use ledger::{Key, List, Outcome, Selection};

/// Specimen's Changes place: every named state at its normal and smallest size, compared with the proof that was last approved.
#[derive(Facts, Default)]
pub struct Changes {
    #[session]
    survey: Survey,
    #[session]
    sheets: List<Sheet>,
    #[session]
    chosen: Selection<Sheet>,
}

#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
pub enum Survey {
    #[default]
    Looking,
    Done,
}

/// One named state at one size: what it looks like now, as a proof, and how that differs from the approved one.
#[derive(Facts)]
pub struct Sheet {
    #[key]
    id: Key<Sheet>,
    pub name: String,
    pub file: String,
    pub now: String,
    pub said: String,
    pub verdict: Verdict,
}

#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
pub enum Verdict {
    #[default]
    Same,
    New,
    Changed,
    Approved,
}

#[derive(Commands)]
pub enum Cmd {
    #[label("Approve “{name}”", short: "Approve", many: "Approve {count} changes")]
    Approve(#[subject] Key<Sheet>),
    #[label("Approve every change")]
    ApproveAll,
    #[internal]
    Saved(Result<Path, Failure>),
}

impl App for Changes {
    type Cmd = Cmd;

    fn available(&self, cmd: &Cmd) -> Available {
        match cmd {
            Cmd::Approve(sheet) => match self.sheets.find(*sheet).map(|s| s.verdict) {
                None => Available::No(Message::from("That sheet is gone")),
                Some(Verdict::Same) => Available::No(Message::from("It renders as approved")),
                Some(Verdict::Approved) => Available::No(Message::from("It is approved")),
                Some(_) => Available::Yes,
            },
            Cmd::ApproveAll => match self.survey {
                Survey::Looking => Available::No(Message::from("Specimen is still looking for changes")),
                Survey::Done => kernel::no_if(!self.sheets.iter().any(|s| matches!(s.verdict, Verdict::New | Verdict::Changed)), Message::from("Nothing has changed")),
            },
            Cmd::Saved(_) => Available::Yes,
        }
    }

    fn update(&mut self, cmd: Cmd, cx: &mut Cx<Cmd>) -> Outcome {
        match cmd {
            Cmd::Approve(sheet) => {
                self.approve(sheet, cx);
                Outcome::Nothing
            }
            Cmd::ApproveAll => {
                let waiting: Vec<Key<Sheet>> = self.sheets.iter().filter(|s| matches!(s.verdict, Verdict::New | Verdict::Changed)).map(|s| s.id).collect();
                for sheet in waiting {
                    self.approve(sheet, cx);
                }
                Outcome::Nothing
            }
            Cmd::Saved(Ok(path)) => {
                let file = path.0.display().to_string();
                let Some(id) = self.sheets.iter().find(|s| s.file == file).map(|s| s.id) else { return Outcome::Nothing };
                self.sheets.change(id, |s| s.verdict = Verdict::Approved)
            }
            Cmd::Saved(Err(failure)) => cx.say_failure(failure),
        }
    }

    fn view(&self, ui: &mut Ui<Cmd>) {
        ui.page(Message::from("Changes"), |ui| match self.survey {
            Survey::Looking => ui.muted("Looking for changes: drawing every named state at its normal and smallest size…"),
            Survey::Done => {
                let waiting = self.sheets.iter().filter(|s| s.verdict != Verdict::Same).count();
                if waiting == 0 {
                    ui.muted(format!("Nothing renders differently from what was approved, in any of {} sheets.", self.sheets.len()));
                    return;
                }
                ui.muted("What renders differently from what was last approved, said in words. Approving one keeps it as the new proof.");
                ui.list(&self.sheets, Changes::chosen, |ui, sheet| {
                    if sheet.verdict == Verdict::Same {
                        return;
                    }
                    ui.columns(|ui| {
                        ui.column(kernel::Fill, |ui| {
                            ui.strong(&sheet.name);
                            match sheet.verdict {
                                Verdict::New => ui.muted("Never approved"),
                                Verdict::Approved => ui.muted("Approved"),
                                _ => ui.text(&sheet.said),
                            }
                        });
                        ui.column(10, |ui| {
                            ui.link(Cmd::Approve);
                        });
                    });
                });
            }
        });
    }
}

impl Changes {
    /// The Changes place while Specimen is still drawing.
    pub fn looking() -> Changes {
        Changes::default()
    }

    /// The Changes place with the survey's sheets, those that changed first.
    pub fn found(mut sheets: Vec<Sheet>) -> Changes {
        sheets.sort_by_key(|s| s.verdict == Verdict::Same);
        let sheets = sheets.into_iter().zip(1..).map(|(sheet, n)| Sheet { id: Key::fixed(n), ..sheet }).collect();
        Changes { survey: Survey::Done, sheets, chosen: Selection::default() }
    }

    fn approve(&self, sheet: Key<Sheet>, cx: &mut Cx<Cmd>) {
        let Some(file) = self.sheets.find(sheet).map(|s| s.file.clone()) else { return };
        cx.write(Path(PathBuf::from(file)), move |changes: &Changes| changes.sheets.find(sheet).map(|s| s.now.clone()).unwrap_or_default(), Cmd::Saved);
    }
}

/// Where an app's approved proofs are kept: `proofs/<app>/<platform>` in the workspace Specimen runs in, found by walking up to the folder with `Cargo.lock`, or in the app's data folder outside a workspace. Each platform has its own, because keys are written differently on each and the bar's layout follows.
pub fn folder() -> PathBuf {
    let app = std::env::current_exe().ok().and_then(|exe| exe.file_stem().map(|s| s.to_string_lossy().into_owned())).unwrap_or_else(|| "app".to_string());
    let workspace = std::env::current_dir().ok().and_then(|dir| dir.ancestors().find(|d| d.join("Cargo.lock").is_file()).map(FilePath::to_path_buf));
    match workspace {
        Some(root) => root.join("proofs").join(app).join(std::env::consts::OS),
        None => kernel::runner::folder().join("Proofs").join(std::env::consts::OS),
    }
}

/// Draws every named state of an app at its normal and smallest size and compares each with its approved proof in `folder`.
pub fn survey<A: App>(folder: &FilePath) -> Vec<Sheet> {
    let mut screen = Offscreen::new(1920, 1280, 2.0);
    let mut sheets = Vec::new();
    for (name, facts) in A::states().list.iter() {
        screen.resize(1920, 1280);
        let mut runner = Runner::new(facts.clone());
        screen.render_face(Appearance::Light, &mut runner);
        sheets.push(sheet(folder, &format!("{name}, default"), &runner.tree));
        let Some((cols, rows)) = runner.minimum() else { continue };
        let m = screen.metrics;
        screen.resize((cols as f32 * m.cell_w) as u32, (rows as f32 * m.cell_h) as u32);
        let mut runner = Runner::new(facts.clone());
        screen.render_face(Appearance::Light, &mut runner);
        sheets.push(sheet(folder, &format!("{name}, smallest"), &runner.tree));
    }
    sheets
}

fn sheet<C>(folder: &FilePath, name: &str, tree: &kernel::tree::Tree<C>) -> Sheet {
    let file = folder.join(format!("{name}.txt"));
    let now = proof::write(&proof::describe(tree));
    let (verdict, said) = match std::fs::read_to_string(&file) {
        Err(_) => (Verdict::New, String::new()),
        Ok(approved) => {
            let said = proof::changes(&proof::read(&approved), &proof::read(&now));
            if said.is_empty() { (Verdict::Same, String::new()) } else { (Verdict::Changed, said.join("\n")) }
        }
    };
    Sheet { id: Key::fixed(0), name: name.to_string(), file: file.display().to_string(), now, said, verdict }
}
