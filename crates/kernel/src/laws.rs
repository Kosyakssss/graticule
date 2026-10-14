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
