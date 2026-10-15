use std::f64::consts::{PI, TAU};

use crate::mvt::{Feature, Geometry, Layer};
use crate::pmtiles::{Tile, write};

/// The units of a tile, and how far past its edges a tile carries what crosses them, so lines and outlines meet across tiles.
pub const EXTENT: u32 = 4096;
const BUFFER: f64 = 64.0;

/// The deepest level the sample map has tiles for; nearer, its tiles are drawn larger.
const DEEPEST: u8 = 12;

/// A point on the map in Web Mercator units: the whole world runs from 0 to 1 across, from the antimeridian, and down, from the north.
pub fn mercator(lon: f64, lat: f64) -> (f64, f64) {
    let lat = lat.clamp(-85.05, 85.05).to_radians();
    ((lon + 180.0) / 360.0, (1.0 - (lat.tan() + 1.0 / lat.cos()).ln() / PI) / 2.0)
}

/// Longitude and latitude in degrees of a point in Web Mercator units.
pub fn degrees((x, y): (f64, f64)) -> (f64, f64) {
    (x * 360.0 - 180.0, (PI * (1.0 - 2.0 * y)).sinh().atan().to_degrees())
}

struct Thing {
    layer: &'static str,
    kind: &'static str,
    name: Option<String>,
    rank: i64,
    /// The shallowest level it shows at.
    from: u8,
    shape: Shape,
    /// Its box in Web Mercator units: left, right, top and bottom.
    reach: [f64; 4],
}

enum Shape {
    Point((f64, f64)),
    Line(Vec<(f64, f64)>),
    Area(Vec<(f64, f64)>),
}

const NAMES: [&str; 32] = [
    "Port Ellery",
    "Calloway",
    "Merrin",
    "Saltmarsh",
    "Oskar Bay",
    "Tolly",
    "Feldane",
    "Wyck",
    "Harrowgate",
    "Pellin",
    "Corvey",
    "Amberly",
    "Stroud",
    "Lisk",
    "Brannock",
    "Quill",
    "Varden",
    "Ostry",
    "Kellow",
    "Ninefield",
    "Tamsin",
    "Garrow",
    "Holm",
    "Dunmere",
    "Rook Harbour",
    "Elsing",
    "Marrow",
    "Teague",
    "Fenwick",
    "Lowen",
    "Brisk",
    "Cadel",
];

const ISLANDS: [(&str, f64, f64, f64); 7] = [
    ("Great Ellery", -45.0, 35.0, 0.34),
    ("Merrin", -44.35, 35.32, 0.2),
    ("Tolly", -45.55, 35.45, 0.16),
    ("Wyck", -44.6, 34.6, 0.14),
    ("Corvey", -45.7, 34.75, 0.11),
    ("Lisk", -44.15, 34.95, 0.08),
    ("Holm", -45.25, 35.72, 0.07),
];

struct Noise(u64);

