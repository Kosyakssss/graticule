use std::any::Any;
use std::fmt;
use std::sync::Arc;

/// A self-describing value: how facts are saved, compared across versions and shown. Fields are kept by name, so a newer version can add a field and still read what an older one saved.
#[derive(Clone, Debug)]
pub enum Value {
    Unit,
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    Str(String),
    Key(u128),
    None,
    Some(Box<Value>),
    List(Vec<Value>),
    Map(Vec<(String, Value)>),
    Variant(String, Box<Value>),
    /// Text held where it already is, such as a document's rope, so making the value costs nothing however long the text; it is read out only when the value is written, shown or compared, and it is written and compared as a `Str`.
    Shared(Shared),
}

/// Where a [`Value::Shared`] reads its text from.
pub trait Source: Any + Send + Sync {
    fn text(&self) -> String;
    fn as_any(&self) -> &dyn Any;
}

/// Text a [`Value`] holds by reference.
#[derive(Clone)]
pub struct Shared(Arc<dyn Source>);

impl Shared {
    pub fn new(source: impl Source) -> Self {
        Shared(Arc::new(source))
    }

    pub fn text(&self) -> String {
        self.0.text()
    }

    /// What it reads from, to take back what was put in without reading it out.
    pub fn source(&self) -> &dyn Any {
        self.0.as_any()
    }
}

impl fmt::Debug for Shared {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Shared({:?})", self.text())
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Shared(a), Value::Shared(b)) => Arc::ptr_eq(&a.0, &b.0) || a.text() == b.text(),
            (Value::Shared(a), Value::Str(s)) | (Value::Str(s), Value::Shared(a)) => a.text() == *s,
            (Value::Unit, Value::Unit) | (Value::None, Value::None) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::UInt(a), Value::UInt(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Key(a), Value::Key(b)) => a == b,
            (Value::Some(a), Value::Some(b)) => a == b,
            (Value::List(a), Value::List(b)) => a == b,
            (Value::Map(a), Value::Map(b)) => a == b,
            (Value::Variant(a, x), Value::Variant(b, y)) => a == b && x == y,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Malformed;

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the saved data is malformed")
    }
}

impl std::error::Error for Malformed {}

fn put_uint(out: &mut Vec<u8>, mut n: u64) {
    loop {
        let byte = (n & 0x7f) as u8;
        n >>= 7;
        if n == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    put_uint(out, s.len() as u64);
    out.extend_from_slice(s.as_bytes());
}

fn get_byte(input: &mut &[u8]) -> Result<u8, Malformed> {
    let (b, rest) = input.split_first().ok_or(Malformed)?;
    *input = rest;
    Ok(*b)
}

fn get_bytes<'a>(input: &mut &'a [u8], n: usize) -> Result<&'a [u8], Malformed> {
    if input.len() < n {
        return Err(Malformed);
    }
    let (a, b) = input.split_at(n);
    *input = b;
    Ok(a)
}

fn get_uint(input: &mut &[u8]) -> Result<u64, Malformed> {
    let mut n = 0u64;
    for shift in (0..64).step_by(7) {
        let b = get_byte(input)?;
        n |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            return Ok(n);
        }
    }
    Err(Malformed)
}

fn get_str(input: &mut &[u8]) -> Result<String, Malformed> {
    let n = get_uint(input)? as usize;
    String::from_utf8(get_bytes(input, n)?.to_vec()).map_err(|_| Malformed)
}

