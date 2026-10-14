# The code we want to write

This is app code for a Graticule that doesn't exist yet. The kernel's job is to make exactly this compile and run. If someone learning Rust can't read a screen here top to bottom, the API is wrong, and we change this page before writing the kernel.

Every example has the same three parts: the **facts** (what the app knows), the **commands** (everything that can happen) and the **view** (how the facts look).

## A to-do list

The whole app.

```rust
use graticule::prelude::*;

#[derive(Facts, Default)]
struct Todo {
    items: Vec<Item>,
    draft: Text,
}

#[derive(Facts)]
struct Item {
    #[key]
    id: Id,
    title: String,
    done: bool,
}

#[derive(Commands)]
enum Cmd {
    #[label("Add")]
    Add,
    #[label("Done")]
    Toggle(Id),
    #[label("Remove")]
    Remove(Id),
    #[label("Clear finished")]
    ClearDone,
}

impl App for Todo {
    type Cmd = Cmd;

    fn update(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Add => {
                let title = self.draft.take();
                self.items.push(Item { id: Id::new(), title, done: false });
            }
            Cmd::Toggle(id) => {
                let item = self.items.find_mut(id);
                item.done = !item.done;
            }
            Cmd::Remove(id) => self.items.remove_key(id),
            Cmd::ClearDone => self.items.retain(|item| !item.done),
        }
    }

    fn available(&self, cmd: &Cmd) -> Available {
        match cmd {
            Cmd::Add if self.draft.is_empty() => No("Type something first"),
            Cmd::ClearDone if !self.items.iter().any(|item| item.done) => No("Nothing is finished yet"),
            _ => Yes,
        }
    }

    fn view(&self, ui: &mut Ui<Cmd>) {
        ui.page("To do", |ui| {
            ui.row(|ui| {
                ui.field(&self.draft, "Something to do").on_submit(Cmd::Add);
                ui.button(Cmd::Add);
            });
            ui.list(&self.items, |ui, item| {
                ui.check(&item.title, item.done, Cmd::Toggle(item.id));
                ui.link(Cmd::Remove(item.id));
            });
            ui.button(Cmd::ClearDone);
        });
    }
}

fn main() {
    graticule::run::<Todo>("To do");
}
```

What Graticule does without being asked:

- Typing into the field is a command, sent by the kernel, so it can be undone like anything else.
- Every command is undoable. "Clear finished" can be taken back.
- The buttons say why they're unavailable. With an empty field, "Add" is dimmed and the status line says "Type something first."
- The list keeps its scroll position by item, not by pixels, so nothing jumps when an item is removed above.
- The facts save themselves and the app reopens where it was.
- Every command is in the action bar, the help page, the palette, the menu bar and automation.
- Nothing here says where anything goes in pixels. `row`, `list` and `page` lay out in cells.

## Named states, for Specimen

One more function shows the app in Specimen in every state that matters.

```rust
impl Todo {
    fn states() -> States<Self> {
        States::new()
            .add("Empty", Todo::default())
            .add("A few things", Todo::sample(5))
            .add("Very long list", Todo::sample(10_000))
            .add("Long titles", Todo::with_titles(&["A title that goes on and on and on past the edge of any reasonable window"]))
    }
}
```

## Settings, derived from the data

A settings page is the data's description, and nothing more.

```rust
#[derive(Facts, Form)]
struct Playback {
    #[label("Volume")]
    volume: Percent,
    #[label("Speed")]
    speed: Speed,
    #[label("Loop the selection")]
    loop_selection: bool,
}

#[derive(Facts, Choice)]
enum Speed {
    Half,
    Normal,
    Double,
}

fn view(&self, ui: &mut Ui<Cmd>) {
    ui.form(&self.playback, Cmd::SetPlayback);
}
```

A slider for the volume, a choice for the speed, a switch for looping, undo for each, and a search field when the form is long.

## Galley's editor, the main screen

Two columns: the open files on the left, the current one on the right.

