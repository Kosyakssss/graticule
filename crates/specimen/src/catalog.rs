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
    component: Component,
}

#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
pub enum Component {
    #[default]
    Text,
    Button,
    Link,
    Check,
    Field,
    List,
    Empty,
    Row,
    Columns,
}

impl Component {
    const ALL: [Component; 9] =
        [Component::Text, Component::Button, Component::Link, Component::Check, Component::Field, Component::List, Component::Empty, Component::Row, Component::Columns];

    fn name(self) -> &'static str {
        match self {
            Component::Text => "Text",
            Component::Button => "Button",
            Component::Link => "Link",
            Component::Check => "Check",
            Component::Field => "Field",
            Component::List => "List",
            Component::Empty => "Empty",
            Component::Row => "Row",
            Component::Columns => "Columns",
        }
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
    #[internal]
    Go(Place),
    #[internal]
    Edit(Edit),
}

impl Default for Catalog {
    fn default() -> Self {
        let sections = Component::ALL.iter().zip(1..).map(|(&component, n)| Section { id: Key::fixed(n), component }).collect();
        Catalog {
            place: Place::Components,
            sections,
            chosen: Selection::default(),
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
            Cmd::Go(place) if place == self.place => Outcome::Nothing,
            Cmd::Go(place) => {
                self.place = place;
                Outcome::Changed
            }
            Cmd::Edit(edit) => edit.apply(self),
        }
    }

    fn view(&self, ui: &mut Ui<Cmd>) {
        match self.place {
            Place::Components => self.components(ui),
            Place::Screens => ui.page(Message::from("Screens"), |ui| {
                ui.muted("Specimen was opened on its own, so there are no app screens to show. Run an app with --specimen to see its named states here.");
            }),
            Place::Style => ui.page(Message::from("Style"), |ui| {
                ui.muted("The style and theme come in a later part of this item.");
            }),
            Place::Changes => ui.page(Message::from("Changes"), |ui| {
                ui.muted("Changes come in a later part of this item.");
            }),
        }
    }

    fn states() -> States<Self> {
        States::new().add("Components", Catalog::default())
    }
}

impl Catalog {
    fn components(&self, ui: &mut Ui<Cmd>) {
        ui.page(Message::from("Components"), |ui| {
            ui.muted("Every component in its default form, live: click it, type into it, use it from the keyboard.");
            ui.list(&self.sections, Catalog::chosen, |ui, section| {
                ui.columns(|ui| {
                    ui.column(12, |ui| ui.text(section.component.name()));
                    ui.column(kernel::Fill, |ui| self.example(ui, section.component));
                });
            });
        });
    }

    fn example(&self, ui: &mut Ui<Cmd, Key<Section>>, component: Component) {
        match component {
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
        }
    }
}
