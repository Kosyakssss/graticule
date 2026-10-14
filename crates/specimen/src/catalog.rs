use graticule_macros::{Commands, Facts};
use kernel::ink::{Theme, contrast, to_oklch};
use kernel::layout::{GAP, MARGIN};
use kernel::metrics::{FONT_POINTS, LINE};
use kernel::{App, Available, Cx, Ink, Message, States, Ui};
use ledger::{Edit, Key, List, Outcome, Selection, Text};

/// Specimen's places. Every one is in the bar wherever you are.
#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
pub enum Place {
    #[default]
    Components,
    Screens,
    Style,
    Changes,
}

impl Place {
    pub const ALL: [Place; 4] = [Place::Components, Place::Screens, Place::Style, Place::Changes];

    pub fn name(self) -> &'static str {
        match self {
            Place::Components => "components",
            Place::Screens => "screens",
            Place::Style => "style",
            Place::Changes => "changes",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Place::Components => "Components",
            Place::Screens => "Screens",
            Place::Style => "Style",
            Place::Changes => "Changes",
        }
    }
}

/// Specimen's own facts: where it is, and the live examples on its pages, which are as real as any app's.
#[derive(Facts)]
pub struct Catalog {
    #[session]
    pub place: Place,
    #[session]
    sections: List<Section>,
    #[session]
    chosen: Selection<Section>,
    #[session]
    page: Option<Component>,
    #[session]
    blocks: List<Block>,
    #[session]
    reading: Selection<Block>,
    #[session]
    settings: List<Setting>,
    #[session]
    looking: Selection<Setting>,
    #[session]
    samples: List<Sample>,
    #[session]
    draft: Text,
    #[session]
    ticked: bool,
    #[session]
    pressed: u32,
}

#[derive(Facts)]
pub struct Section {
    #[key]
    id: Key<Section>,
    name: String,
    component: Component,
}

/// One piece of a component's page: a line of its documentation, or a live example.
#[derive(Facts)]
pub struct Block {
    #[key]
    id: Key<Block>,
    kind: Piece,
    text: String,
}

#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
pub enum Piece {
    #[default]
    Paragraph,
    Heading,
    Bullet,
    Code,
    Live,
    Awkward,
}

/// One line of the Style place: a heading, a sentence, an ink in both appearances, or a value of the style.
#[derive(Facts)]
pub struct Setting {
    #[key]
    id: Key<Setting>,
    shown: Shown,
    name: String,
    light: String,
    dark: String,
}

#[derive(Facts, Debug, Default, PartialEq)]
pub enum Shown {
    #[default]
    Heading,
    Paragraph,
    Columns,
    Ink(Ink),
    Value,
}

/// Content that breaks careless components: nothing, too much, the other direction, other scripts, pictures and long numbers.
#[derive(Facts)]
pub struct Sample {
    #[key]
    id: Key<Sample>,
    what: String,
    text: String,
}

#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
pub enum Component {
    #[default]
    Page,
    Text,
    Button,
    Link,
    Check,
    Field,
    List,
    Empty,
    Swatch,
    Row,
    Columns,
    Bar,
    Status,
    Message,
    Palette,
}

impl Component {
    const ALL: [Component; 15] = [
        Component::Page,
        Component::Row,
        Component::Columns,
        Component::Text,
        Component::Button,
        Component::Link,
        Component::Check,
        Component::Field,
        Component::List,
        Component::Empty,
        Component::Swatch,
        Component::Bar,
        Component::Status,
        Component::Message,
        Component::Palette,
    ];

