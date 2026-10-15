use crate::path::Field;
use crate::value::Value;

/// What was typed into an editor: the kernel sends one when the user types, deletes, moves or clicks in it, by the platform's own conventions. The text it reaches decides what each means at its cursors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Typed {
    /// Text typed or pasted, replacing what each selection holds.
    Insert(String),
    /// Return: a new line at each cursor.
    NewLine,
    /// Backspace or Delete, by a character or a word, or what each selection holds.
    Delete { forward: bool, word: bool },
    /// The cursors moved; extending keeps where each began, so it selects what was passed over.
    Move { to: Travel, extend: bool },
    /// A click at a position, in characters, or a drag that extends to it.
    Place { at: usize, extend: bool },
    /// Everything selected.
    SelectAll,
    /// Escape: only the primary cursor kept.
    Collapse,
}

/// Where the platform's movement keys go.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Travel {
    Left,
    Right,
    WordLeft,
    WordRight,
    Up,
    Down,
    /// Up by one of the rows long lines wrap into at this many cells, as the editor draws them.
    RowUp(u16),
    RowDown(u16),
    LineStart,
    LineEnd,
    /// Up by this many lines: what shows of the editor.
    PageUp(u16),
    PageDown(u16),
    Start,
    End,
}

/// Text an editor shows. The kernel asks only about the lines on screen and the cursors, so what it costs never grows with the text. Positions are counted in characters.
pub trait Editable: Field {
    /// How many lines there are.
    fn line_count(&self) -> usize;

    /// Hands the text of a line, without its line break, to `each` in pieces.
    fn line(&self, line: usize, each: &mut dyn FnMut(&str));

    /// Where a line starts.
    fn line_start(&self, line: usize) -> usize;

    /// The line a position is on.
    fn line_of(&self, at: usize) -> usize;

    /// Each cursor or selection as where it began and where it is, in order; returns which is primary.
    fn cursors(&self, out: &mut Vec<(usize, usize)>) -> usize;

    /// What the selections hold, one line each: what copying takes.
    fn selected(&self) -> String;
}

impl Travel {
    pub(crate) fn value(&self) -> Value {
        let (name, n) = match self {
            Travel::Left => ("left", 0),
            Travel::Right => ("right", 0),
            Travel::WordLeft => ("word left", 0),
            Travel::WordRight => ("word right", 0),
            Travel::Up => ("up", 0),
            Travel::Down => ("down", 0),
            Travel::RowUp(n) => ("row up", *n),
            Travel::RowDown(n) => ("row down", *n),
            Travel::LineStart => ("line start", 0),
            Travel::LineEnd => ("line end", 0),
            Travel::PageUp(n) => ("page up", *n),
            Travel::PageDown(n) => ("page down", *n),
            Travel::Start => ("start", 0),
            Travel::End => ("end", 0),
        };
        Value::Variant(name.into(), Box::new(Value::UInt(n as u64)))
    }

    pub(crate) fn from_value(value: &Value) -> Option<Travel> {
        let Value::Variant(name, n) = value else { return None };
        let n = match **n {
            Value::UInt(n) => u16::try_from(n).ok()?,
            _ => return None,
        };
        Some(match name.as_str() {
            "left" => Travel::Left,
            "right" => Travel::Right,
            "word left" => Travel::WordLeft,
            "word right" => Travel::WordRight,
            "up" => Travel::Up,
            "down" => Travel::Down,
            "row up" => Travel::RowUp(n),
            "row down" => Travel::RowDown(n),
            "line start" => Travel::LineStart,
            "line end" => Travel::LineEnd,
            "page up" => Travel::PageUp(n),
            "page down" => Travel::PageDown(n),
            "start" => Travel::Start,
            "end" => Travel::End,
            _ => return None,
        })
    }
}

impl Typed {
    pub(crate) fn value(&self) -> Value {
        let (name, inner) = match self {
            Typed::Insert(text) => ("insert", Value::Str(text.clone())),
            Typed::NewLine => ("new line", Value::Unit),
            Typed::Delete { forward, word } => ("delete", Value::List(vec![Value::Bool(*forward), Value::Bool(*word)])),
            Typed::Move { to, extend } => ("move", Value::List(vec![to.value(), Value::Bool(*extend)])),
            Typed::Place { at, extend } => ("place", Value::List(vec![Value::UInt(*at as u64), Value::Bool(*extend)])),
            Typed::SelectAll => ("select all", Value::Unit),
            Typed::Collapse => ("collapse", Value::Unit),
        };
        Value::Variant(name.into(), Box::new(inner))
    }

    pub(crate) fn from_value(value: &Value) -> Option<Typed> {
        let Value::Variant(name, inner) = value else { return None };
        Some(match (name.as_str(), &**inner) {
            ("insert", Value::Str(text)) => Typed::Insert(text.clone()),
            ("new line", _) => Typed::NewLine,
            ("delete", Value::List(p)) => match p.as_slice() {
                [Value::Bool(forward), Value::Bool(word)] => Typed::Delete { forward: *forward, word: *word },
                _ => return None,
            },
            ("move", Value::List(p)) => match p.as_slice() {
                [to, Value::Bool(extend)] => Typed::Move { to: Travel::from_value(to)?, extend: *extend },
                _ => return None,
            },
            ("place", Value::List(p)) => match p.as_slice() {
                [Value::UInt(at), Value::Bool(extend)] => Typed::Place { at: *at as usize, extend: *extend },
                _ => return None,
            },
            ("select all", _) => Typed::SelectAll,
            ("collapse", _) => Typed::Collapse,
            _ => return None,
        })
    }
}
