//! Typed filters: `people>100000 country:spain`, read against a record's own fields, as a filter field types them.

use ledger::{Facts, Field, Value};

use crate::message::Message;

/// A query read against a record's fields. Each term must hold: `name:word` holds when the field contains the word, `name=word` when it is the word, and `name>10`, `name>=10`, `name<10` and `name<=10` compare numbers. A bare word holds when any field of words contains it, and a term after `-` holds when it doesn't. Words in double quotes may hold spaces: `country:"united kingdom"`. Case is ignored.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Filter {
    terms: Vec<Term>,
}

#[derive(Clone, Debug, PartialEq)]
struct Term {
    not: bool,
    field: Option<String>,
    op: Op,
    word: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    Has,
    Is,
    More,
    AtLeast,
    Less,
    AtMost,
}

impl Filter {
    /// Reads `query` for records of type `T`, or says what is wrong with it: a field `T` doesn't have, a term with nothing after it, a comparison with something that isn't a number.
    pub fn parse<T: Facts>(query: &str) -> Result<Filter, Message> {
        let mut terms = Vec::new();
        for token in tokens(query) {
            let (not, token) = match token.strip_prefix('-') {
                Some(rest) if !rest.is_empty() => (true, rest.to_string()),
                _ => (false, token),
            };
            let Some(at) = token.find([':', '=', '<', '>']) else {
                terms.push(Term { not, field: None, op: Op::Has, word: token.to_lowercase() });
                continue;
            };
            let (name, rest) = token.split_at(at);
            let (op, word) = match rest.as_bytes() {
                [b'>', b'=', ..] => (Op::AtLeast, &rest[2..]),
                [b'<', b'=', ..] => (Op::AtMost, &rest[2..]),
                [b'>', ..] => (Op::More, &rest[1..]),
                [b'<', ..] => (Op::Less, &rest[1..]),
                [b'=', ..] => (Op::Is, &rest[1..]),
                _ => (Op::Has, &rest[1..]),
            };
            let Some(field) = T::FIELDS.iter().find(|f| f.name.eq_ignore_ascii_case(name)) else {
                return Err(Message::plain(format!("There is no field “{name}”; the fields are {}", names::<T>().join(", "))));
            };
            if word.is_empty() {
                return Err(Message::plain(format!("Say what “{}” should be", field.name)));
            }
            if !matches!(op, Op::Has | Op::Is) && number(word).is_none() {
                return Err(Message::plain(format!("“{word}” is not a number to compare “{}” with", field.name)));
            }
            terms.push(Term { not, field: Some(field.name.to_string()), op, word: word.trim_matches('"').to_lowercase() });
        }
        Ok(Filter { terms })
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// Whether `item` passes every term.
    pub fn keeps<T: Field>(&self, item: &T) -> bool {
        if self.terms.is_empty() {
            return true;
        }
        let Value::Map(fields) = item.value() else { return false };
        self.terms.iter().all(|t| t.not != t.holds(&fields))
    }
}

impl Term {
    fn holds(&self, fields: &[(String, Value)]) -> bool {
        let Some(name) = &self.field else {
            return fields.iter().any(|(_, v)| matches!(plain(v), Value::Str(_) | Value::Variant(..)) && words(v).to_lowercase().contains(&self.word));
        };
        let Some((_, value)) = fields.iter().find(|(n, _)| n == name) else { return false };
        let value = plain(value);
        let compare = |f: fn(f64, f64) -> bool| matches!((number_of(value), number(&self.word)), (Some(a), Some(b)) if f(a, b));
        match self.op {
            Op::Has => words(value).to_lowercase().contains(&self.word),
            Op::Is => words(value).to_lowercase() == self.word,
            Op::More => compare(|a, b| a > b),
            Op::AtLeast => compare(|a, b| a >= b),
            Op::Less => compare(|a, b| a < b),
            Op::AtMost => compare(|a, b| a <= b),
        }
    }
}

/// The names of `T`'s fields that a filter can read, in order.
pub fn names<T: Facts>() -> Vec<&'static str> {
    T::FIELDS.iter().map(|f| f.name).filter(|n| *n != "id").collect()
}

fn tokens(query: &str) -> Vec<String> {
    let (mut out, mut word, mut quoted) = (Vec::new(), String::new(), false);
    for c in query.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !word.is_empty() {
                    out.push(std::mem::take(&mut word));
                }
            }
            c => word.push(c),
        }
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

fn plain(value: &Value) -> &Value {
    match value {
        Value::Some(inner) => plain(inner),
        v => v,
    }
}

fn words(value: &Value) -> String {
    match plain(value) {
        Value::Str(s) => s.clone(),
        Value::Shared(s) => s.text(),
        Value::Bool(b) => if *b { "yes" } else { "no" }.to_string(),
        Value::Int(n) => n.to_string(),
        Value::UInt(n) => n.to_string(),
        Value::Float(n) => n.to_string(),
        Value::Variant(name, _) => name.clone(),
        _ => String::new(),
    }
}

fn number_of(value: &Value) -> Option<f64> {
    match plain(value) {
        Value::Int(n) => Some(*n as f64),
        Value::UInt(n) => Some(*n as f64),
        Value::Float(n) => Some(*n),
        _ => None,
    }
}

fn number(word: &str) -> Option<f64> {
    word.chars().filter(|c| *c != ',' && *c != '_').collect::<String>().parse().ok()
}