    fn name(self) -> &'static str {
        match self {
            Component::Page => "Page",
            Component::Text => "Text",
            Component::Button => "Button",
            Component::Link => "Link",
            Component::Check => "Check",
            Component::Field => "Field",
            Component::List => "List",
            Component::Empty => "Empty",
            Component::Swatch => "Swatch",
            Component::Row => "Row",
            Component::Columns => "Columns",
            Component::Bar => "Action bar",
            Component::Status => "Status line",
            Component::Message => "Message line",
            Component::Palette => "Palette",
        }
    }

    fn doc(self) -> &'static str {
        match self {
            Component::Page => include_str!("../../../docs/components/page.md"),
            Component::Text => include_str!("../../../docs/components/text.md"),
            Component::Button => include_str!("../../../docs/components/button.md"),
            Component::Link => include_str!("../../../docs/components/link.md"),
            Component::Check => include_str!("../../../docs/components/check.md"),
            Component::Field => include_str!("../../../docs/components/field.md"),
            Component::List => include_str!("../../../docs/components/list.md"),
            Component::Empty => include_str!("../../../docs/components/empty.md"),
            Component::Swatch => include_str!("../../../docs/components/swatch.md"),
            Component::Row => include_str!("../../../docs/components/row.md"),
            Component::Columns => include_str!("../../../docs/components/columns.md"),
            Component::Bar => include_str!("../../../docs/components/bar.md"),
            Component::Status => include_str!("../../../docs/components/status.md"),
            Component::Message => include_str!("../../../docs/components/message.md"),
            Component::Palette => include_str!("../../../docs/components/palette.md"),
        }
    }

    fn awkward(self) -> bool {
        matches!(self, Component::Text | Component::Button | Component::Link | Component::Check)
    }
}