impl Value {
    pub fn field(&self, name: &str) -> Option<&Value> {
        match self {
            Value::Map(fields) => fields.iter().find(|(n, _)| n == name).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn encode(&self, out: &mut Vec<u8>) {
        match self {
            Value::Unit => out.push(0),
            Value::Bool(b) => out.push(if *b { 2 } else { 1 }),
            Value::Int(i) => {
                out.push(3);
                put_uint(out, ((i << 1) ^ (i >> 63)) as u64);
            }
            Value::UInt(u) => {
                out.push(4);
                put_uint(out, *u);
            }
            Value::Float(f) => {
                out.push(5);
                out.extend_from_slice(&f.to_le_bytes());
            }
            Value::Str(s) => {
                out.push(6);
                put_str(out, s);
            }
            Value::Shared(s) => {
                out.push(6);
                put_str(out, &s.text());
            }
            Value::Key(k) => {
                out.push(7);
                out.extend_from_slice(&k.to_le_bytes());
            }
            Value::None => out.push(8),
            Value::Some(v) => {
                out.push(9);
                v.encode(out);
            }
            Value::List(items) => {
                out.push(10);
                put_uint(out, items.len() as u64);
                for item in items {
                    item.encode(out);
                }
            }
            Value::Map(fields) => {
                out.push(11);
                put_uint(out, fields.len() as u64);
                for (name, v) in fields {
                    put_str(out, name);
                    v.encode(out);
                }
            }
            Value::Variant(name, v) => {
                out.push(12);
                put_str(out, name);
                v.encode(out);
            }
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.encode(&mut out);
        out
    }

    pub fn decode(input: &mut &[u8]) -> Result<Value, Malformed> {
        Ok(match get_byte(input)? {
            0 => Value::Unit,
            1 => Value::Bool(false),
            2 => Value::Bool(true),
            3 => {
                let z = get_uint(input)?;
                Value::Int(((z >> 1) as i64) ^ -((z & 1) as i64))
            }
            4 => Value::UInt(get_uint(input)?),
            5 => Value::Float(f64::from_le_bytes(get_bytes(input, 8)?.try_into().map_err(|_| Malformed)?)),
            6 => Value::Str(get_str(input)?),
            7 => Value::Key(u128::from_le_bytes(get_bytes(input, 16)?.try_into().map_err(|_| Malformed)?)),
            8 => Value::None,
            9 => Value::Some(Box::new(Value::decode(input)?)),
            10 => {
                let n = get_uint(input)? as usize;
                let mut items = Vec::with_capacity(n.min(1 << 16));
                for _ in 0..n {
                    items.push(Value::decode(input)?);
                }
                Value::List(items)
            }
            11 => {
                let n = get_uint(input)? as usize;
                let mut fields = Vec::with_capacity(n.min(1 << 12));
                for _ in 0..n {
                    let name = get_str(input)?;
                    fields.push((name, Value::decode(input)?));
                }
                Value::Map(fields)
            }
            12 => {
                let name = get_str(input)?;
                Value::Variant(name, Box::new(Value::decode(input)?))
            }
            _ => return Err(Malformed),
        })
    }

    pub fn from_bytes(mut bytes: &[u8]) -> Result<Value, Malformed> {
        let v = Value::decode(&mut bytes)?;
        if bytes.is_empty() { Ok(v) } else { Err(Malformed) }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Unit => f.write_str("()"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Int(i) => write!(f, "{i}"),
            Value::UInt(u) => write!(f, "{u}"),
            Value::Float(x) => write!(f, "{x}"),
            Value::Str(s) => write!(f, "{s:?}"),
            Value::Shared(s) => write!(f, "{:?}", s.text()),
            Value::Key(k) => write!(f, "⟨{:x}:{}⟩", (k >> 64) as u64, *k as u64),
            Value::None => f.write_str("none"),
            Value::Some(v) => write!(f, "{v}"),
            Value::List(items) => {
                f.write_str("[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{item}")?;
                }
                f.write_str("]")
            }
            Value::Map(fields) => {
                f.write_str("{")?;
                for (i, (name, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{name}: {v}")?;
                }
                f.write_str("}")
            }
            Value::Variant(name, v) if **v == Value::Unit => f.write_str(name),
            Value::Variant(name, v) => write!(f, "{name}({v})"),
        }
    }
}