impl Noise {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn world() -> Vec<Thing> {
    let mut things = made();
    for thing in &mut things {
        let points: &[(f64, f64)] = match &thing.shape {
            Shape::Point(p) => std::slice::from_ref(p),
            Shape::Line(ps) | Shape::Area(ps) => ps,
        };
        thing.reach = points.iter().fold([f64::MAX, f64::MIN, f64::MAX, f64::MIN], |r, p| [r[0].min(p.0), r[1].max(p.0), r[2].min(p.1), r[3].max(p.1)]);
    }
    things
}

fn made() -> Vec<Thing> {
    let mut noise = Noise(0x9e37_79b9_7f4a_7c15);
    let mut things = Vec::new();
    let mut names = NAMES.iter();
    for (n, &(island, lon, lat, r)) in ISLANDS.iter().enumerate() {
        let phase: Vec<f64> = (0..3).map(|_| noise.next() * TAU).collect();
        let reach = |a: f64| r * (1.0 + 0.22 * (3.0 * a + phase[0]).sin() + 0.1 * (7.0 * a + phase[1]).sin() + 0.04 * (17.0 * a + phase[2]).sin());
        let at = |a: f64, d: f64| mercator(lon + d * a.cos() / lat.to_radians().cos(), lat + d * a.sin());
        let outline: Vec<(f64, f64)> = (0..240).map(|k| f64::from(k) / 240.0 * TAU).map(|a| at(a, reach(a))).collect();
        things.push(Thing { layer: "earth", kind: "earth", name: None, rank: 0, from: 0, shape: Shape::Area(outline), reach: [0.0; 4] });
        things.push(Thing {
            layer: "places",
            kind: "island",
            name: Some(island.to_string()),
            rank: 12 - n as i64,
            from: if n < 3 { 6 } else { 8 },
            shape: Shape::Point(at(PI * 1.5, r * 0.55)),
            reach: [0.0; 4],
        });
        if r > 0.15 {
            let lake = (noise.next() * TAU, r * 0.35, r * 0.09);
            let middle = (lon + lake.1 * lake.0.cos(), lat + lake.1 * lake.0.sin());
            let ring = (0..48).map(|k| f64::from(k) / 48.0 * TAU).map(|a| mercator(middle.0 + lake.2 * 1.6 * a.cos(), middle.1 + lake.2 * a.sin())).collect();
            things.push(Thing { layer: "water", kind: "lake", name: None, rank: 0, from: 8, shape: Shape::Area(ring), reach: [0.0; 4] });
            let park = (lake.0 + PI, r * 0.4, r * 0.12);
            let corner = (lon + park.1 * park.0.cos(), lat + park.1 * park.0.sin());
            let ring = (0..36).map(|k| f64::from(k) / 36.0 * TAU).map(|a| mercator(corner.0 + park.2 * a.cos() * (1.0 + 0.3 * (3.0 * a).sin()), corner.1 + park.2 * a.sin())).collect();
            things.push(Thing { layer: "landuse", kind: "park", name: None, rank: 0, from: 10, shape: Shape::Area(ring), reach: [0.0; 4] });
        }
        let towns = 1 + (r * 14.0) as usize;
        let mut placed: Vec<((f64, f64), i64)> = Vec::new();
        for t in 0..towns {
            let a = noise.next() * TAU;
            let d = reach(a) * (0.25 + 0.6 * noise.next());
            let rank = if t == 0 { 9 - n as i64 / 2 } else { 3 + (noise.next() * 4.0) as i64 };
            let Some(name) = names.next() else { break };
            let p = at(a, d);
            placed.push((p, rank));
            things.push(Thing {
                layer: "places",
                kind: "locality",
                name: Some(name.to_string()),
                rank,
                from: if rank >= 8 {
                    7
                } else if rank >= 5 {
                    9
                } else {
                    10
                },
                shape: Shape::Point(p),
                reach: [0.0; 4],
            });
            streets(&mut things, &mut noise, degrees(p), rank);
        }
        if let Some((&(hub, _), rest)) = placed.split_first() {
            for &(town, rank) in rest {
                let mid = ((hub.0 + town.0) / 2.0, (hub.1 + town.1) / 2.0);
                let bend = (r * 0.04 * (noise.next() - 0.5)) / 360.0;
                let road: Vec<(f64, f64)> = vec![hub, (mid.0 + bend, mid.1 - bend), town];
                let major = rank >= 5;
                things.push(Thing {
                    layer: "roads",
                    kind: if major { "major_road" } else { "minor_road" },
                    name: None,
                    rank: 0,
                    from: if major { 8 } else { 10 },
                    shape: Shape::Line(road),
                    reach: [0.0; 4],
                });
            }
        }
    }
    things
}

/// A town's streets, running out from its middle and round it, and its houses along them.
fn streets(things: &mut Vec<Thing>, noise: &mut Noise, (lon, lat): (f64, f64), rank: i64) {
    let reach = 0.0015 * rank as f64;
    let wide = 1.0 / lat.to_radians().cos();
    let at = |a: f64, d: f64| mercator(lon + d * a.cos() * wide, lat + d * a.sin());
    let ring = (0..=32).map(|k| f64::from(k) / 32.0 * TAU).map(|a| at(a, reach * 0.6)).collect();
    things.push(Thing { layer: "roads", kind: "minor_road", name: None, rank: 0, from: 11, shape: Shape::Line(ring), reach: [0.0; 4] });
    let ways = 3 + rank as usize / 2;
    let turn = noise.next() * TAU;
    for k in 0..ways {
        let a = turn + k as f64 / ways as f64 * TAU;
        things.push(Thing { layer: "roads", kind: "minor_road", name: None, rank: 0, from: 11, shape: Shape::Line(vec![at(a, 0.0), at(a, reach)]), reach: [0.0; 4] });
        let mut d = 0.0006;
        while d < reach {
            for side in [-1.0, 1.0] {
                let (along, off) = (at(a, d), 0.00018 * side);
                let (x, y) = degrees(along);
                let (nx, ny) = (-a.sin() * off * wide, a.cos() * off);
                let s = 0.00007 + 0.00004 * noise.next();
                let middle = (x + nx, y + ny);
                let house = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].iter().map(|(u, v)| mercator(middle.0 + u * s * wide, middle.1 + v * s)).collect();
                things.push(Thing { layer: "buildings", kind: "building", name: None, rank: 0, from: 12, shape: Shape::Area(house), reach: [0.0; 4] });
            }
            d += 0.0005;
        }
    }
}

