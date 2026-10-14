use graticule_macros::{Commands, Facts};
use kernel::{App, Available, Cx, Message, States, Ui};
use ledger::{Key, List, Outcome, Selection, Text};
use proptest::prelude::*;

use crate::{Law, Plan, assay};

#[derive(Facts, Default)]
struct Tray {
    things: List<Thing>,
    #[session]
    draft: Text,
    #[session]
    chosen: Selection<Thing>,
}

#[derive(Facts)]
struct Thing {
    #[key]
    id: Key<Thing>,
    name: String,
}

#[derive(Commands)]
enum Cmd {
    #[label("Put")]
    Put,
    #[label("Take “{name}”")]
    Take(#[subject] Key<Thing>),
    #[label("Tidy")]
    Tidy,
    #[internal]
    Edit(ledger::Edit),
}

impl App for Tray {
    type Cmd = Cmd;

    fn available(&self, cmd: &Cmd) -> Available {
        match cmd {
            Cmd::Put => kernel::no_if(self.draft.is_empty(), Message::from("Type a name first")),
            Cmd::Take(thing) => kernel::no_if(!self.things.has(*thing), Message::from("It is gone")),
            Cmd::Tidy | Cmd::Edit(_) => Available::Yes,
        }
    }

    fn update(&mut self, cmd: Cmd, cx: &mut Cx<Cmd>) -> Outcome {
        match cmd {
            Cmd::Put => {
                let name = self.draft.take();
                self.things.push(Thing { id: cx.key(), name })
            }
            Cmd::Take(thing) => self.things.remove(thing),
            Cmd::Tidy => Outcome::Nothing,
            Cmd::Edit(edit) => edit.apply(self),
        }
    }

    fn view(&self, ui: &mut Ui<Cmd>) {
        ui.page(Message::from("Tray"), |ui| {
            ui.row(|ui| {
                ui.field(Tray::draft, Message::from("A name")).on_submit(Cmd::Put);
                ui.button(Cmd::Put);
                ui.button(Cmd::Tidy);
            });
            ui.list(&self.things, Tray::chosen, |ui, thing| {
                ui.text(&thing.name);
                ui.link(Cmd::Take);
            });
        });
    }

    fn states() -> States<Self> {
        let things = (1..=3).map(|n| Thing { id: Key::fixed(n), name: format!("thing {n}") }).collect();
        States::new().add("Empty", Tray::default()).add("Three things", Tray { things, ..Tray::default() })
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(6))]

    #[test]
    fn a_check_that_reads_nothing_new_finds_the_same_things(seed in any::<u64>()) {
        let plan = Plan { sequences: 4, steps: 12, seed };
        let timeless = |law: &Law| !matches!(law, Law::FrameBudget | Law::AvailableBudget);
        let first: Vec<_> = assay::<Tray>(plan).findings.into_iter().filter(|f| timeless(&f.law)).collect();
        let again: Vec<_> = assay::<Tray>(plan).findings.into_iter().filter(|f| timeless(&f.law)).collect();
        prop_assert_eq!(first, again);
    }
}
