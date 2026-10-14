use graticule_macros::{Commands, Facts};
use kernel::{App, Available, Cx, Message, States, Ui};
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
    Row,
    Columns,
    Bar,
    Status,
    Message,
    Palette,
}

impl Component {
    const ALL: [Component; 14] = [
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
            (Place::Style, _) => ui.page(Message::from("Style"), |ui| {
                ui.muted("The style and theme come in a later part of this item.");
            }),
            (Place::Changes, _) => ui.page(Message::from("Changes"), |ui| {
                ui.muted("Changes come in a later part of this item.");
            }),
        }
    }

    fn states() -> States<Self> {
        Component::ALL.iter().fold(States::new().add("Components", Catalog::default()), |states, &component| {
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
