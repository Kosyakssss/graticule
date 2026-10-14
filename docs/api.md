# The code we want to write

This is app code for a Graticule that doesn't exist yet. The kernel's job is to make exactly this compile and run. If someone learning Rust can't read a screen here top to bottom, the API is wrong, and we change this page before writing the kernel. It has been through two rounds of review; the decisions behind it are in the spec, §3.

Every example has the same parts: the **facts** (what the app knows), the **commands** (everything that can happen), `available` (what can happen right now, and why not), `update` (what each command does) and the **view** (how the facts look).

## Three kinds of text

- `String` is a plain value inside the facts, like an item's title.
- `Text` is what a field edits: a value with a caret and a selection, edited by typing.
- `Document` is Galley's: very large text with many cursors, syntax and spelling.

## A to-do list

The whole app.

```rust
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
    #[label("Remove “{title}”")]
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
            Cmd::ClearDone => no_if(self.items.none(|item| item.done), t!("Nothing is finished yet")),
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
                cx.save_as("To do.md", self.as_markdown(), Cmd::Exported);
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
            ui.list(&self.items, &self.selection, |ui, item| {
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
}

fn main() {
    graticule::run::<Todo>(t!("To do"));
}
```

Reading it:

- **`items` is a `List`, not a `Vec`.** The document part of the facts is built from Ledger's types, so the list syncs between your devices without losing anything. A `Vec` there doesn't compile.
- **`#[session]`** marks where the user is rather than what they're making: the half-typed field and the selection are kept on this device and never synced.
- **`available` names every command.** There's no catch-all, so a new command can't become available by accident. Each "no" carries the reason the status line shows.
- **`update` handles every case.** `change` and `remove` return `Nothing` if the item is gone by then. New keys come from `cx`, so replaying the history gives the same list.
- **`#[subject]`**: inside a list row, `Cmd::Toggle` and `Cmd::Remove` get the row's key without being told. The same `Remove` works from a keymap sequence, the palette and the context menu, on the item with focus, or on every selected item at once.
- **Labels speak about their subject**: a screen reader hears "Remove 'buy milk'", not ten thousand identical "Remove"s. `Toggle`'s label follows the item's state.
- **Effects are asked for, not done.** `Export` asks for a save dialog and a file write; the result comes back as `Exported`, which is `#[internal]`: never in the palette, the menus, the keys or undo. In Specimen the effect is shown instead of run.
- **Typing is a command.** The kernel sends `Edit` for the field bound to `Todo::draft`; `edit.apply(self)` is the usual handling, and this is the one place to validate a value or keep an invariant.
- **Undo is one history.** Type "milk", add it, press Cmd-Z: the item goes and "milk" is back in the field. Press it again and the typing goes.
- `t!` marks text for translation. It's written in English; the ids are generated.
- `states()` puts the app in Specimen in every state that matters.
- Nothing here says where anything goes in pixels. `page`, `row` and `list` lay out in cells.

## Settings, derived from the data

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
    ui.form(Player::playback);
}
```

A slider for the volume, a choice for the speed, a switch for looping. Each change arrives in `update` as an `Edit`; dragging the slider is one undo step, not two hundred. A long form gets a search field.

## Galley's editor, the main screen

Two columns: the open files on the left, the current one on the right.

```rust
#[derive(Facts)]
struct Editor {
    files: List<File>,
    #[session]
    current: Option<Key<File>>,
    #[session]
    filter: Text,
    #[session]
    find: Find,
}

#[derive(Facts)]
struct File {
    #[key]
    id: Key<File>,
    name: String,
    body: Document,
    edited: Moment,
}

