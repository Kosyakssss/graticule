use crate::pmtiles::put_varint;

/// What a feature is drawn as, in the tile's own units.
#[derive(Clone, Debug, PartialEq)]
pub enum Geometry {
    Points(Vec<(i32, i32)>),
    Lines(Vec<Vec<(i32, i32)>>),
    /// Rings: an outline goes one way around and a hole the other, so filling by the winding leaves the holes empty.
    Areas(Vec<Vec<(i32, i32)>>),
}

/// One thing on a map tile: its geometry and the properties a map draws it by.
#[derive(Clone, Debug, PartialEq)]
pub struct Feature {
    pub geometry: Geometry,
    pub kind: String,
    pub name: Option<String>,
    /// How important it is among its kind, higher first: a place's population rank.
    pub rank: i64,
}

/// A named layer of a tile and its features, in units where the tile runs from 0 to `extent`.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub name: String,
    pub extent: u32,
    pub features: Vec<Feature>,
}

#[derive(Clone)]
enum Value {
    Text(String),
    Number(i64),
    Other,
}

struct Reader<'b> {
    bytes: &'b [u8],
    at: usize,
}

impl<'b> Reader<'b> {
    fn new(bytes: &'b [u8]) -> Self {
        Reader { bytes, at: 0 }
    }

    fn varint(&mut self) -> Option<u64> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let b = *self.bytes.get(self.at)?;
            self.at += 1;
            value |= u64::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return Some(value);
            }
        }
        None
    }

    fn field(&mut self) -> Option<(u64, Field<'b>)> {
        if self.at >= self.bytes.len() {
            return None;
        }
        let key = self.varint()?;
        let field = match key & 7 {
            0 => Field::Varint(self.varint()?),
            1 => Field::Fixed(self.take(8)?),
            2 => {
                let length = usize::try_from(self.varint()?).ok()?;
                Field::Bytes(self.take(length)?)
            }
            5 => Field::Fixed(self.take(4)?),
            _ => return None,
        };
        Some((key >> 3, field))
    }

    fn take(&mut self, length: usize) -> Option<&'b [u8]> {
        let bytes = self.bytes.get(self.at..self.at.checked_add(length)?)?;
        self.at += length;
        Some(bytes)
    }
}

