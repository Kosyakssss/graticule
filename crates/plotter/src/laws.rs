use ledger::{ErasedPath, Field, Key, Op, Selection, Transaction};
use proptest::prelude::*;

use crate::{Asset, Board, Bounds, End, Gesture, Index, Intent, Kind, Page, Point, Shape, Tool, order};

#[derive(Clone, Debug)]
enum Act {
    Add(u8, f64, f64, f64, f64),
    Arrow(f64, f64, f64, f64),
    Translate(u8, f64, f64),
    Resize(u8, f64, f64, f64, f64),
    Bind(u8, bool, u8, f64, f64),
    Delete(u8),
    Front(u8),
    Back(u8),
    Group(u8, u8),
    Ungroup(u8),
    AddPage,
    RemovePage(u8),
    Gesture(u8, f64, f64, f64, f64, bool),
}

fn coord() -> impl Strategy<Value = f64> {
    (-200i32..200).prop_map(f64::from)
}

fn act() -> impl Strategy<Value = Act> {
    prop_oneof![
        (any::<u8>(), coord(), coord(), coord(), coord()).prop_map(|(k, x, y, w, h)| Act::Add(k, x, y, w, h)),
        (coord(), coord(), coord(), coord()).prop_map(|(a, b, c, d)| Act::Arrow(a, b, c, d)),
        (any::<u8>(), coord(), coord()).prop_map(|(k, x, y)| Act::Translate(k, x, y)),
        (any::<u8>(), coord(), coord(), coord(), coord()).prop_map(|(k, x, y, w, h)| Act::Resize(k, x, y, w, h)),
        (any::<u8>(), any::<bool>(), any::<u8>(), coord(), coord()).prop_map(|(a, e, t, x, y)| Act::Bind(a, e, t, x, y)),
        any::<u8>().prop_map(Act::Delete),
        any::<u8>().prop_map(Act::Front),
        any::<u8>().prop_map(Act::Back),
        (any::<u8>(), any::<u8>()).prop_map(|(a, b)| Act::Group(a, b)),
        any::<u8>().prop_map(Act::Ungroup),
        Just(Act::AddPage),
        any::<u8>().prop_map(Act::RemovePage),
        (any::<u8>(), coord(), coord(), coord(), coord(), any::<bool>()).prop_map(|(t, a, b, c, d, s)| Act::Gesture(t, a, b, c, d, s)),
    ]
}

fn nth(board: &Board, k: u8) -> Option<Key<Shape>> {
    let n = board.shapes.len();
    (n > 0).then(|| board.shapes.get(k as usize % n).map(|s| s.id)).flatten()
}

