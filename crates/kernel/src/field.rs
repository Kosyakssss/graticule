use std::ops::Range;

use crate::keys::Key;
use crate::shell::Mods;

/// Where a field's caret is and where its selection started. It belongs to the field, not to the facts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Caret {
    pub caret: usize,
    pub anchor: usize,
}

impl Caret {
    pub fn at(i: usize) -> Self {
        Self { caret: i, anchor: i }
    }

    pub fn selection(&self) -> Range<usize> {
        self.anchor.min(self.caret)..self.anchor.max(self.caret)
    }

    /// Keeps the caret on the text after it changed under the field: through a splice when there was one, clamped otherwise.
    pub fn follow(&mut self, text: &str, splice: Option<(Range<usize>, usize)>) {
        let moved = |i: usize| match &splice {
            Some((r, len)) if i >= r.end => i - r.len() + len,
            Some((r, _)) if i > r.start => r.start,
            _ => i,
        };
        self.caret = boundary(text, moved(self.caret));
        self.anchor = boundary(text, moved(self.anchor));
    }
}

/// How a change to a field's text continues the one before it, so typing a word becomes one undo step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Alone,
    Typing,
    Deleting,
}

/// What a key does to a field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Ignored,
    Moved(Caret),
    Splice { range: Range<usize>, text: String, caret: usize, kind: Kind },
    Copy(String),
    Cut { range: Range<usize>, text: String },
    Paste,
    Submit,
    SelectAll,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    Back,
    Forward,
    WordBack,
    WordForward,
    Start,
    End,
    DeleteBack,
    DeleteForward,
    DeleteWordBack,
    DeleteWordForward,
    DeleteToStart,
    Kill,
    Yank,
    Transpose,
    Enter,
    SelectAll,
    Copy,
    Cut,
    Paste,
}

fn bind(key: Key, m: Mods) -> Option<(Op, bool)> {
    let Mods { shift, alt, primary, control } = m;
    let mac = cfg!(target_os = "macos");
    let op = match (key, primary, alt, control) {
        (Key::Char('a'), true, false, false) => Op::SelectAll,
        (Key::Char('c'), true, false, false) => Op::Copy,
        (Key::Char('x'), true, false, false) => Op::Cut,
        (Key::Char('v'), true, false, false) => Op::Paste,
        (Key::Left, false, false, false) => Op::Back,
        (Key::Right, false, false, false) => Op::Forward,
        (Key::Char('b'), false, false, true) if mac => Op::Back,
        (Key::Char('f'), false, false, true) if mac => Op::Forward,
        (Key::Left, false, true, false) if mac => Op::WordBack,
        (Key::Right, false, true, false) if mac => Op::WordForward,
        (Key::Left, true, false, false) if !mac => Op::WordBack,
        (Key::Right, true, false, false) if !mac => Op::WordForward,
        (Key::Left, true, false, false) | (Key::Home, false, false, false) | (Key::Up, false, false, false) => Op::Start,
        (Key::Right, true, false, false) | (Key::End, false, false, false) | (Key::Down, false, false, false) => Op::End,
        (Key::Char('a'), false, false, true) if mac => Op::Start,
        (Key::Char('e'), false, false, true) if mac => Op::End,
        (Key::Backspace, false, false, _) => Op::DeleteBack,
        (Key::Char('h'), false, false, true) if mac => Op::DeleteBack,
        (Key::Delete, false, false, false) => Op::DeleteForward,
        (Key::Char('d'), false, false, true) if mac => Op::DeleteForward,
        (Key::Backspace, false, true, _) if mac => Op::DeleteWordBack,
        (Key::Backspace, true, false, _) if !mac => Op::DeleteWordBack,
        (Key::Delete, false, true, false) if mac => Op::DeleteWordForward,
        (Key::Delete, true, false, false) if !mac => Op::DeleteWordForward,
        (Key::Backspace, true, false, false) => Op::DeleteToStart,
        (Key::Char('k'), false, false, true) if mac => Op::Kill,
        (Key::Char('y'), false, false, true) if mac => Op::Yank,
        (Key::Char('t'), false, false, true) if mac => Op::Transpose,
        (Key::Enter, false, false, false) => Op::Enter,
        _ => return None,
    };
    let extend = shift && !matches!(op, Op::Enter | Op::SelectAll | Op::Copy | Op::Cut | Op::Paste);
    Some((op, extend))
}

/// Whether a field takes this key for itself, even when it changes nothing, so it never falls through to a command.
pub fn handles(key: Key, m: Mods) -> bool {
    bind(key, m).is_some()
}

fn word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

