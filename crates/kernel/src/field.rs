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

/// What a key does in an editor: a change the text makes at its cursors, or one that goes through the clipboard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorKey {
    Typed(ledger::Typed),
    Copy,
    Cut,
    Paste,
}

/// The key an editor takes, by the platform's conventions, given how many rows it shows and whether it has one cursor with nothing selected.
pub fn editor_key(key: Key, m: Mods, rows: u16, single: bool) -> Option<EditorKey> {
    use ledger::{Travel, Typed};
    let Mods { shift, alt, primary, control } = m;
    let mac = cfg!(target_os = "macos");
    let to = match (key, primary, alt, control) {
        (Key::Left, false, false, false) => Travel::Left,
        (Key::Right, false, false, false) => Travel::Right,
        (Key::Up, false, false, false) => Travel::Up,
        (Key::Down, false, false, false) => Travel::Down,
        (Key::Char('b'), false, false, true) if mac => Travel::Left,
        (Key::Char('f'), false, false, true) if mac => Travel::Right,
        (Key::Char('p'), false, false, true) if mac => Travel::Up,
        (Key::Char('n'), false, false, true) if mac => Travel::Down,
        (Key::Left, false, true, false) if mac => Travel::WordLeft,
        (Key::Right, false, true, false) if mac => Travel::WordRight,
        (Key::Left, true, false, false) if !mac => Travel::WordLeft,
        (Key::Right, true, false, false) if !mac => Travel::WordRight,
        (Key::Left, true, false, false) | (Key::Home, false, false, false) => Travel::LineStart,
        (Key::Right, true, false, false) | (Key::End, false, false, false) => Travel::LineEnd,
        (Key::Char('a'), false, false, true) if mac => Travel::LineStart,
        (Key::Char('e'), false, false, true) if mac => Travel::LineEnd,
        (Key::Up, true, false, false) if mac => Travel::Start,
        (Key::Down, true, false, false) if mac => Travel::End,
        (Key::Home, true, false, false) if !mac => Travel::Start,
        (Key::End, true, false, false) if !mac => Travel::End,
        (Key::PageUp, false, false, false) => Travel::PageUp(rows.saturating_sub(1).max(1)),
        (Key::PageDown, false, false, false) => Travel::PageDown(rows.saturating_sub(1).max(1)),
        _ => {
            let typed = match (key, primary, alt, control) {
                (Key::Char('a'), true, false, false) => Typed::SelectAll,
                (Key::Char('c'), true, false, false) => return Some(EditorKey::Copy),
                (Key::Char('x'), true, false, false) => return Some(EditorKey::Cut),
                (Key::Char('v'), true, false, false) => return Some(EditorKey::Paste),
                (Key::Backspace, false, false, _) => Typed::Delete { forward: false, word: false },
                (Key::Char('h'), false, false, true) if mac => Typed::Delete { forward: false, word: false },
                (Key::Backspace, false, true, _) if mac => Typed::Delete { forward: false, word: true },
                (Key::Backspace, true, false, _) if !mac => Typed::Delete { forward: false, word: true },
                (Key::Delete, false, false, false) => Typed::Delete { forward: true, word: false },
                (Key::Char('d'), false, false, true) if mac => Typed::Delete { forward: true, word: false },
                (Key::Delete, false, true, false) if mac => Typed::Delete { forward: true, word: true },
                (Key::Delete, true, false, false) if !mac => Typed::Delete { forward: true, word: true },
                (Key::Enter, false, false, false) => Typed::NewLine,
                (Key::Escape, false, false, false) if !single => Typed::Collapse,
                _ => return None,
            };
            return Some(EditorKey::Typed(typed));
        }
    };
    Some(EditorKey::Typed(Typed::Move { to, extend: shift }))
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

/// The rows a text wraps into at `width` cells, as byte ranges: a new line ends a row, and a line too long for the width breaks after its last space that fits, or mid-word when there is none. A row that wraps keeps its space, so the rows of a line touch.
pub fn rows(text: &str, width: usize) -> Rows<'_> {
    Rows { text, width: width.max(1), at: 0, done: false }
}

/// The rows of a text, from [`rows`].
pub struct Rows<'a> {
    text: &'a str,
    width: usize,
    at: usize,
    done: bool,
}

impl Iterator for Rows<'_> {
    type Item = Range<usize>;

    fn next(&mut self) -> Option<Range<usize>> {
        if self.done {
            return None;
        }
        let at = self.at;
        let line_end = self.text[at..].find('\n').map_or(self.text.len(), |k| at + k);
        let (mut used, mut space) = (0, None);
        for (k, c) in self.text[at..line_end].char_indices() {
            let w = crate::text::cells(c);
            if used + w > self.width && k > 0 {
                let end = space.unwrap_or(at + k);
                self.at = end;
                return Some(at..end);
            }
            used += w;
            if c == ' ' {
                space = Some(at + k + 1);
            }
        }
        if line_end == self.text.len() {
            self.done = true;
        } else {
            self.at = line_end + 1;
        }
        Some(at..line_end)
    }
}