```rust
#[derive(Facts)]
struct Editor {
    files: Vec<File>,
    current: Option<Id>,
    filter: Text,
    find: Find,
}

#[derive(Facts)]
struct File {
    #[key]
    id: Id,
    name: String,
    body: Document,
    edited: Moment,
}

#[derive(Commands)]
enum Cmd {
    #[label("New file")]
    New,
    #[label("Open")]
    Open(Id),
    #[label("Find")]
    Find,
    #[label("Print")]
    Print,
}

fn view(&self, ui: &mut Ui<Cmd>) {
    ui.columns(|ui| {
        ui.column(28, |ui| {
            ui.picker(&self.files, &self.filter, |ui, file| {
                ui.text(&file.name);
                ui.date(file.edited);
            })
            .on_choose(|file| Cmd::Open(file.id));
        });
        ui.column(Fill, |ui| match self.current_file() {
            Some(file) => ui.document(&file.body).find(&self.find),
            None => ui.empty("No file open", Cmd::New),
        });
    });
}
```

- `picker` filters the files as you type, fuzzy, in the background, with the matched letters marked.
- `document` is Galley: huge files, multiple selections, find and replace, spelling, printing.
- `28` is a width in columns, `Fill` takes the rest.

## Ticker, the main screen

Prices arrive from the network many times a second. They are a live value: a background job writes them, the screen reads them once per frame.

```rust
#[derive(Facts)]
struct Ticker {
    watchlist: Vec<Symbol>,
    chosen: Option<Symbol>,
    #[live]
    quotes: Live<Quotes>,
}

#[derive(Commands)]
enum Cmd {
    #[label("Choose")]
    Choose(Symbol),
    #[label("Add to watchlist")]
    Watch(Symbol),
}

impl App for Ticker {
    type Cmd = Cmd;

    fn start(&self, jobs: &mut Jobs<Cmd>) {
        jobs.stream("Market data", market::connect(), self.quotes.writer());
    }

    fn view(&self, ui: &mut Ui<Cmd>) {
        let quotes = ui.read(&self.quotes);
        ui.columns(|ui| {
            ui.column(Fill, |ui| {
                ui.table(&self.watchlist, |row, symbol| {
                    let quote = &quotes[symbol];
                    row.cell("Symbol", symbol);
                    row.cell("Last", quote.last).glow_on_change();
                    row.cell("Change", quote.change).tone_by_sign();
                    row.cell("Volume", quote.volume);
                })
                .on_choose(|symbol| Cmd::Choose(*symbol));
            });
            ui.column(56, |ui| {
                if let Some(symbol) = self.chosen {
                    ui.chart(Candles::new(quotes.history(symbol))).time_axis();
                }
            });
        });
    }
}
```

- The market feed never slows the screen down, however fast it ticks.
- "Market data" shows as a running job, with its state, in the status line and in Scope.
- The candles are drawn on a canvas inside a box on the grid. The chart's axis labels sit on cells; the candles are as thin as the data needs.

## When you need the metal

The same app can drop to the GPU for one box. Lightbox shows a 16-bit scan with adjustable levels:

```rust
fn view(&self, ui: &mut Ui<Cmd>) {
    ui.columns(|ui| {
        ui.column(Fill, |ui| {
            ui.canvas("Scan", |canvas| {
                canvas.paint(&self.levels_shader, &self.scan);
                canvas.overlay(&self.measurements);
            });
        });
        ui.column(32, |ui| {
            ui.form(&self.levels, Cmd::SetLevels);
            ui.chart(Histogram::new(&self.scan));
        });
    });
}
```

The shader is Rust too. It receives the theme's inks, so the scan's false colors follow dark and light. Everything around the box stays ordinary Graticule.

## What this asks of the kernel

- Layout in cells only: `row`, `columns`, `column(width)`, `Fill`, `page`, `list`, `table`.
- Components wired to commands, not to handlers; ids from position and from `#[key]`.
- `Available` with a reason; the status line shows it.
- Derives: `Facts` (saving, undo, migrations, Scope's facts view), `Commands` (the registry), `Form` and `Choice` (settings from data).
- `Text`, `Document`, `Find`, `Moment`, `Percent`: typed values with one look each.
- `Live` and `Jobs`: background work that never touches the main thread's budget.
- `States`: named sets of facts for Specimen.

## Open

- Whether `update` and `available` can live next to each command instead of in two `match` blocks, without making the code harder to follow.
- How a list item's commands reach the item without repeating `item.id` everywhere.
- How an app composes its own component and gives it a Specimen page.