enum Field<'b> {
    Varint(u64),
    Fixed(&'b [u8]),
    Bytes(&'b [u8]),
}

fn packed(bytes: &[u8]) -> Vec<u32> {
    let mut r = Reader::new(bytes);
    std::iter::from_fn(|| (r.at < r.bytes.len()).then(|| r.varint()).flatten()).map(|v| v as u32).collect()
}

fn zigzag(v: u32) -> i32 {
    ((v >> 1) as i32) ^ -((v & 1) as i32)
}

/// The layers of a Mapbox vector tile, version 2. What can't be read is left out rather than failing the tile.
pub fn decode(bytes: &[u8]) -> Vec<Layer> {
    let mut layers = Vec::new();
    let mut r = Reader::new(bytes);
    while let Some((n, field)) = r.field() {
        if let (3, Field::Bytes(layer)) = (n, field) {
            layers.extend(layer_of(layer));
        }
    }
    layers
}

fn layer_of(bytes: &[u8]) -> Option<Layer> {
    let (mut name, mut extent, mut keys, mut values, mut raw) = (String::new(), 4096, Vec::new(), Vec::new(), Vec::new());
    let mut r = Reader::new(bytes);
    while let Some((n, field)) = r.field() {
        match (n, field) {
            (1, Field::Bytes(b)) => name = String::from_utf8_lossy(b).into_owned(),
            (2, Field::Bytes(b)) => raw.push(b),
            (3, Field::Bytes(b)) => keys.push(String::from_utf8_lossy(b).into_owned()),
            (4, Field::Bytes(b)) => values.push(value_of(b)),
            (5, Field::Varint(v)) => extent = v as u32,
            _ => {}
        }
    }
    let features = raw.into_iter().filter_map(|b| feature_of(b, &keys, &values)).collect();
    Some(Layer { name, extent, features })
}

fn value_of(bytes: &[u8]) -> Value {
    let mut r = Reader::new(bytes);
    match r.field() {
        Some((1, Field::Bytes(b))) => Value::Text(String::from_utf8_lossy(b).into_owned()),
        Some((4 | 5, Field::Varint(v))) => Value::Number(v as i64),
        Some((6, Field::Varint(v))) => Value::Number(((v >> 1) as i64) ^ -((v & 1) as i64)),
        Some((3, Field::Fixed(b))) if b.len() == 8 => Value::Number(f64::from_le_bytes(b.try_into().unwrap_or_default()) as i64),
        Some((2, Field::Fixed(b))) if b.len() == 4 => Value::Number(f32::from_le_bytes(b.try_into().unwrap_or_default()) as i64),
        _ => Value::Other,
    }
}

fn feature_of(bytes: &[u8], keys: &[String], values: &[Value]) -> Option<Feature> {
    let (mut tags, mut kind_of, mut commands) = (Vec::new(), 0, Vec::new());
    let mut r = Reader::new(bytes);
    while let Some((n, field)) = r.field() {
        match (n, field) {
            (2, Field::Bytes(b)) => tags = packed(b),
            (3, Field::Varint(v)) => kind_of = v,
            (4, Field::Bytes(b)) => commands = packed(b),
            _ => {}
        }
    }
    let mut feature = Feature { geometry: Geometry::Points(Vec::new()), kind: String::new(), name: None, rank: 0 };
    for pair in tags.chunks_exact(2) {
        let (Some(key), Some(value)) = (keys.get(pair[0] as usize), values.get(pair[1] as usize)) else { continue };
        match (key.as_str(), value) {
            ("kind" | "pmap:kind", Value::Text(t)) => feature.kind = t.clone(),
            ("name", Value::Text(t)) => feature.name = Some(t.clone()),
            ("population_rank" | "rank", Value::Number(v)) => feature.rank = *v,
            _ => {}
        }
    }
    let rings = paths(&commands);
    feature.geometry = match kind_of {
        1 => Geometry::Points(rings.into_iter().flatten().collect()),
        2 => Geometry::Lines(rings),
        3 => Geometry::Areas(rings),
        _ => return None,
    };
    Some(feature)
}

fn paths(commands: &[u32]) -> Vec<Vec<(i32, i32)>> {
    let (mut out, mut path, mut at, mut k) = (Vec::new(), Vec::new(), (0i32, 0i32), 0);
    while k < commands.len() {
        let (id, count) = (commands[k] & 7, commands[k] >> 3);
        k += 1;
        match id {
            1 | 2 => {
                for _ in 0..count {
                    let (Some(&dx), Some(&dy)) = (commands.get(k), commands.get(k + 1)) else { return out };
                    k += 2;
                    at = (at.0.wrapping_add(zigzag(dx)), at.1.wrapping_add(zigzag(dy)));
                    if id == 1 && !path.is_empty() {
                        out.push(std::mem::take(&mut path));
                    }
                    path.push(at);
                }
            }
            7 => {
                if let Some(&first) = path.first() {
                    path.push(first);
                }
            }
            _ => return out,
        }
    }
    if !path.is_empty() {
        out.push(path);
    }
    out
}

/// A tile holding `layers`, as a Mapbox vector tile: the other half of [`decode`], for the sample map.
pub fn encode(layers: &[Layer]) -> Vec<u8> {
    let mut tile = Vec::new();
    for layer in layers {
        let mut keys: Vec<&str> = Vec::new();
        let mut values: Vec<Result<&str, i64>> = Vec::new();
        let mut body = Vec::new();
        bytes_field(&mut body, 1, layer.name.as_bytes());
        for feature in &layer.features {
            let mut tags = Vec::new();
            let mut pairs = vec![("kind", Ok(feature.kind.as_str()))];
            pairs.extend(feature.name.as_deref().map(|n| ("name", Ok(n))));
            if feature.rank != 0 {
                pairs.push(("population_rank", Err(feature.rank)));
            }
            for (key, value) in pairs {
                let k = keys.iter().position(|k| *k == key).unwrap_or_else(|| {
                    keys.push(key);
                    keys.len() - 1
                });
                let v = values.iter().position(|v| *v == value).unwrap_or_else(|| {
                    values.push(value);
                    values.len() - 1
                });
                tags.extend([k as u32, v as u32]);
            }
            let mut f = Vec::new();
            packed_field(&mut f, 2, &tags);
            let (kind_of, commands) = commands_of(&feature.geometry);
            put_varint(&mut f, 3 << 3);
            put_varint(&mut f, kind_of);
            packed_field(&mut f, 4, &commands);
            bytes_field(&mut body, 2, &f);
        }
        for key in &keys {
            bytes_field(&mut body, 3, key.as_bytes());
        }
        for value in &values {
            let mut v = Vec::new();
            match value {
                Ok(text) => bytes_field(&mut v, 1, text.as_bytes()),
                Err(n) => {
                    put_varint(&mut v, 4 << 3);
                    put_varint(&mut v, *n as u64);
                }
            }
            bytes_field(&mut body, 4, &v);
        }
        put_varint(&mut body, 5 << 3);
        put_varint(&mut body, u64::from(layer.extent));
        put_varint(&mut body, 15 << 3);
        put_varint(&mut body, 2);
        bytes_field(&mut tile, 3, &body);
    }
    tile
}

fn commands_of(geometry: &Geometry) -> (u64, Vec<u32>) {
    let mut out = Vec::new();
    let mut at = (0i32, 0i32);
    let mut step = |out: &mut Vec<u32>, p: (i32, i32)| {
        let (dx, dy) = (p.0 - at.0, p.1 - at.1);
        out.extend([((dx << 1) ^ (dx >> 31)) as u32, ((dy << 1) ^ (dy >> 31)) as u32]);
        at = p;
    };
    match geometry {
        Geometry::Points(points) => {
            out.push(1 | ((points.len() as u32) << 3));
            for &p in points {
                step(&mut out, p);
            }
            (1, out)
        }
        Geometry::Lines(paths) | Geometry::Areas(paths) => {
            let closed = matches!(geometry, Geometry::Areas(_));
            for path in paths {
                let path = if closed && path.len() > 1 && path.first() == path.last() { &path[..path.len() - 1] } else { &path[..] };
                let Some((&first, rest)) = path.split_first() else { continue };
                out.push(1 | (1 << 3));
                step(&mut out, first);
                out.push(2 | ((rest.len() as u32) << 3));
                for &p in rest {
                    step(&mut out, p);
                }
                if closed {
                    out.push(7 | (1 << 3));
                }
            }
            (if closed { 3 } else { 2 }, out)
        }
    }
}

fn bytes_field(out: &mut Vec<u8>, n: u64, bytes: &[u8]) {
    put_varint(out, (n << 3) | 2);
    put_varint(out, bytes.len() as u64);
    out.extend_from_slice(bytes);
}

fn packed_field(out: &mut Vec<u8>, n: u64, values: &[u32]) {
    let mut body = Vec::new();
    for v in values {
        put_varint(&mut body, u64::from(*v));
    }
    bytes_field(out, n, &body);
}