#[derive(Commands)]
pub enum Cmd {
    #[label("Press")]
    Press,
    #[label("Follow the link")]
    Follow,
    #[label("Tick or untick the box")]
    Tick,
    #[label("Say it")]
    Say,
    #[label("Open the page for “{name}”", short: "Open")]
    Open(#[subject] Key<Section>),
    #[label("Back to all components")]
    Back,
    #[label("{text}")]
    Echo(#[subject] Key<Sample>),
    #[internal]
    Go(Place),
    #[internal]
    Edit(Edit),
}

const SAMPLES: [(&str, &str); 6] = [
    ("Nothing", ""),
    ("Very long", "A label that goes on and on, well past the edge of any reasonable window, to see where it wraps and what it pushes aside"),
    ("Right to left", "مرحبا بالعالم"),
    ("Chinese", "你好，世界"),
    ("Emoji", "Party 🎉🎈"),
    ("A huge number", "12345678901234567890"),
];

impl Default for Catalog {
    fn default() -> Self {
        let sections = Component::ALL.iter().zip(1..).map(|(&component, n)| Section { id: Key::fixed(n), name: component.name().to_string(), component }).collect();
        let samples = SAMPLES.iter().zip(1..).map(|((what, text), n)| Sample { id: Key::fixed(n), what: what.to_string(), text: text.to_string() }).collect();
        Catalog {
            place: Place::Components,
            sections,
            chosen: Selection::default(),
            page: None,
            blocks: List::default(),
            reading: Selection::default(),
            settings: settings(),
            looking: Selection::default(),
            samples,
            draft: Text::default(),
            ticked: false,
            pressed: 0,
        }
    }
}

impl App for Catalog {
    type Cmd = Cmd;

    fn available(&self, cmd: &Cmd) -> Available {
        match cmd {
            Cmd::Say => kernel::no_if(self.draft.is_empty(), Message::from("Type something first")),
            Cmd::Open(section) => kernel::no_if(!self.sections.has(*section), Message::from("There is no such component")),
            Cmd::Back => kernel::no_if(self.page.is_none(), Message::from("All components are showing")),
            Cmd::Echo(sample) => kernel::no_if(!self.samples.has(*sample), Message::from("There is no such sample")),
            Cmd::Press | Cmd::Follow | Cmd::Tick | Cmd::Go(_) | Cmd::Edit(_) => Available::Yes,
        }
    }

    fn update(&mut self, cmd: Cmd, cx: &mut Cx<Cmd>) -> Outcome {
        match cmd {
            Cmd::Press => {
                self.pressed += 1;
                let _ = cx.say(Message::plain(format!("The button was pressed {} times", self.pressed)));
                Outcome::Changed
            }
            Cmd::Follow => cx.say(Message::from("A link runs a command, the same as a button")),
            Cmd::Tick => {
                self.ticked = !self.ticked;
                Outcome::Changed
            }
            Cmd::Say => {
                let said = self.draft.take();
                let _ = cx.say(Message::plain(format!("You typed “{said}”")));
                Outcome::Changed
            }
            Cmd::Open(section) => {
                let Some(component) = self.sections.find(section).map(|s| s.component) else { return Outcome::Nothing };
                self.page = Some(component);
                self.blocks = blocks(component);
                Outcome::Changed
            }
            Cmd::Back => {
                self.page = None;
                self.blocks = List::default();
                Outcome::Changed
            }
            Cmd::Echo(sample) => {
                let what = self.samples.find(sample).map(|s| s.what.clone()).unwrap_or_default();
                cx.say(Message::plain(format!("The “{what}” sample ran its command")))
            }
            Cmd::Go(place) if place == self.place => Outcome::Nothing,
            Cmd::Go(place) => {
                self.place = place;
                Outcome::Changed
            }
            Cmd::Edit(edit) => edit.apply(self),
        }
    }

    fn view(&self, ui: &mut Ui<Cmd>) {
        match (self.place, self.page) {
            (Place::Components, None) => self.overview(ui),
            (Place::Components, Some(component)) => self.sheet(ui, component),
            (Place::Screens, _) => ui.page(Message::from("Screens"), |ui| {
                ui.muted("Specimen was opened on its own, so there are no app screens to show. Run an app with --specimen to see its named states here.");
            }),
            (Place::Style, _) => self.style(ui),
            (Place::Changes, _) => ui.page(Message::from("Changes"), |ui| {
                ui.muted("Changes come in a later part of this item.");
            }),
        }
    }

    fn states() -> States<Self> {
        let start = States::new().add("Components", Catalog::default()).add("Style", Catalog { place: Place::Style, ..Catalog::default() });
        Component::ALL.iter().fold(start, |states, &component| {
            states.add(component.name(), Catalog { page: Some(component), blocks: blocks(component), ..Catalog::default() })
        })
    }
}

impl Catalog {
    fn overview(&self, ui: &mut Ui<Cmd>) {
        ui.page(Message::from("Components"), |ui| {
            ui.muted("Every component in its default form, live: click it, type into it, use it from the keyboard. Open one for its page.");
            ui.list(&self.sections, Catalog::chosen, |ui, section| {
                ui.columns(|ui| {
                    ui.column(14, |ui| ui.text(&section.name));
                    ui.column(kernel::Fill, |ui| self.example(ui, section.component));
                    ui.column(6, |ui| {
                        ui.link(Cmd::Open);
                    });
                });
            });
        });
    }

    fn style(&self, ui: &mut Ui<Cmd>) {
        ui.page(Message::from("Style"), |ui| {
            ui.list(&self.settings, Catalog::looking, |ui, setting| match setting.shown {
                Shown::Heading => ui.strong(&setting.name),
                Shown::Paragraph => ui.muted(&setting.name),
                Shown::Columns => ui.columns(|ui| {
                    ui.column(SWATCH_COLUMN, |ui| ui.muted("Now"));
                    ui.column(NAME_COLUMN, |ui| ui.muted(&setting.name));
                    ui.column(kernel::Fill, |ui| ui.muted(&setting.light));
                    ui.column(kernel::Fill, |ui| ui.muted(&setting.dark));
                }),
                Shown::Ink(ink) => ui.columns(|ui| {
                    ui.column(SWATCH_COLUMN, |ui| ui.swatch(ink));
                    ui.column(NAME_COLUMN, |ui| ui.text(&setting.name));
                    ui.column(kernel::Fill, |ui| ui.mono(&setting.light));
                    ui.column(kernel::Fill, |ui| ui.mono(&setting.dark));
                }),
                Shown::Value => ui.columns(|ui| {
                    ui.column(SWATCH_COLUMN + NAME_COLUMN + GAP, |ui| ui.text(&setting.name));
                    ui.column(kernel::Fill, |ui| ui.mono(&setting.light));
                    ui.column(kernel::Fill, |ui| ui.muted(&setting.dark));
                }),
            });
        });
    }

    fn sheet(&self, ui: &mut Ui<Cmd>, component: Component) {
        ui.page(Message::plain(component.name()), |ui| {
            ui.list(&self.blocks, Catalog::reading, |ui, block| match block.kind {
                Piece::Heading => ui.strong(&block.text),
                Piece::Paragraph => ui.text(&block.text),
                Piece::Bullet => ui.text(format!("• {}", block.text)),
                Piece::Code => ui.mono(&block.text),
                Piece::Live => ui.columns(|ui| {
                    ui.column(14, |ui| ui.muted("Live"));
                    ui.column(kernel::Fill, |ui| self.example(ui, component));
                }),
                Piece::Awkward => {
                    let Some(sample) = self.samples.iter().find(|s| s.what == block.text) else { return };
                    ui.columns(|ui| {
                        ui.column(14, |ui| ui.muted(&sample.what));
                        ui.column(kernel::Fill, |ui| awkward(ui, component, sample));
                    });
                }
            });
        });
    }

    fn example<S: Copy + 'static>(&self, ui: &mut Ui<Cmd, S>, component: Component) {
        match component {
            Component::Page => ui.muted("This whole place is one"),
            Component::Text => ui.row(|ui| {
                ui.text("Plain text");
                ui.mono("mono");
                ui.muted("muted");
            }),
            Component::Button => {
                ui.button(Cmd::Press);
            }
            Component::Link => {
                ui.link(Cmd::Follow);
            }
            Component::Check => {
                ui.check("A yes-or-no fact", self.ticked, Cmd::Tick);
            }
            Component::Field => ui.row(|ui| {
                ui.field(Catalog::draft, Message::from("Type here")).on_submit(Cmd::Say);
                ui.button(Cmd::Say);
            }),
            Component::List => ui.muted("This page is one: the arrows move through it, Shift with them chooses several"),
            Component::Empty => ui.muted("What a list shows when it has nothing, with the command that fills it"),
            Component::Row => ui.row(|ui| {
                ui.text("Side");
                ui.text("by");
                ui.text("side");
            }),
            Component::Columns => ui.columns(|ui| {
                ui.column(10, |ui| ui.text("A column"));
                ui.column(kernel::Fill, |ui| ui.muted("and one that fills the rest"));
            }),
            Component::Bar => ui.muted("At the bottom of this window"),
            Component::Status => ui.muted("Just above the bar: hover over something"),
            Component::Message => ui.muted("Under the page: press the button above"),
            Component::Palette => ui.muted("Open it from the bar"),
            Component::Swatch => ui.row(|ui| {
                ui.swatch(Ink::Link);
                ui.swatch(Ink::Danger);
                ui.swatch(Ink::Ok);
                ui.swatch(Ink::Selection);
            }),
        }
    }
}

fn awkward<S: Copy + 'static>(ui: &mut Ui<Cmd, S>, component: Component, sample: &Sample) {
    match component {
        Component::Text => ui.text(&sample.text),
        Component::Button => {
            ui.button(Cmd::Echo(sample.id));
        }
        Component::Link => {
            ui.link(Cmd::Echo(sample.id));
        }
        Component::Check => {
            ui.check(&sample.text, false, Cmd::Echo(sample.id));
        }
        _ => {}
    }
}

