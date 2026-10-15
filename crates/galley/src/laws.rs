use proptest::prelude::*;

use ledger::{Field, Transaction};

use crate::{Doc, Motion, Selections, Span};

#[derive(Clone, Debug)]
enum Act {
    Type(String),
    NewLine,
    DeleteBack,
    DeleteForward,
    Move(Motion, bool),
    AddCursor(usize),
    ReplaceAll(String, String),
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

fn act() -> impl Strategy<Value = Act> {
    prop_oneof![
        text().prop_map(Act::Type),
        Just(Act::NewLine),
        Just(Act::DeleteBack),
        Just(Act::DeleteForward),
        (motion(), any::<bool>()).prop_map(|(m, e)| Act::Move(m, e)),
        (0..40usize).prop_map(Act::AddCursor),
        (text(), text()).prop_map(|(a, b)| Act::ReplaceAll(a, b)),
    ]
}

fn run(doc: &mut Doc, selections: &mut Selections, act: &Act) {
    match act {
        Act::Type(t) => {
            let _ = doc.insert(selections, t);
        }
        Act::NewLine => {
            let _ = doc.insert_line(selections);
        }
        Act::DeleteBack => {
            let _ = doc.delete_back(selections);
        }
        Act::DeleteForward => {
            let _ = doc.delete_forward(selections);
        }
        Act::Move(m, extend) => selections.moved(doc, *m, *extend),
        Act::AddCursor(at) => selections.add(Span::cursor(*at), doc.len()),
        Act::ReplaceAll(needle, with) => {
            let _ = doc.replace_all(needle, with, selections);
        }
    }
}

fn valid(doc: &Doc, selections: &Selections) -> bool {
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
        let mut selections = Selections::cursor(0);
        let tx = Transaction::begin();
        for act in &acts {
            run(&mut doc, &mut selections, act);
        }
        let ops = tx.end();
        let end = doc.clone();
        for op in ops.iter().rev() {
            prop_assert!(doc.apply(&[], &op.inverse()));
        }
        prop_assert_eq!(doc.to_string(), start);
        for op in &ops {
            prop_assert!(doc.apply(&[], op));
        }
        prop_assert_eq!(doc, end);
    }

    #[test]
    fn selections_stay_valid_through_every_edit(start in text(), acts in proptest::collection::vec(act(), 0..24)) {
        let mut doc = Doc::new(&start);
        let mut selections = Selections::cursor(0);
        for act in &acts {
            run(&mut doc, &mut selections, act);
            prop_assert!(valid(&doc, &selections), "after {:?}: {:?} in {:?}", act, selections, doc);
        }
    }
}
