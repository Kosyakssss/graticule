use proptest::prelude::*;

use crate::ink::LIGHT;
use crate::metrics::Metrics;
use crate::pace::Pace;
use crate::paint::Painter;
use crate::text::Text;

#[derive(Clone, Copy, Debug)]
enum Happening {
    Changed,
    Unchanged,
    Redraw,
}

fn happening() -> impl Strategy<Value = Happening> {
    prop_oneof![Just(Happening::Changed), Just(Happening::Unchanged), Just(Happening::Redraw)]
}

proptest! {
    #[test]
    fn idle_is_zero(happenings in prop::collection::vec(happening(), 0..200)) {
        let mut pace = Pace::Idle;
        let (mut asked, mut changes, mut pending) = (0, 0, false);
        for h in happenings {
            match h {
                Happening::Changed => {
                    changes += 1;
                    if pace.want() {
                        prop_assert!(!pending);
                        asked += 1;
                        pending = true;
                    }
                }
                Happening::Unchanged => {}
                Happening::Redraw if pending => {
                    pace.drawn();
                    pending = false;
                }
                Happening::Redraw => {}
            }
        }
        prop_assert!(asked <= changes);
        if pending {
            pace.drawn();
        }
        prop_assert_eq!(pace, Pace::Idle);
    }
}

fn paint(quads: &mut Vec<crate::gpu::Quad>, text: &mut Text, m: Metrics) {
    quads.clear();
    text.begin_frame();
    let mut p = Painter::new(quads, text, m, *LIGHT);
    p.grid();
    let mono = p.m.mono();
    let sans = p.m.sans(400.0);
    p.text("Graticule ┌──┐ 格子", mono, (0.0, 0.0), None, p.theme.ink);
    p.text("A frame on the grid", sans, (0.0, 20.0), Some(80.0), p.theme.muted);
}

#[test]
fn no_allocation_per_frame() {
    let mut text = Text::new();
    let m = Metrics::measure(&mut text, 2.0, 1600, 1000);
    let mut quads = Vec::new();
    paint(&mut quads, &mut text, m);
    paint(&mut quads, &mut text, m);
    let info = allocation_counter::measure(|| paint(&mut quads, &mut text, m));
    assert_eq!(info.count_total, 0, "a steady frame allocated {} times", info.count_total);
}

mod tree {
    use std::collections::HashSet;

    use graticule_macros::Facts;
    use ledger::{Key, List, Selection};
    use proptest::prelude::*;

    use crate::components::Look;
    use crate::grid::Area;
    use crate::hit::Hits;
    use crate::ink::LIGHT;
    use crate::kept::Kept;
    use crate::layout::Layout;
    use crate::metrics::Metrics;
    use crate::paint::Painter;
    use crate::text::Text;
    use crate::tree::{Fill, Kind, NONE, Tree, cols};
    use crate::ui::{Scroll, Scrolls, Ui};

    #[derive(Facts, Default)]
    struct Board {
        items: List<Item>,
        #[session]
        chosen: Selection<Item>,
    }

    #[derive(Facts)]
    struct Item {
        #[key]
        id: Key<Item>,
        title: String,
        done: bool,
    }

    #[derive(Clone, Debug)]
    enum Shape {
        Text(String),
        Mono(String),
        Button(String),
        Link(String),
        Check(String, bool),
        Row(Vec<Shape>),
        Columns(Vec<(i32, bool, Shape)>),
        List(Vec<Shape>),
    }

    fn words() -> impl Strategy<Value = String> {
        "[a-z]{1,9}( [a-z]{1,9}){0,8}"
    }