/// A component's page: its documentation, line by line, with the live example after the code and, for components that show text, the awkward samples at the end.
fn blocks(component: Component) -> List<Block> {
    let mut pieces: Vec<(Piece, String)> = Vec::new();
    let mut code: Option<Vec<&str>> = None;
    let mut paragraph: Vec<&str> = Vec::new();
    let flush = |pieces: &mut Vec<(Piece, String)>, paragraph: &mut Vec<&str>| {
        if !paragraph.is_empty() {
            pieces.push((Piece::Paragraph, paragraph.join(" ").replace('`', "")));
            paragraph.clear();
        }
    };
    for line in component.doc().lines().skip(1) {
        if let Some(lines) = &mut code {
            if line.starts_with("```") {
                pieces.push((Piece::Code, lines.join("\n")));
                pieces.push((Piece::Live, String::new()));
                code = None;
            } else {
                lines.push(line);
            }
        } else if line.starts_with("```") {
            flush(&mut pieces, &mut paragraph);
            code = Some(Vec::new());
        } else if let Some(heading) = line.strip_prefix("## ") {
            flush(&mut pieces, &mut paragraph);
            pieces.push((Piece::Paragraph, String::new()));
            pieces.push((Piece::Heading, heading.to_string()));
        } else if let Some(bullet) = line.strip_prefix("- ") {
            flush(&mut pieces, &mut paragraph);
            pieces.push((Piece::Bullet, bullet.replace('`', "")));
        } else if line.trim().is_empty() {
            flush(&mut pieces, &mut paragraph);
        } else {
            paragraph.push(line.trim());
        }
    }
    flush(&mut pieces, &mut paragraph);
    if !pieces.iter().any(|(p, _)| *p == Piece::Live) {
        pieces.insert(pieces.len().min(1), (Piece::Live, String::new()));
    }
    if component.awkward() {
        pieces.push((Piece::Paragraph, String::new()));
        pieces.push((Piece::Heading, "With awkward content".to_string()));
        pieces.extend(SAMPLES.iter().map(|(what, _)| (Piece::Awkward, what.to_string())));
    }
    pieces.into_iter().zip(1..).map(|((kind, text), n)| Block { id: Key::fixed(n), kind, text }).collect()
}

