use proptest::prelude::*;

use ledger::{ErasedPath, Field, Transaction, Travel, Typed};

use crate::{Doc, Motion};

#[derive(Clone, Debug)]
enum Act {
    Typed(Typed),
    Go(Motion, bool),
    AddCursor(usize),
    ReplaceAll(String, String),
    SelectMatches(String),
}

fn text() -> impl Strategy<Value = String> {
    proptest::collection::vec(prop_oneof![Just("a"), Just("b"), Just(" "), Just("\n"), Just("é"), Just("e\u{301}"), Just("👍🏽"), Just("\t"), Just(".")], 0..12).prop_map(|parts| parts.concat())
}

fn motion() -> impl Strategy<Value = Motion> {
    prop_oneof![
        Just(Motion::Left),
        Just(Motion::Right),
        Just(Motion::WordNext),
        Just(Motion::WordPrevious),
        Just(Motion::WordEnd),
        Just(Motion::LineStart),
        Just(Motion::LineIndent),
        Just(Motion::LineEnd),
        Just(Motion::Up),
        Just(Motion::Down),
        Just(Motion::ParagraphNext),
        Just(Motion::ParagraphPrevious),
        Just(Motion::Start),
        Just(Motion::End),
    ]
}

fn travel() -> impl Strategy<Value = Travel> {
    prop_oneof![
        Just(Travel::Left),
        Just(Travel::Right),
        Just(Travel::WordLeft),
        Just(Travel::WordRight),
        Just(Travel::Up),
        Just(Travel::Down),
        Just(Travel::LineStart),
        Just(Travel::LineEnd),
        (0..4u16).prop_map(Travel::PageUp),
        (0..4u16).prop_map(Travel::PageDown),
        Just(Travel::Start),
        Just(Travel::End),
    ]
}

fn typed() -> impl Strategy<Value = Typed> {
    prop_oneof![
        text().prop_map(Typed::Insert),
        Just(Typed::NewLine),
        (any::<bool>(), any::<bool>()).prop_map(|(forward, word)| Typed::Delete { forward, word }),
        (travel(), any::<bool>()).prop_map(|(to, extend)| Typed::Move { to, extend }),
        (0..40usize, any::<bool>()).prop_map(|(at, extend)| Typed::Place { at, extend }),
        Just(Typed::SelectAll),
        Just(Typed::Collapse),
    ]
}

fn act() -> impl Strategy<Value = Act> {
    prop_oneof![
        typed().prop_map(Act::Typed),
        (motion(), any::<bool>()).prop_map(|(m, e)| Act::Go(m, e)),
        (0..40usize).prop_map(Act::AddCursor),
        (text(), text()).prop_map(|(a, b)| Act::ReplaceAll(a, b)),
        text().prop_map(Act::SelectMatches),
    ]
}

fn run(doc: &mut Doc, act: &Act) {
    match act {
        Act::Typed(t) => {
            let _ = doc.typed_in(t);
        }
        Act::Go(m, extend) => doc.go(*m, *extend),
        Act::AddCursor(at) => doc.add_cursor(*at),
        Act::ReplaceAll(needle, with) => {
            let _ = doc.replace_all(needle, with);
        }
        Act::SelectMatches(needle) => {
            let _ = doc.select_matches(needle);
        }
    }
}

fn valid(doc: &Doc) -> bool {
    let selections = doc.selections();
    let spans = selections.spans();
    !spans.is_empty()
        && selections.primary_index() < spans.len()
        && spans.iter().all(|s| s.anchor <= doc.len() && s.head <= doc.len())
        && spans.windows(2).all(|w| w[0].range().end <= w[1].range().start && w[0].range() != w[1].range())
}

proptest! {
    #[test]
    fn any_sequence_of_edits_undoes_to_the_start_and_redoes_to_the_end(start in text(), acts in proptest::collection::vec(act(), 0..24)) {
        let mut doc = Doc::new(&start);
        let first = doc.selections().clone();
        let tx = Transaction::begin();
        for act in &acts {
            let before = doc.snap();
            run(&mut doc, act);
            let mut ops = Vec::new();
            doc.diff(&before, ErasedPath::ROOT, &mut ops);
            for op in ops {
                ledger::record(|| op);
            }
        }
        let ops = tx.end();
        let (end, last) = (doc.clone(), doc.selections().clone());
        for op in ops.iter().rev() {
            prop_assert!(doc.apply(&[], &op.inverse()));
        }
        prop_assert_eq!(doc.to_string(), start);
        prop_assert_eq!(doc.selections(), &first);
        for op in &ops {
            prop_assert!(doc.apply(&[], op));
        }
        prop_assert_eq!(&doc, &end);
        prop_assert_eq!(doc.selections(), &last);
    }

    #[test]
    fn selections_stay_valid_through_every_edit(start in text(), acts in proptest::collection::vec(act(), 0..24)) {
        let mut doc = Doc::new(&start);
        for act in &acts {
            run(&mut doc, act);
            prop_assert!(valid(&doc), "after {:?}: {:?} in {:?}", act, doc.selections(), doc);
        }
    }
}