/// The row a byte sits on, and that row: the last row that starts at or before it.
pub fn row_of(text: &str, width: usize, at: usize) -> (usize, Range<usize>) {
    let mut found = (0, 0..0);
    for (k, row) in rows(text, width).enumerate() {
        if row.start > at {
            break;
        }
        found = (k, row);
    }
    found
}

/// Where a row really ends for the caret: before the last character of a row that wraps, since the end of such a row is the start of the next.
fn row_end(text: &str, row: &Range<usize>) -> usize {
    if row.end < text.len() && !text[row.end..].starts_with('\n') { prev(text, row.end) } else { row.end }
}

/// The byte on `row` nearest to `cells` cells in from its start.
pub fn at_cells(text: &str, row: &Range<usize>, cells: usize) -> usize {
    let end = row_end(text, row);
    let mut used = 0;
    for (k, c) in text[row.start..end].char_indices() {
        let w = crate::text::cells(c);
        if used + w / 2 >= cells {
            return row.start + k;
        }
        used += w;
    }
    end
}

/// What a key does to a field of several rows, `width` cells wide: Return starts a new line, the arrows up and down move between rows, and Home and End go to the ends of the row. Every other key does what it does in one line.
pub fn key_rows(text: &str, at: Caret, key: Key, m: Mods, kill: &mut String, width: usize) -> Action {
    let Mods { shift, alt, primary, control } = m;
    let at = Caret { caret: boundary(text, at.caret), anchor: boundary(text, at.anchor) };
    let moved = |to: usize| Action::Moved(Caret { caret: to, anchor: if shift { at.anchor } else { to } });
    match (key, primary, alt, control) {
        (Key::Enter, false, false, false) => insert(at, "\n"),
        (Key::Enter, true, false, false) => Action::Submit,
        (Key::Up | Key::Down, false, false, false) => {
            let (k, row) = row_of(text, width, at.caret);
            let cells: usize = text[row.start..at.caret].chars().map(crate::text::cells).sum();
            let target = if key == Key::Up { k.checked_sub(1) } else { Some(k + 1) };
            match target.and_then(|t| rows(text, width).nth(t)) {
                Some(row) => moved(at_cells(text, &row, cells)),
                None => moved(if key == Key::Up { 0 } else { text.len() }),
            }
        }
        (Key::Home, false, false, false) => moved(row_of(text, width, at.caret).1.start),
        (Key::End, false, false, false) => {
            let row = row_of(text, width, at.caret).1;
            moved(row_end(text, &row))
        }
        (Key::Up, true, false, false) => moved(0),
        (Key::Down, true, false, false) => moved(text.len()),
        _ => self::key(text, at, key, m, kill),
    }
}

/// What an expression a person typed comes to: whole and decimal numbers with `+`, `-`, `*`, `/` and brackets, the usual way round. Spaces, commas and underscores between digits are ignored, so a number as it is shown reads back.
pub fn evaluate(text: &str) -> Result<f64, &'static str> {
    let tokens: Vec<char> = text.chars().filter(|c| !matches!(c, ' ' | ',' | '_' | '\u{2009}' | '\u{202f}')).collect();
    if tokens.is_empty() {
        return Err("There is no number");
    }
    let mut at = 0;
    let value = sum(&tokens, &mut at)?;
    if at < tokens.len() {
        return Err("That isn't a number or a sum");
    }
    if value.is_finite() { Ok(value) } else { Err("That comes to no number, as dividing by zero does") }
}

fn sum(t: &[char], at: &mut usize) -> Result<f64, &'static str> {
    let mut value = product(t, at)?;
    while let Some(&op @ ('+' | '-')) = t.get(*at) {
        *at += 1;
        let next = product(t, at)?;
        value = if op == '+' { value + next } else { value - next };
    }
    Ok(value)
}

fn product(t: &[char], at: &mut usize) -> Result<f64, &'static str> {
    let mut value = unary(t, at)?;
    while let Some(&op @ ('*' | '×' | '/' | '÷')) = t.get(*at) {
        *at += 1;
        let next = unary(t, at)?;
        value = if matches!(op, '*' | '×') { value * next } else { value / next };
    }
    Ok(value)
}

fn unary(t: &[char], at: &mut usize) -> Result<f64, &'static str> {
    match t.get(*at) {
        Some('-' | '−') => {
            *at += 1;
            Ok(-unary(t, at)?)
        }
        Some('+') => {
            *at += 1;
            unary(t, at)
        }
        Some('(') => {
            *at += 1;
            let value = sum(t, at)?;
            if t.get(*at) != Some(&')') {
                return Err("A bracket is left open");
            }
            *at += 1;
            Ok(value)
        }
        _ => {
            let start = *at;
            while t.get(*at).is_some_and(|c| c.is_ascii_digit() || *c == '.') {
                *at += 1;
            }
            t[start..*at].iter().collect::<String>().parse().map_err(|_| "That isn't a number or a sum")
        }
    }
}