#[derive(Commands)]
enum Cmd {
    #[label("New file")]
    New,
    #[label("Open “{name}”")]
    Open(#[subject] Key<File>),
    #[label("Find")]
    Find,
    #[label("Print…")]
    Print,
    #[internal]
    Edit(Edit),
}

fn view(&self, ui: &mut Ui<Cmd>) {
    ui.columns(|ui| {
        ui.column(28.min(20), |ui| {
            ui.picker(&self.files, Editor::filter, |ui, file| {
                ui.text(&file.name);
                ui.date(file.edited);
            })
            .on_choose(Cmd::Open);
        });
        ui.column(Fill.min(40), |ui| {
            if let Some(file) = self.current_file() {
                ui.document(&file.body).find(Editor::find);
            } else {
                ui.empty(t!("No file open"), Cmd::New);
            }
        });
    });
}
```

- `picker` filters the files as you type, fuzzy, in the background, with the matched letters marked.
- `document` is Galley: huge files, multiple selections, find and replace, spelling, printing.
- `28.min(20)` is 28 columns, never fewer than 20; `Fill.min(40)` takes the rest, never fewer than 40. When the window can't fit both minimums, the columns stack: that's the narrow layout, never a silent cut.
- `update` keeps `edited` current in its `Edit` arm, when the edit touched `body`.

## Ticker, the main screen

Prices arrive from the network many times a second. They are a live value: a background job writes them, and the view reads a snapshot once per frame. The candles are derived: a job computes them from the prices and the chosen symbol, and recomputes only when those change.

```rust
#[derive(Facts)]
struct Ticker {
    watchlist: List<Symbol>,
    #[session]
    chosen: Option<Symbol>,
    #[live]
    quotes: Live<Quotes>,
    #[derived(Candles::of, from = [quotes, chosen])]
    candles: Derived<Candles>,
}

#[derive(Commands)]
enum Cmd {
    #[label("Show {symbol}")]
    Choose(#[subject] Symbol),
    #[label("Add {symbol} to the watchlist")]
    Watch(Symbol),
}

impl App for Ticker {
    type Cmd = Cmd;

    fn start(&self, jobs: &mut Jobs<Cmd>) {
        jobs.stream(t!("Market data"), market::connect(), Ticker::quotes);
    }

    fn view(&self, ui: &mut Ui<Cmd>) {
        let quotes = self.quotes.now();
        ui.columns(|ui| {
            ui.column(Fill.min(40), |ui| {
                ui.table(&self.watchlist, |row, symbol| {
                    let quote = &quotes[symbol];
                    row.cell(t!("Symbol"), symbol);
                    row.cell(t!("Last"), quote.last).glow_on_change();
                    row.cell(t!("Change"), quote.change).tone_by_sign();
                    row.cell(t!("Volume"), quote.volume);
                })
                .on_choose(Cmd::Choose);
            });
            ui.column(56.min(32), |ui| {
                ui.chart(&self.candles).time_axis();
            });
        });
    }
}
```

- `quotes.now()` is a snapshot, not tied to `ui`, so the rest of the view can use both freely.
- The market feed never slows the screen down, however fast it ticks. "Market data" shows as a running job in the status line and in Scope.
- Until the first candles are ready, the chart says they're coming.
- The candles are drawn on a canvas inside a box on the grid. The axis labels sit on cells; the candles are as thin as the data needs.

## When you need the metal

Lightbox shows a 16-bit scan with adjustable levels. The scan is a resource: big, loaded once, never copied into history. Its histogram is derived, computed in the background when the scan changes, never while drawing.

```rust
#[derive(Facts)]
struct Lightbox {
    #[resource]
    scan: Scan,
    #[resource]
    levels_shader: Shader,
    levels: Levels,
    measurements: List<Measurement>,
    #[derived(Histogram::of, from = [scan])]
    histogram: Derived<Histogram>,
}

fn view(&self, ui: &mut Ui<Cmd>) {
    ui.columns(|ui| {
        ui.column(Fill.min(40), |ui| {
            ui.canvas(t!("Scan"), |canvas| {
                canvas.paint(&self.levels_shader, &self.scan, &self.levels);
                canvas.overlay(&self.measurements);
            });
        });
        ui.column(32, |ui| {
            ui.form(Lightbox::levels);
            ui.chart(&self.histogram);
        });
    });
}
```

The shader is Rust too. It receives the theme's inks, so the scan's false colors follow dark and light. Everything around the box stays ordinary Graticule.

## What this asks of the kernel

- Layout in cells only: `page`, `row`, `columns`, `column(width)`, `Fill`, `.min()`, `.max()`, `list`, `table`, with a stated narrow form.
- Components wired to commands, not to handlers; ids from position and from `#[key]`; subjects from rows.
- Paths into the facts (`Todo::draft`, `Player::playback`) generated by `derive(Facts)`, for fields, forms and live and derived values.
- `available` with a reason; `update` with `Cx` and `Outcome`; `#[internal]` commands; `Edit`.
- The four kinds of state: document (Ledger's types), `#[session]`, `#[live]`, `#[resource]`.
- `Derived` keyed by the versions of the paths it reads; `Live` read as a snapshot.
- Derives: `Facts`, `Commands`, `Form`, `Choice`.
- `t!` for every text the user reads, with generated ids.

## Open

- The syntax for labels that follow their subject's state (`done: …, else: …` above).
- How an app composes its own component and gives it a Specimen page.
- How a page is opened in a second window, and how places and back and forward look in app code.