fn run(board: &mut Board, chosen: &mut Selection<Shape>, act: &Act, next: &mut u64) {
    *next += 1;
    match *act {
        Act::Gesture(t, a, b, c, d, shift) => {
            let tool = [Tool::Select, Tool::Rect, Tool::Ellipse, Tool::Arrow, Tool::Draw, Tool::Frame][t as usize % 6];
            let (from, to) = (Point::new(a, b), Point::new(c, d));
            let gesture = Gesture { from, to, trail: vec![from, to], moved: from.distance(to) > 3.0, shift };
            let intent = Intent::of(board, &Index::of(board, &Page::FIRST), chosen, tool, &gesture, 1.0);
            let _ = board.perform(chosen, intent, || {
                *next += 1;
                Key::<Shape>::fixed(*next).raw()
            });
        }
        Act::Add(k, x, y, w, h) => {
            let kind = if k % 2 == 0 { Kind::Rect } else { Kind::Ellipse };
            let pages = board.pages_in_order();
            let page = if pages.is_empty() { Page::FIRST } else { pages[k as usize % pages.len()].id };
            let shape = Shape { page, ..Shape::new(Key::fixed(*next), kind, Bounds::new(x, y, w, h)) };
            let _ = match k % 3 {
                2 => board.add_image(shape, Asset::new(Key::fixed(*next), &[k % 4])),
                _ => board.add(shape),
            };
        }
        Act::AddPage => {
            let _ = board.add_page(Key::fixed(*next), "Next", "First");
        }
        Act::RemovePage(k) => {
            let pages: Vec<_> = board.pages_in_order().iter().map(|p| p.id).collect();
            if !pages.is_empty() {
                let _ = board.remove_page(pages[k as usize % pages.len()]);
            }
        }
        Act::Arrow(a, b, c, d) => {
            let _ = board.add(Shape::arrow(Key::fixed(*next), Point::new(a, b), Point::new(c, d)));
        }
        Act::Translate(k, dx, dy) => {
            if let Some(key) = nth(board, k) {
                let _ = board.translate(&[key], dx, dy);
            }
        }
        Act::Resize(k, x, y, w, h) => {
            if let Some(key) = nth(board, k) {
                let _ = board.resize(key, Bounds::new(x, y, w, h));
            }
        }
        Act::Bind(a, start, t, x, y) => {
            if let (Some(arrow), Some(to)) = (nth(board, a), nth(board, t)) {
                let _ = board.bind(Key::fixed(*next), arrow, if start { End::Start } else { End::End }, to, Point::new(x, y));
            }
        }
        Act::Delete(k) => {
            if let Some(key) = nth(board, k) {
                let _ = board.delete(&[key]);
            }
        }
        Act::Front(k) => {
            if let Some(key) = nth(board, k) {
                let _ = board.to_front(&[key]);
            }
        }
        Act::Back(k) => {
            if let Some(key) = nth(board, k) {
                let _ = board.to_back(&[key]);
            }
        }
        Act::Group(a, b) => {
            if let (Some(a), Some(b)) = (nth(board, a), nth(board, b)) {
                let _ = board.group(&[a, b], Key::fixed(*next));
            }
        }
        Act::Ungroup(k) => {
            if let Some(key) = nth(board, k) {
                let _ = board.ungroup(&[key]);
            }
        }
    }
}

fn bound(board: &Board) -> Result<(), String> {
    for b in board.bindings.iter() {
        let (Some(arrow), Some(to)) = (board.shapes.find(b.arrow), board.shapes.find(b.to)) else { return Err(format!("binding {:?} names a shape that is gone", b.id)) };
        let (at, want) = (arrow.end(b.end), b.anchor(to));
        if at.distance(want) > 1e-6 {
            return Err(format!("the {:?} end of {:?} is at {at:?}, its anchor at {want:?}", b.end, b.arrow));
        }
    }
    let parents = board.shapes.iter().map(|s| s.group).chain(board.groups.iter().map(|g| g.group));
    for parent in parents.flatten() {
        if !board.groups.has(parent) {
            return Err(format!("group {parent:?} is gone but still holds something"));
        }
    }
    for s in board.shapes.iter() {
        if !(board.pages.has(s.page) || (board.pages.is_empty() && s.page == Page::FIRST)) {
            return Err(format!("{:?} is on a page the board doesn't have", s.id));
        }
        if let Some(f) = s.frame.and_then(|f| board.shapes.find(f)).filter(|f| f.page != s.page) {
            return Err(format!("{:?} is in a frame on another page, {:?}", s.id, f.id));
        }
        let Some(f) = s.frame else { continue };
        if !board.shapes.find(f).is_some_and(|f| matches!(f.kind, Kind::Frame(_))) {
            return Err(format!("{:?} is in {f:?}, which is not a frame on the board", s.id));
        }
        if board.frames(Some(f)).contains(&s.id) {
            return Err(format!("{:?} is inside itself", s.id));
        }
    }
    for s in board.shapes.iter() {
        if let Kind::Image(a) = s.kind
            && !board.assets.has(a)
        {
            return Err(format!("{:?} shows {a:?}, which is gone", s.id));
        }
    }
    for a in board.assets.iter() {
        if !board.shapes.iter().any(|s| s.kind == Kind::Image(a.id)) {
            return Err(format!("{:?} is shown by no shape but stays", a.id));
        }
        if board.assets.iter().any(|b| b.id != a.id && b.data == a.data) {
            return Err(format!("{:?} is kept twice", a.id));
        }
    }
    for g in board.groups.iter() {
        let held = board.shapes.iter().filter(|s| s.group == Some(g.id)).count() + board.groups.iter().filter(|h| h.group == Some(g.id)).count();
        if held < 2 {
            return Err(format!("group {:?} holds {held}", g.id));
        }
    }
    Ok(())
}

