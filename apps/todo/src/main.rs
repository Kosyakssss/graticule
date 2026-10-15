use graticule::prelude::*;

#[derive(Facts, Default)]
struct Todo {
    items: List<Item>,
    #[session]
    draft: Text,
    #[session]
    selection: Selection<Item>,
}

#[derive(Facts)]
struct Item {
    #[key]
    id: Key<Item>,
    title: String,
    done: bool,
}

#[derive(Commands)]
enum Cmd {
    #[label("Add")]
    Add,
    #[label(done: "Mark “{title}” not done", else: "Mark “{title}” done")]
    Toggle(#[subject] Key<Item>),
    #[label("Remove “{title}”", short: "Remove", many: "Remove {count} items")]
    Remove(#[subject] Key<Item>),
    #[label("Clear finished")]
    ClearDone,
    #[label("Export…")]
    Export,
    #[internal]
    Exported(Result<Path, Failure>),
    #[internal]
    Edit(Edit),
}

impl App for Todo {
    type Cmd = Cmd;

    fn available(&self, cmd: &Cmd) -> Available {
        match cmd {
            Cmd::Add => no_if(self.draft.is_empty(), t!("Type something first")),
            Cmd::Toggle(item) | Cmd::Remove(item) => no_if(!self.items.has(*item), t!("That item is gone")),
            Cmd::ClearDone => no_if(!self.items.any(Item::done), t!("Nothing is finished yet")),
            Cmd::Export => no_if(self.items.is_empty(), t!("The list is empty")),
            Cmd::Exported(_) | Cmd::Edit(_) => Yes,
        }
    }

    fn update(&mut self, cmd: Cmd, cx: &mut Cx<Cmd>) -> Outcome {
        match cmd {
            Cmd::Add => {
                let title = self.draft.take();
                self.items.push(Item { id: cx.key(), title, done: false })
            }
            Cmd::Toggle(item) => self.items.change(item, |item| item.done = !item.done),
            Cmd::Remove(item) => self.items.remove(item),
            Cmd::ClearDone => self.items.retain(|item| !item.done),
            Cmd::Export => {
                cx.save_as("To do.md", Todo::as_markdown, Cmd::Exported);
                Outcome::Nothing
            }
            Cmd::Exported(Ok(path)) => cx.say(t!("Saved to {path}")),
            Cmd::Exported(Err(failure)) => cx.say_failure(failure),
            Cmd::Edit(edit) => edit.apply(self),
        }
    }

    fn view(&self, ui: &mut Ui<Cmd>) {
        ui.page(t!("To do"), |ui| {
            ui.row(|ui| {
                ui.field(Todo::draft, t!("Something to do")).on_submit(Cmd::Add);
                ui.button(Cmd::Add);
            });
            ui.list(&self.items, Todo::selection, |ui, item| {
                ui.check(&item.title, item.done, Cmd::Toggle);
                ui.link(Cmd::Remove);
            });
            ui.row(|ui| {
                ui.button(Cmd::ClearDone);
                ui.button(Cmd::Export);
            });
        });
    }

    fn states() -> States<Self> {
        States::new()
            .add("Empty", Todo::default())
            .add("A few things", Todo::sample(5))
            .add("Very long list", Todo::sample(10_000))
            .add("Long titles", Todo::with_titles(&["A title that goes on and on and on past the edge of any reasonable window"]))
    }

    fn translations() -> Translations {
        &[("de", include_str!("../locales/de.ftl"))]
    }
}

impl Todo {
    fn as_markdown(&self) -> String {
        self.items.iter().map(|item| format!("- [{}] {}\n", if item.done { "x" } else { " " }, item.title)).collect()
    }

    fn sample(count: usize) -> Todo {
        let titles: Vec<String> = (1..=count).map(|n| format!("Thing number {n}")).collect();
        Todo::with_titles(&titles)
    }

    fn with_titles(titles: &[impl ToString]) -> Todo {
        let items = titles.iter().zip(1..).map(|(title, n)| Item { id: Key::fixed(n), title: title.to_string(), done: n % 3 == 0 }).collect();
        Todo { items, ..Todo::default() }
    }
}

fn main() {
    graticule::run::<Todo>(t!("To do"));
}