    fn shape() -> impl Strategy<Value = Shape> {
        let leaf = prop_oneof![
            words().prop_map(Shape::Text),
            "[a-z0-9 ]{1,20}".prop_map(Shape::Mono),
            "[A-Z][a-z]{1,8}".prop_map(Shape::Button),
            "[a-z]{1,8}".prop_map(Shape::Link),
            (words(), any::<bool>()).prop_map(|(w, on)| Shape::Check(w, on)),
        ];
        leaf.prop_recursive(3, 24, 4, |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 1..4).prop_map(Shape::Row),
                prop::collection::vec((4..30i32, any::<bool>(), inner.clone()), 1..3).prop_map(Shape::Columns),
                prop::collection::vec(inner, 1..3).prop_map(Shape::List),
            ]
        })
    }

    fn place<S: 'static>(ui: &mut Ui<u32, S>, shape: &Shape, board: &Board) {
        match shape {
            Shape::Text(t) => ui.text(t),
            Shape::Mono(t) => ui.mono(t),
            Shape::Button(_) => {
                ui.button(1u32);
            }
            Shape::Link(_) => {
                ui.link(2u32);
            }
            Shape::Check(t, on) => {
                ui.check(t, *on, 3u32);
            }
            Shape::Row(children) => ui.row(|ui| children.iter().for_each(|c| place(ui, c, board))),
            Shape::Columns(columns) => ui.columns(|ui| {
                for (width, fill, c) in columns {
                    let span = if *fill { Fill.min(*width) } else { cols(*width) };
                    ui.column(span, |ui| place(ui, c, board));
                }
            }),
            Shape::List(row) => ui.list(&board.items, Board::chosen, |ui, _| row.iter().for_each(|c| place(ui, c, board))),
        }
    }

    fn board(n: u64) -> Board {
        let items = (1..=n).map(|n| Item { id: Key::fixed(n), title: format!("Item {n}"), done: n % 2 == 0 }).collect();
        Board { items, ..Board::default() }
    }

    fn flatten(s: &Shape) -> Vec<String> {
        match s {
            Shape::Button(t) | Shape::Link(t) => vec![t.clone()],
            Shape::Row(c) | Shape::List(c) => c.iter().flat_map(flatten).collect(),
            Shape::Columns(c) => c.iter().flat_map(|(_, _, s)| flatten(s)).collect(),
            _ => vec![],
        }
    }

    fn labels(tree: &mut Tree<u32>, shapes: &[Shape]) {
        let names: Vec<String> = shapes.iter().flat_map(flatten).collect();
        let mut names = names.into_iter().cycle();
        for i in 0..tree.nodes.len() {
            if matches!(tree.nodes[i].kind, Kind::Button | Kind::Link) {
                let name = names.next().unwrap_or_else(|| "Go".into());
                let r = tree.put(&name);
                tree.nodes[i].text = r;
            }
        }
        for i in 0..tree.nodes.len() {
            tree.hash_node(i);
        }
        tree.reindex();
    }

    fn build(tree: &mut Tree<u32>, shapes: &[Shape], board: &Board, scrolls: &Scrolls) {
        let root = tree.root();
        Ui::new(tree, board, scrolls, root).page("Board".into(), |ui| shapes.iter().for_each(|s| place(ui, s, board)));
        labels(tree, shapes);
    }

    fn lay(tree: &mut Tree<u32>, text: &mut Text, m: Metrics) {
        let mut layout = Layout::new(text, m, Vec::new(), Vec::new());
        layout.run(tree);
    }

    fn metrics(text: &mut Text, cols: u32, rows: u32) -> Metrics {
        let probe = Metrics::measure(text, 1.0, 100, 100);
        Metrics::measure(text, 1.0, (cols as f32 * probe.cell_w) as u32, (rows as f32 * probe.cell_h) as u32)
    }

    fn within(inner: Area, outer: Area) -> bool {
        inner.col >= outer.col && inner.row >= outer.row && inner.right() <= outer.right() && inner.bottom() <= outer.bottom()
    }

    fn drawn(quads: &[crate::gpu::Quad]) -> Vec<Vec<u8>> {
        let mut v: Vec<Vec<u8>> = quads.iter().filter(|q| q.size != [0.0; 2]).map(|q| bytemuck::bytes_of(q).to_vec()).collect();
        v.sort();
        v
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn every_box_lies_within_its_parent(shapes in prop::collection::vec(shape(), 1..5), cols in 10u32..160, rows in 6u32..60, items in 0u64..40) {
            let mut text = Text::new();
            let m = metrics(&mut text, cols, rows);
            let board = board(items);
            let mut tree = Tree::default();
            build(&mut tree, &shapes, &board, &Scrolls::new());
            lay(&mut tree, &mut text, m);
            let screen = m.screen();
            for n in &tree.nodes {
                if n.area.is_empty() {
                    continue;
                }
                prop_assert!(within(n.area, screen), "{:?} at {:?} leaves the window {:?}", n.kind, n.area, screen);
                if n.parent != NONE {
                    let parent = tree.nodes[n.parent as usize];
                    prop_assert!(within(n.area, parent.area), "{:?} at {:?} leaves its parent {:?} at {:?}", n.kind, n.area, parent.kind, parent.area);
                }
            }
        }

        #[test]
        fn an_approved_sheet_that_renders_the_same_reports_no_change(shapes in prop::collection::vec(shape(), 1..5), cols in 10u32..160, rows in 6u32..60, items in 0u64..40) {
            let mut text = Text::new();
            let m = metrics(&mut text, cols, rows);
            let board = board(items);
            let (mut approved, mut now) = (Tree::default(), Tree::default());
            for tree in [&mut approved, &mut now] {
                build(tree, &shapes, &board, &Scrolls::new());
                lay(tree, &mut text, m);
            }
            let kept = crate::proof::write(&crate::proof::describe(&approved));
            prop_assert_eq!(crate::proof::read(&kept), crate::proof::describe(&approved));
            let said = crate::proof::changes(&crate::proof::read(&kept), &crate::proof::describe(&now));
            prop_assert!(said.is_empty(), "{:?}", said);
        }

        #[test]
        fn no_two_nodes_share_an_id(shapes in prop::collection::vec(shape(), 1..5), items in 0u64..40) {
            let board = board(items);
            let mut tree = Tree::default();
            build(&mut tree, &shapes, &board, &Scrolls::new());
            let mut seen = HashSet::new();
            for n in &tree.nodes {
                prop_assert!(seen.insert(n.id), "two nodes share {:?}", n.id);
            }
        }

        #[test]
        fn the_node_hit_is_the_topmost_one_there(shapes in prop::collection::vec(shape(), 1..5), cols in 10u32..120, rows in 6u32..40, items in 0u64..20) {
            let mut text = Text::new();
            let m = metrics(&mut text, cols, rows);
            let board = board(items);
            let mut tree = Tree::default();
            build(&mut tree, &shapes, &board, &Scrolls::new());
            lay(&mut tree, &mut text, m);
            let mut hits = Hits::default();
            hits.fill(&tree, m.cols(), m.rows());
            for row in 0..m.rows() {
                for col in 0..m.cols() {
                    let top = tree.nodes.iter().rposition(|n| n.kind.acts() && n.area.contains(col, row)).map(|i| i as u32);
                    prop_assert_eq!(hits.at(col, row), top);
                }
            }
        }

        #[test]
        fn patching_the_kept_quads_paints_what_painting_afresh_does(first in prop::collection::vec(shape(), 1..4), then in prop::collection::vec(shape(), 1..4), hovered in 0usize..40, cols in 20u32..120, rows in 10u32..40) {
            let mut text = Text::new();
            let m = metrics(&mut text, cols, rows);
            let board = board(12);
            let scrolls = Scrolls::new();
            let mut quads = Vec::new();
            let mut kept = Kept::default();
            let mut tree = Tree::default();
            for shapes in [&first, &then] {
                build(&mut tree, shapes, &board, &scrolls);
                lay(&mut tree, &mut text, m);
                let mut p = Painter::new(&mut quads, &mut text, m, *LIGHT);
                kept.update(&mut p, &tree, |_| Look::default());
            }
            let hover = tree.nodes.get(hovered).map(|n| n.id);
            let look = move |id| Look { hovered: Some(id) == hover, ..Look::default() };
            let mut p = Painter::new(&mut quads, &mut text, m, *LIGHT);
            kept.update(&mut p, &tree, look);
            let mut fresh = Kept::default();
            let mut p = Painter::new(&mut quads, &mut text, m, *LIGHT);
            fresh.update(&mut p, &tree, look);
            for k in 0..2 {
                prop_assert_eq!(drawn(&kept.layers[k].quads), drawn(&fresh.layers[k].quads));
            }
        }

        #[test]
        fn a_list_keeps_its_place_when_items_come_and_go(len in 20u64..200, top in 0u64..20, changes in prop::collection::vec((any::<bool>(), 0u64..300), 0..20)) {
            let shapes = [Shape::List(vec![Shape::Text("x".into())])];
            let mut board = board(len);
            let mut tree = Tree::default();
            let anchor = Key::<Item>::fixed(top + 1);
            let mut scrolls = Scrolls::new();
            build(&mut tree, &shapes, &board, &scrolls);
            let id = tree.nodes.iter().find(|n| matches!(n.kind, Kind::List { .. })).map(|n| n.id).expect("a list");
            scrolls.insert(id, Scroll { anchor: Some(anchor.raw()), index: top as u32, rows: 5 });
            let mut next = len + 1;
            for (insert, at) in changes {
                let nth = board.items.keys().nth(at as usize % board.items.len().max(1));
                if insert {
                    let _ = board.items.insert_after(nth, Item { id: Key::fixed(next), title: String::new(), done: false });
                    next += 1;
                } else if let Some(key) = nth
                    && key != anchor
                {
                    let _ = board.items.remove(key);
                }
            }
            build(&mut tree, &shapes, &board, &scrolls);
            let first = tree.nodes.iter().find_map(|n| match n.kind {
                Kind::ListRow { key, .. } => Some(key),
                _ => None,
            });
            prop_assert_eq!(first, Some(anchor.raw()));
        }
    }
}