/// Where a town of the sample map is, in Web Mercator units.
pub fn town(name: &str) -> Option<(f64, f64)> {
    world().into_iter().find(|t| t.name.as_deref() == Some(name)).and_then(|t| if let Shape::Point(p) = t.shape { Some(p) } else { None })
}

/// The sample map: a made-up archipelago in the open Atlantic, cut into vector tiles at every level down to the twelfth and packed as a PMTiles archive, as a real map file is.
pub fn sample() -> Vec<u8> {
    let things = world();
    let bounds = things.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |b, t| {
        let points: &[(f64, f64)] = match &t.shape {
            Shape::Point(p) => std::slice::from_ref(p),
            Shape::Line(ps) | Shape::Area(ps) => ps,
        };
        points.iter().fold(b, |b, p| (b.0.min(p.0), b.1.min(p.1), b.2.max(p.0), b.3.max(p.1)))
    });
    let mut tiles = Vec::new();
    for z in 0..=DEEPEST {
        let n = f64::from(1u32 << z);
        let span = |a: f64, b: f64| ((a * n).floor() as u32)..=(((b * n).floor() as u32).min((1 << z) - 1));
        for x in span(bounds.0, bounds.2) {
            for y in span(bounds.1, bounds.3) {
                let tile = Tile { z, x, y };
                let layers = cut(&things, tile);
                if !layers.is_empty() {
                    tiles.push((tile, crate::mvt::encode(&layers)));
                }
            }
        }
    }
    let (lon, lat) = degrees(((bounds.0 + bounds.2) / 2.0, (bounds.1 + bounds.3) / 2.0));
    write(tiles, (0, DEEPEST), (lon, lat, 7))
}

