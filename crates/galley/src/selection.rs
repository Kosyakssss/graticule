use std::ops::Range;

use ledger::{Field, Value};

use crate::changes::Changes;

/// One cursor or selected range: where it started and where it is now, in characters. When they are equal it is a cursor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Span {
    pub anchor: usize,
    pub head: usize,
}

impl Span {
    pub const fn cursor(at: usize) -> Self {
        Span { anchor: at, head: at }
    }

    pub const fn new(anchor: usize, head: usize) -> Self {
        Span { anchor, head }
    }

    /// The characters it covers, from its start to its end, whichever way it was made.
    pub fn range(&self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }
}

/// Every cursor and selected range in a text, at least one, in order and never overlapping, with one of them primary: the one the view follows and a single-cursor command uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selections {
    spans: Vec<Span>,
    primary: usize,
}

impl Default for Selections {
    fn default() -> Self {
        Selections::cursor(0)
    }
}

impl Selections {
    pub fn cursor(at: usize) -> Self {
        Selections { spans: vec![Span::cursor(at)], primary: 0 }
    }

    /// Selections from spans in any order, overlapping ones merged, inside a text `len` characters long. The primary is the one made from `spans[primary]`.
    pub fn new(spans: Vec<Span>, primary: usize, len: usize) -> Self {
        let mut s = Selections { primary: primary.min(spans.len().saturating_sub(1)), spans };
        if s.spans.is_empty() {
            s.spans.push(Span::cursor(0));
        }
        s.settle(len);
        s
    }

    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    pub fn primary(&self) -> Span {
        self.spans[self.primary]
    }

    pub fn primary_index(&self) -> usize {
        self.primary
    }

    /// Adds a span and makes it primary.
    pub fn add(&mut self, span: Span, len: usize) {
        self.spans.push(span);
        self.primary = self.spans.len() - 1;
        self.settle(len);
    }

    /// Keeps only the primary.
    pub fn only_primary(&mut self) {
        self.spans = vec![self.primary()];
        self.primary = 0;
    }

    /// Every span moved where `changes` put the text under it, in a text now `len` characters long.
    pub fn map(&mut self, changes: &Changes, len: usize) {
        let mut mapper = changes.mapper();
        for span in &mut self.spans {
            let (start, end) = (mapper.map(span.anchor.min(span.head)), mapper.map(span.anchor.max(span.head)));
            *span = if span.anchor <= span.head { Span::new(start, end) } else { Span::new(end, start) };
        }
        self.settle(len);
    }

    /// Every span replaced by what `each` makes of it, in a text `len` characters long.
    pub fn each(&mut self, len: usize, mut each: impl FnMut(Span) -> Span) {
        for span in &mut self.spans {
            *span = each(*span);
        }
        self.settle(len);
    }

    fn settle(&mut self, len: usize) {
        for span in &mut self.spans {
            *span = Span::new(span.anchor.min(len), span.head.min(len));
        }
        let primary = self.spans[self.primary];
        let mut order: Vec<usize> = (0..self.spans.len()).collect();
        order.sort_by_key(|&i| (self.spans[i].range().start, self.spans[i].range().end));
        let mut merged: Vec<Span> = Vec::with_capacity(self.spans.len());
        let mut primary_at = 0;
        for i in order {
            let span = self.spans[i];
            match merged.last_mut() {
                Some(last) if span.range().start < last.range().end || span.range() == last.range() => {
                    let range = last.range().start..last.range().end.max(span.range().end);
                    *last = if last.head < last.anchor { Span::new(range.end, range.start) } else { Span::new(range.start, range.end) };
                    if span == primary {
                        primary_at = merged.len() - 1;
                    }
                }
                _ => {
                    if span == primary {
                        primary_at = merged.len();
                    }
                    merged.push(span);
                }
            }
        }
        self.spans = merged;
        self.primary = primary_at;
    }
}

impl Field for Selections {
    ledger::register!(Selections);

    fn same(&self, other: &Self) -> bool {
        self == other
    }

    fn value(&self) -> Value {
        let spans = self.spans.iter().map(|s| Value::List(vec![Value::UInt(s.anchor as u64), Value::UInt(s.head as u64)]));
        Value::Map(vec![("spans".into(), Value::List(spans.collect())), ("primary".into(), Value::UInt(self.primary as u64))])
    }

    fn from_value(value: &Value) -> Option<Self> {
        let (Some(Value::List(spans)), Some(Value::UInt(primary))) = (value.field("spans"), value.field("primary")) else { return None };
        let spans = spans
            .iter()
            .map(|s| match s {
                Value::List(p) => match p.as_slice() {
                    [Value::UInt(a), Value::UInt(h)] => Some(Span::new(*a as usize, *h as usize)),
                    _ => None,
                },
                _ => None,
            })
            .collect::<Option<Vec<Span>>>()?;
        (!spans.is_empty() && (*primary as usize) < spans.len()).then_some(Selections { spans, primary: *primary as usize })
    }
}