mod commands {
    use graticule_macros::{Commands, Facts};
    use ledger::{Edit, Key, List, Outcome, Selection, Text};
    use proptest::prelude::*;

    use crate::app::{App, Cx};
    use crate::command::{Available, no_if};
    use crate::field::{self, Action, Caret};
    use crate::keys::{Chord, Key as Pressed};
    use crate::message::Message;
    use crate::runner::{Runner, Slice};
    use crate::ui::Ui;

    #[derive(Facts, Default)]
    struct Shelf {
        items: List<Item>,
        #[session]
        draft: Text,
        #[session]
        chosen: Selection<Item>,
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
        #[label("Finish “{title}”")]
        Finish(#[subject] Key<Item>),
        #[label("Remove “{title}”", many: "Remove {count} items")]
        Remove(#[subject] Key<Item>),
        #[label("Clear finished")]
        ClearDone,
        #[internal]
        Edit(Edit),
        #[internal]
        Explode,
    }

    impl App for Shelf {
        type Cmd = Cmd;

        fn available(&self, cmd: &Cmd) -> Available {
            let done = |item: &Key<Item>| self.items.find(*item).is_some_and(|i| i.done);
            match cmd {
                Cmd::Add => no_if(self.draft.is_empty(), Message::from("Type something first")),
                Cmd::Finish(item) | Cmd::Remove(item) if !self.items.has(*item) => Available::No(Message::from("That item is gone")),
                Cmd::Finish(item) => no_if(done(item), Message::from("It is finished already")),
                Cmd::Remove(item) => no_if(done(item), Message::from("Finished items stay")),
                Cmd::ClearDone => no_if(!self.items.any(Item::done), Message::from("Nothing is finished")),
                Cmd::Edit(_) | Cmd::Explode => Available::Yes,
            }
        }

        fn update(&mut self, cmd: Cmd, cx: &mut Cx<Cmd>) -> Outcome {
            assert!(self.available(&cmd).is_yes(), "ran a command that available refused");
            match cmd {
                Cmd::Add => {
                    let title = self.draft.take();
                    self.items.push(Item { id: cx.key(), title, done: false })
                }
                Cmd::Finish(item) => self.items.change(item, |item| item.done = true),
                Cmd::Remove(item) => self.items.remove(item),
                Cmd::ClearDone => self.items.retain(|item| !item.done),
                Cmd::Edit(edit) => edit.apply(self),
                Cmd::Explode => {
                    let _ = self.items.push(Item { id: cx.key(), title: "half done".into(), done: false });
                    panic!("it broke halfway")
                }
            }
        }

        fn view(&self, ui: &mut Ui<Cmd>) {
            ui.page(Message::from("Shelf"), |ui| {
                ui.row(|ui| {
                    ui.field(Shelf::draft, Message::from("Something")).on_submit(Cmd::Add);
                    ui.button(Cmd::Add);
                });
                ui.list(&self.items, Shelf::chosen, |ui, item| {
                    ui.check(&item.title, item.done, Cmd::Finish);
                    ui.link(Cmd::Remove);
                });
            });
        }
    }

    fn shelf(done: &[bool]) -> Shelf {
        let items = done.iter().zip(1..).map(|(&done, n)| Item { id: Key::fixed(n), title: format!("item {n}"), done }).collect();
        Shelf { items, ..Shelf::default() }
    }

    #[derive(Clone, Debug)]
    enum Step {
        Press(&'static str),
        Type(char),
        Run(&'static str),
        Slice(usize),
    }

    const KEYS: &[&str] =
        &["down", "up", "shift+down", "shift+up", "home", "end", "space", "enter", "tab", "shift+tab", "escape", "delete", "backspace", "left", "right", "primary+a", "primary+shift+p"];
    const NAMES: &[&str] = &["add", "finish", "remove", "palette", "up", "down", "extend-down", "select-all", "cancel", "next-control"];

    fn step() -> impl Strategy<Value = Step> {
        prop_oneof![prop::sample::select(KEYS).prop_map(Step::Press), prop::char::range('a', 'e').prop_map(Step::Type), Just(Step::Type(' ')), prop::sample::select(NAMES).prop_map(Step::Run)]
    }

    fn take(runner: &mut Runner<Shelf>, step: &Step) {
        match step {
            Step::Press(keys) => {
                runner.press(Chord::parse(keys).expect("a chord"), None);
            }
            Step::Type(c) => {
                runner.press(Chord { key: Pressed::Char(*c), mods: Default::default() }, Some(&c.to_string()));
            }
            Step::Run(name) => {
                runner.run_named(name);
            }
            Step::Slice(most) => {
                runner.slice(Slice::Items(*most));
            }
        }
    }

    const FIELD_KEYS: &[&str] = &[
        "backspace",
        "delete",
        "left",
        "right",
        "shift+left",
        "shift+right",
        "home",
        "end",
        "shift+home",
        "shift+end",
        "primary+left",
        "primary+right",
        "alt+left",
        "alt+right",
        "primary+backspace",
        "alt+backspace",
        "primary+a",
        "up",
        "down",
    ];

    fn field_step() -> impl Strategy<Value = Step> {
        prop_oneof![prop::sample::select(FIELD_KEYS).prop_map(Step::Press), prop::char::range('a', 'e').prop_map(Step::Type), Just(Step::Type(' ')), Just(Step::Type('\u{e9}'))]
    }

    proptest! {
        #[test]
        fn a_command_is_never_run_when_available_says_no(done in prop::collection::vec(any::<bool>(), 0..12), steps in prop::collection::vec(step(), 0..60)) {
            let mut runner = Runner::new(shelf(&done));
            for step in &steps {
                take(&mut runner, step);
                prop_assert_eq!(runner.failure(), None);
            }
        }

        #[test]
        fn a_bulk_command_is_one_transaction(done in prop::collection::vec(any::<bool>(), 2..30), down in 0usize..30, more in 1usize..30) {
            let mut runner = Runner::new(shelf(&done));
            for _ in 0..=down {
                runner.run_named("down");
            }
            for _ in 0..more {
                runner.run_named("extend-down");
            }
            let chosen: Vec<Key<Item>> = runner.facts.chosen.iter().collect();
            let stays = |k: &Key<Item>| !chosen.contains(k) || runner.facts.items.find(*k).is_some_and(|i| i.done);
            let left: Vec<Key<Item>> = runner.facts.items.keys().filter(stays).collect();
            let removed = runner.facts.items.len() - left.len();
            let before = runner.history.len();
            runner.run_named("remove");
            prop_assert_eq!(runner.history.len() - before, usize::from(removed > 0));
            prop_assert_eq!(runner.facts.items.keys().collect::<Vec<_>>(), left);
        }

        #[test]
        fn field_edits_through_update_give_the_same_text_as_the_field_model(steps in prop::collection::vec(field_step(), 0..60)) {
            let mut runner = Runner::new(shelf(&[]));
            runner.run_named("next-control");
            let (mut text, mut caret, mut kill) = (String::new(), Caret::default(), String::new());
            for step in &steps {
                let action = match step {
                    Step::Type(c) => field::insert(caret, &c.to_string()),
                    Step::Press(keys) => {
                        let chord = Chord::parse(keys).expect("a chord");
                        if !field::handles(chord.key, chord.mods) {
                            continue;
                        }
                        field::key(&text, caret, chord.key, chord.mods, &mut kill)
                    }
                    Step::Run(_) | Step::Slice(_) => continue,
                };
                match action {
                    Action::Splice { range, text: inserted, caret: at, .. } => {
                        text.replace_range(range, &inserted);
                        caret = Caret::at(at);
                    }
                    Action::Moved(at) => caret = at,
                    Action::SelectAll => caret = Caret { caret: text.len(), anchor: 0 },
                    _ => {}
                }
                take(&mut runner, step);
                prop_assert_eq!(runner.facts.draft.as_str(), text.as_str());
            }
        }
    }

    proptest! {
        #[test]
        fn undo_returns_to_where_you_were_and_redo_to_where_you_left(done in prop::collection::vec(any::<bool>(), 0..8), steps in prop::collection::vec(step(), 0..40)) {
            let mut runner = Runner::new(shelf(&done));
            let mut befores = Vec::new();
            for step in &steps {
                let (value, len) = (ledger::Field::value(&runner.facts), runner.history.len());
                take(&mut runner, step);
                if runner.history.len() > len {
                    befores.push(value);
                }
            }
            let left = ledger::Field::value(&runner.facts);
            for before in befores.iter().rev() {
                prop_assert!(runner.run_named("undo"));
                prop_assert_eq!(&ledger::Field::value(&runner.facts), before);
            }
            for after in befores.iter().skip(1).chain([&left]) {
                prop_assert!(runner.run_named("redo"));
                prop_assert_eq!(&ledger::Field::value(&runner.facts), after);
            }
        }
    }
    fn scratch_folder() -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::env::temp_dir().join(format!("graticule laws {} {n}", std::process::id()))
    }

    proptest! {
        #[test]
        fn replaying_the_command_log_gives_the_same_facts(done in prop_oneof![prop::collection::vec(any::<bool>(), 0..8), prop::collection::vec(any::<bool>(), 250..700)], steps in prop::collection::vec(bulk_step(), 0..60)) {
            let mut runner = Runner::new(shelf(&done));
            runner.remember();
            for step in &steps {
                take(&mut runner, step);
                if runner.history.undo_label().is_some() && undoes_after(step) {
                    runner.run_named("undo");
                }
            }
            let mut again = Runner::new(shelf(&done));
            prop_assert!(again.replay(runner.entries()));
            prop_assert_eq!(ledger::Field::value(&again.facts), ledger::Field::value(&runner.facts));
        }

        #[test]
        fn a_transaction_rolled_back_after_a_panic_leaves_the_facts_as_they_were(done in prop::collection::vec(any::<bool>(), 0..8), steps in prop::collection::vec(step(), 0..30)) {
            let mut runner = Runner::new(shelf(&done));
            for step in &steps {
                take(&mut runner, step);
            }
            let (before, len) = (ledger::Field::value(&runner.facts), runner.history.len());
            runner.run(Cmd::Explode);
            prop_assert!(runner.failure().is_some_and(|f| f.contains("nothing changed")));
            prop_assert_eq!(ledger::Field::value(&runner.facts), before);
            prop_assert_eq!(runner.history.len(), len);
        }

        #[test]
        fn a_selection_never_holds_a_key_that_no_longer_exists(done in prop::collection::vec(any::<bool>(), 0..12), steps in prop::collection::vec(step(), 0..60)) {
            let mut runner = Runner::new(shelf(&done));
            for step in &steps {
                take(&mut runner, step);
                prop_assert!(runner.facts.chosen.iter().all(|k| runner.facts.items.has(k)));
            }
        }

        #[test]
        fn a_lists_tallies_always_equal_a_count_over_its_items(done in prop::collection::vec(any::<bool>(), 0..12), steps in prop::collection::vec(step(), 0..60)) {
            let mut runner = Runner::new(shelf(&done));
            for step in &steps {
                take(&mut runner, step);
                prop_assert_eq!(runner.facts.items.count(Item::done), runner.facts.items.iter().filter(|i| i.done).count());
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn saved_facts_reopen_as_they_were(done in prop::collection::vec(any::<bool>(), 0..8), first in prop::collection::vec(step(), 0..40), then in prop::collection::vec(step(), 0..40)) {
            let folder = scratch_folder();
            let fresh = || shelf(&done);
            let mut runner = Runner::open(&folder, fresh);
            for step in &first {
                take(&mut runner, step);
            }
            let kept = ledger::Field::value(&runner.facts);
            drop(runner);
            let mut runner = Runner::open(&folder, fresh);
            prop_assert_eq!(runner.failure(), None);
            prop_assert_eq!(&ledger::Field::value(&runner.facts), &kept);
            for step in &then {
                take(&mut runner, step);
            }
            let kept = ledger::Field::value(&runner.facts);
            drop(runner);
            let runner = Runner::open(&folder, fresh);
            let reopened = ledger::Field::value(&runner.facts);
            drop(runner);
            let _ = std::fs::remove_dir_all(&folder);
            prop_assert_eq!(reopened, kept);
        }

        #[test]
        fn a_reload_keeps_the_state(done in prop::collection::vec(any::<bool>(), 0..8), steps in prop::collection::vec(step(), 0..40)) {
            use crate::shell::Face;
            let mut runner = Runner::new(shelf(&done));
            for step in &steps {
                take(&mut runner, step);
            }
            let mut reloaded = Runner::new(Shelf::default());
            reloaded.restore(&runner.keep());
            prop_assert_eq!(ledger::Field::value(&reloaded.facts), ledger::Field::value(&runner.facts));
            prop_assert_eq!(reloaded.whereabouts(), runner.whereabouts());
        }
    }


    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn a_bulk_change_applied_in_slices_gives_the_same_facts_as_one_applied_at_once(done in prop::collection::vec(any::<bool>(), 0..1500), slices in prop::collection::vec(1usize..400, 1..40), stop in any::<bool>()) {
            let mut runner = Runner::new(shelf(&done));
            let before = ledger::Field::value(&runner.facts);
            let mut at_once = shelf(&done);
            let _ = at_once.items.retain(|item| !item.done);
            runner.run(Cmd::ClearDone);
            for most in &slices {
                runner.slice(Slice::Items(*most));
            }
            if stop {
                runner.run_named("undo");
                prop_assert_eq!(ledger::Field::value(&runner.facts), before);
            } else {
                runner.finish();
                prop_assert_eq!(runner.facts.items.keys().collect::<Vec<_>>(), at_once.items.keys().collect::<Vec<_>>());
                runner.run_named("undo");
                prop_assert_eq!(ledger::Field::value(&runner.facts), before);
            }
        }
    }

    fn bulk_step() -> impl Strategy<Value = Step> {
        prop_oneof![4 => step(), 1 => Just(Step::Run("clear-done")), 1 => (1usize..300).prop_map(Step::Slice)]
    }

    fn undoes_after(step: &Step) -> bool {
        matches!(step, Step::Type('a' | 'c') | Step::Press("delete" | "space"))
    }
}