fn cut(things: &[Thing], tile: Tile) -> Vec<Layer> {
    let n = f64::from(1u32 << tile.z);
    let e = f64::from(EXTENT);
    let local = |(x, y): (f64, f64)| ((x * n - f64::from(tile.x)) * e, (y * n - f64::from(tile.y)) * e);
    let edge = (-BUFFER, e + BUFFER);
    let mut layers: Vec<Layer> = Vec::new();
    let (x0, y0) = (f64::from(tile.x) / n, f64::from(tile.y) / n);
    let margin = BUFFER / e / n;
    for thing in things.iter().filter(|t| t.from <= tile.z && t.reach[0] <= x0 + 1.0 / n + margin && t.reach[1] >= x0 - margin && t.reach[2] <= y0 + 1.0 / n + margin && t.reach[3] >= y0 - margin) {
        let rounded = |ps: Vec<(f64, f64)>| {
            let mut out: Vec<(i32, i32)> = ps.into_iter().map(|(x, y)| (x.round() as i32, y.round() as i32)).collect();
            out.dedup();
            out
        };
        let geometry = match &thing.shape {
            Shape::Point(p) => {
                let p = local(*p);
                if !(0.0..e).contains(&p.0) || !(0.0..e).contains(&p.1) {
                    continue;
                }
                Geometry::Points(rounded(vec![p]))
            }
            Shape::Area(ring) => {
                let ring = rounded(clip(ring.iter().copied().map(local).collect(), edge));
                if ring.len() < 3 {
                    continue;
                }
                Geometry::Areas(vec![ring])
            }
            Shape::Line(line) => {
                let pieces: Vec<Vec<(i32, i32)>> = cut_line(&line.iter().copied().map(local).collect::<Vec<_>>(), edge).into_iter().map(rounded).filter(|p| p.len() > 1).collect();
                if pieces.is_empty() {
                    continue;
                }
                Geometry::Lines(pieces)
            }
        };
        let feature = Feature { geometry, kind: thing.kind.to_string(), name: thing.name.clone(), rank: thing.rank };
        match layers.iter_mut().find(|l| l.name == thing.layer) {
            Some(layer) => layer.features.push(feature),
            None => layers.push(Layer { name: thing.layer.to_string(), extent: EXTENT, features: vec![feature] }),
        }
    }
    layers
}

/// The part of a closed outline inside the square from `lo` to `hi` both ways.
fn clip(mut ring: Vec<(f64, f64)>, (lo, hi): (f64, f64)) -> Vec<(f64, f64)> {
    let sides: [(usize, f64, bool); 4] = [(0, lo, true), (0, hi, false), (1, lo, true), (1, hi, false)];
    for (axis, at, above) in sides {
        let inside = |p: (f64, f64)| {
            let v = if axis == 0 { p.0 } else { p.1 };
            if above { v >= at } else { v <= at }
        };
        let cross = |a: (f64, f64), b: (f64, f64)| {
            let (va, vb) = if axis == 0 { (a.0, b.0) } else { (a.1, b.1) };
            let t = (at - va) / (vb - va);
            (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1))
        };
        let mut out = Vec::with_capacity(ring.len());
        for k in 0..ring.len() {
            let (a, b) = (ring[(k + ring.len() - 1) % ring.len()], ring[k]);
            match (inside(a), inside(b)) {
                (true, true) => out.push(b),
                (true, false) => out.push(cross(a, b)),
                (false, true) => out.extend([cross(a, b), b]),
                (false, false) => {}
            }
        }
        ring = out;
        if ring.is_empty() {
            break;
        }
    }
    ring
}

/// The pieces of a line inside the square from `lo` to `hi` both ways.
fn cut_line(line: &[(f64, f64)], (lo, hi): (f64, f64)) -> Vec<Vec<(f64, f64)>> {
    let mut pieces = Vec::new();
    let mut piece: Vec<(f64, f64)> = Vec::new();
    for pair in line.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
        let d = (b.0 - a.0, b.1 - a.1);
        let mut keep = true;
        for (p, q) in [(-d.0, a.0 - lo), (d.0, hi - a.0), (-d.1, a.1 - lo), (d.1, hi - a.1)] {
            if p == 0.0 {
                keep &= q >= 0.0;
            } else {
                let r = q / p;
                if p < 0.0 { t0 = t0.max(r) } else { t1 = t1.min(r) }
            }
        }
        if !keep || t0 > t1 {
            if piece.len() > 1 {
                pieces.push(std::mem::take(&mut piece));
            }
            piece.clear();
            continue;
        }
        let (from, to) = ((a.0 + t0 * d.0, a.1 + t0 * d.1), (a.0 + t1 * d.0, a.1 + t1 * d.1));
        if piece.last() != Some(&from) {
            if piece.len() > 1 {
                pieces.push(std::mem::take(&mut piece));
            }
            piece = vec![from];
        }
        piece.push(to);
        if t1 < 1.0 {
            pieces.push(std::mem::take(&mut piece));
        }
    }
    if piece.len() > 1 {
        pieces.push(piece);
    }
    pieces
}