const SWATCH_COLUMN: i32 = 6;
const NAME_COLUMN: i32 = 12;

/// The Style place's lines: every ink of the theme in both appearances with its contrast on the background, then the values the grid and the type are built from.
fn settings() -> List<Setting> {
    let light = Theme::of(kernel::ink::Appearance::Light);
    let dark = Theme::of(kernel::ink::Appearance::Dark);
    let describe = |theme: &Theme, ink: Ink| {
        let (l, c, h) = to_oklch(theme.get(ink));
        let numbers = format!("{l:.2} {c:.3} {h:.0}");
        if ink.surface() {
            return numbers;
        }
        let ratio = contrast(theme.get(ink), theme.bg);
        match ink.needs() {
            Some(needs) if ratio < needs => format!("{numbers} · {ratio:.1}, below {needs}"),
            _ => format!("{numbers} · {ratio:.1}"),
        }
    };
    let line = |shown: Shown, name: &str, light: String, dark: String| (shown, name.to_string(), light, dark);
    let mut lines = vec![
        line(Shown::Heading, "Inks", String::new(), String::new()),
        line(Shown::Paragraph, "Everything is drawn in one of these inks, so a theme restyles everything at once. Each is given as OKLCH lightness, chroma and hue, then, for inks that text and lines are drawn in, its contrast on the background: text needs 4.5 to 1, faint text 3 to 1. The swatch shows the appearance the window is in now.", String::new(), String::new()),
        line(Shown::Columns, "Ink", "Light".to_string(), "Dark".to_string()),
    ];
    lines.extend(Ink::ALL.into_iter().map(|ink| line(Shown::Ink(ink), ink.name(), describe(light, ink), describe(dark, ink))));
    lines.extend([
        line(Shown::Paragraph, "", String::new(), String::new()),
        line(Shown::Heading, "The grid and the type", String::new(), String::new()),
        line(Shown::Paragraph, "Editing these here, and the inks above, comes with style and theme files.", String::new(), String::new()),
        line(Shown::Value, "Type size", format!("{FONT_POINTS} pt"), "The monospace face at this size sets a cell's width, rounded to a whole point".to_string()),
        line(Shown::Value, "Line height", format!("{LINE}"), "A cell is this many lines of type tall, rounded to a whole point".to_string()),
        line(Shown::Value, "Text face", "IBM Plex Sans".to_string(), "For words".to_string()),
        line(Shown::Value, "Data face", "IBM Plex Mono".to_string(), "For data, code and anything that lines up in cells".to_string()),
        line(Shown::Value, "Strong weight", "600".to_string(), "Text that heads what follows it; other text is 400".to_string()),
        line(Shown::Value, "Gap", format!("{GAP} cells"), "Between things side by side".to_string()),
        line(Shown::Value, "Margin", format!("{MARGIN} cells"), "Around a page".to_string()),
    ]);
    lines.into_iter().zip(1..).map(|((shown, name, light, dark), n)| Setting { id: Key::fixed(n), shown, name, light, dark }).collect()
}