fn picture(board: &Board) -> Vec<(u128, String, String)> {
    board.stacked().iter().map(|s| (s.id.raw(), format!("{:?} {} {} {} {} {:?} {:?} {:?} {:?}", s.kind, s.x, s.y, s.w, s.h, s.ink, s.group, s.frame, s.page), s.index.clone())).collect()
}

proptest! {
    #[test]
    fn a_key_made_between_two_sorts_between_them(places in proptest::collection::vec(any::<u16>(), 1..200)) {
        let mut keys: Vec<String> = Vec::new();
        for place in places {
            let at = place as usize % (keys.len() + 1);
            let key = order::between(at.checked_sub(1).map(|i| keys[i].as_str()), keys.get(at).map(String::as_str));
            prop_assert!(at == 0 || keys[at - 1] < key, "{:?} is not after {:?}", key, keys[at - 1]);
            prop_assert!(at == keys.len() || key < keys[at], "{:?} is not before {:?}", key, keys[at]);
            keys.insert(at, key);
        }
    }

    #[test]
    fn hit_testing_agrees_with_what_is_drawn(acts in proptest::collection::vec(act(), 0..60), probes in proptest::collection::vec((coord(), coord(), coord(), coord()), 1..20)) {
        let mut board = Board::default();
        let (mut next, mut chosen) = (0, Selection::new());
        for act in &acts {
            run(&mut board, &mut chosen, act, &mut next);
        }
        let page = board.showing(Page::FIRST);
        let index = Index::of(&board, &page);
        for (x, y, w, h) in probes {
            let p = Point::new(x, y);
            let drawn = board.on(page).into_iter().rev().find(|s| s.geometry().hit(p, 4.0)).map(|s| s.id);
            prop_assert_eq!(index.hit(&board, p, 4.0), drawn, "at {:?}", p);
            let area = Bounds::new(x, y, w, h);
            let inside: Vec<_> = board.on(page).into_iter().filter(|s| s.bounds().intersects(&area)).map(|s| s.id).collect();
            let found: Vec<_> = index.query(area).into_iter().map(|e| e.key).filter(|k| inside.contains(k)).collect();
            prop_assert_eq!(found, inside, "in {:?}", area);
        }
    }

    #[test]
    fn export_and_reopen_give_the_same_board(acts in proptest::collection::vec(act(), 0..30)) {
        let mut board = Board::default();
        board.attach(ErasedPath::ROOT);
        let mut next = 0;
        let mut chosen = Selection::new();
        for act in &acts {
            run(&mut board, &mut chosen, act, &mut next);
        }
        let reopened = Board::from_svg(&crate::svg(&board, Page::FIRST));
        prop_assert!(reopened.as_ref().is_some_and(|r| r.same(&board)), "the board didn't come back from its picture");
    }

    #[test]
    fn a_bound_arrow_stays_bound_through_any_move_resize_or_undo(acts in proptest::collection::vec(act(), 0..40)) {
        let mut board = Board::default();
        board.attach(ErasedPath::ROOT);
        let mut next = 0;
        let mut chosen = Selection::new();
        let mut steps = Vec::new();
        for act in &acts {
            let before = picture(&board);
            let tx = Transaction::begin();
            run(&mut board, &mut chosen, act, &mut next);
            steps.push((tx.end(), before));
            prop_assert_eq!(bound(&board), Ok(()), "after {:?}", act);
        }
        while let Some((ops, before)) = steps.pop() {
            for op in ops.iter().rev().filter(|op| !matches!(op, Op::Select { .. })) {
                prop_assert!(board.apply(op.path().steps(), &op.inverse()), "undoing {:?}", op);
            }
            prop_assert_eq!(picture(&board), before);
            prop_assert_eq!(bound(&board), Ok(()), "after undoing a step");
        }
    }
}