pub fn boundary(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

fn prev(text: &str, from: usize) -> usize {
    text[..from].char_indices().next_back().map_or(0, |(i, _)| i)
}

fn next(text: &str, from: usize) -> usize {
    text[from..].chars().next().map_or(from, |c| from + c.len_utf8())
}

fn word_start(text: &str, from: usize) -> usize {
    let before = &text[..from];
    let end = before.trim_end_matches(|c| !word(c)).len();
    before[..end].trim_end_matches(word).len()
}

fn word_end(text: &str, from: usize) -> usize {
    let after = &text[from..];
    let skipped = after.len() - after.trim_start_matches(|c| !word(c)).len();
    let rest = &after[skipped..];
    from + skipped + (rest.len() - rest.trim_start_matches(word).len())
}

/// Typing `s` over the selection.
pub fn insert(at: Caret, s: &str) -> Action {
    let range = at.selection();
    let caret = range.start + s.len();
    Action::Splice { range, text: s.to_string(), caret, kind: Kind::Typing }
}

fn delete(range: Range<usize>) -> Action {
    if range.is_empty() {
        return Action::Ignored;
    }
    let caret = range.start;
    Action::Splice { range, text: String::new(), caret, kind: Kind::Deleting }
}

/// What a key does to a one-line field showing `text`, by the platform's own text-editing conventions. `kill` is the field's kill ring, as on a Mac.
pub fn key(text: &str, at: Caret, key: Key, m: Mods, kill: &mut String) -> Action {
    let Some((op, extend)) = bind(key, m) else { return Action::Ignored };
    let at = Caret { caret: boundary(text, at.caret), anchor: boundary(text, at.anchor) };
    let selection = at.selection();
    let selecting = !selection.is_empty();
    let to = match op {
        Op::SelectAll => return Action::SelectAll,
        Op::Copy if selecting => return Action::Copy(text[selection].to_string()),
        Op::Cut if selecting => return Action::Cut { text: text[selection.clone()].to_string(), range: selection },
        Op::Copy | Op::Cut => return Action::Ignored,
        Op::Paste => return Action::Paste,
        Op::Enter => return Action::Submit,
        Op::DeleteBack | Op::DeleteForward | Op::DeleteWordBack | Op::DeleteWordForward | Op::DeleteToStart if selecting => return delete(selection),
        Op::DeleteBack => return delete(prev(text, at.caret)..at.caret),
        Op::DeleteForward => return delete(at.caret..next(text, at.caret)),
        Op::DeleteWordBack => return delete(word_start(text, at.caret)..at.caret),
        Op::DeleteWordForward => return delete(at.caret..word_end(text, at.caret)),
        Op::DeleteToStart => return delete(0..at.caret),
        Op::Kill => {
            if at.caret == text.len() {
                return Action::Ignored;
            }
            *kill = text[at.caret..].to_string();
            return delete(at.caret..text.len());
        }
        Op::Yank if !kill.is_empty() => {
            let range = selection;
            return Action::Splice { caret: range.start + kill.len(), range, text: kill.clone(), kind: Kind::Alone };
        }
        Op::Yank => return Action::Ignored,
        Op::Transpose => {
            if text.is_empty() || at.caret == 0 {
                return Action::Ignored;
            }
            let mid = if at.caret == text.len() { prev(text, at.caret) } else { at.caret };
            let before = prev(text, mid);
            let end = next(text, mid);
            if before == mid {
                return Action::Ignored;
            }
            let swapped = format!("{}{}", &text[mid..end], &text[before..mid]);
            return Action::Splice { range: before..end, text: swapped, caret: end, kind: Kind::Alone };
        }
        Op::Back if selecting && !extend => selection.start,
        Op::Forward if selecting && !extend => selection.end,
        Op::Back => prev(text, at.caret),
        Op::Forward => next(text, at.caret),
        Op::WordBack => word_start(text, at.caret),
        Op::WordForward => word_end(text, at.caret),
        Op::Start => 0,
        Op::End => text.len(),
    };
    Action::Moved(Caret { caret: to, anchor: if extend { at.anchor } else { to } })
}

/// The word around a byte, for a double click.
pub fn word_at(text: &str, at: usize) -> Caret {
    let at = boundary(text, at);
    if text[at..].chars().next().is_some_and(word) {
        let start = at - text[..at].chars().rev().take_while(|&c| word(c)).map(char::len_utf8).sum::<usize>();
        let end = at + text[at..].chars().take_while(|&c| word(c)).map(char::len_utf8).sum::<usize>();
        Caret { caret: end, anchor: start }
    } else {
        Caret { caret: next(text, at), anchor: at }
    }
}
