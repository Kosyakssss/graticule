use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::Mutex;

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;

/// A tile's place: its zoom level and its column and row at that level.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tile {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

impl Tile {
    /// Its number in a PMTiles archive: the tiles of every lower level first, then its place along a Hilbert curve over its own level.
    pub fn id(self) -> u64 {
        let n = 1u64 << self.z;
        let before = ((n * n) - 1) / 3;
        let (mut x, mut y) = (u64::from(self.x), u64::from(self.y));
        let mut d = 0;
        let mut s = n / 2;
        while s > 0 {
            let rx = u64::from(x & s > 0);
            let ry = u64::from(y & s > 0);
            d += s * s * ((3 * rx) ^ ry);
            if ry == 0 {
                if rx == 1 {
                    x = n - 1 - x;
                    y = n - 1 - y;
                }
                std::mem::swap(&mut x, &mut y);
            }
            s /= 2;
        }
        before + d
    }
}

#[derive(Copy, Clone, Debug)]
struct Entry {
    id: u64,
    offset: u64,
    length: u64,
    run: u64,
}

/// Where an archive's bytes are: a file read a piece at a time, or bytes held whole.
enum Bytes {
    File(Mutex<File>),
    Held(Vec<u8>),
}

impl Bytes {
    fn read(&self, offset: u64, length: u64) -> Option<Vec<u8>> {
        match self {
            Bytes::File(file) => {
                let mut file = file.lock().ok()?;
                file.seek(SeekFrom::Start(offset)).ok()?;
                let mut out = vec![0; usize::try_from(length).ok()?];
                file.read_exact(&mut out).ok()?;
                Some(out)
            }
            Bytes::Held(bytes) => bytes.get(usize::try_from(offset).ok()?..usize::try_from(offset + length).ok()?).map(<[u8]>::to_vec),
        }
    }
}

/// An archive of map tiles in the PMTiles format, version 3: one file holding every tile, found through directories at its start, so a map reads only the tiles it shows.
pub struct Archive {
    bytes: Bytes,
    root: Vec<Entry>,
    leaves: Mutex<HashMap<u64, Vec<Entry>>>,
    leaf_start: u64,
    data_start: u64,
    gzipped_tiles: bool,
    gzipped_directories: bool,
    pub min_zoom: u8,
    pub max_zoom: u8,
    /// Where the map starts: longitude and latitude in degrees, and the zoom.
    pub center: (f64, f64, u8),
}

const HEADER: usize = 127;

impl Archive {
    /// Opens the archive in a file, reading only its header and root directory.
    pub fn open(path: &str) -> Result<Archive, String> {
        let file = File::open(path).map_err(|e| e.to_string())?;
        Archive::from(Bytes::File(Mutex::new(file)))
    }

    /// The archive in `bytes`.
    pub fn held(bytes: Vec<u8>) -> Result<Archive, String> {
        Archive::from(Bytes::Held(bytes))
    }

    fn from(bytes: Bytes) -> Result<Archive, String> {
        let head = bytes.read(0, HEADER as u64).ok_or("The file is too short to be a map")?;
        if &head[0..7] != b"PMTiles" || head[7] != 3 {
            return Err("The file isn't a PMTiles map of version 3".into());
        }
        let u = |at: usize| u64::from_le_bytes(head[at..at + 8].try_into().unwrap_or_default());
        let i = |at: usize| f64::from(i32::from_le_bytes(head[at..at + 4].try_into().unwrap_or_default())) / 1e7;
        let compression = |c: u8| match c {
            1 => Ok(false),
            2 => Ok(true),
            _ => Err("The map is compressed in a way Atlas can't read yet".to_string()),
        };
        if head[99] != 1 {
            return Err("The map's tiles aren't vector tiles".into());
        }
        let mut archive = Archive {
            root: Vec::new(),
            leaves: Mutex::new(HashMap::new()),
            leaf_start: u(40),
            data_start: u(56),
            gzipped_directories: compression(head[97])?,
            gzipped_tiles: compression(head[98])?,
            min_zoom: head[100],
            max_zoom: head[101],
            center: (i(119), i(123), head[118]),
            bytes,
        };
        let root = archive.bytes.read(u(8), u(16)).ok_or("The map's directory is cut short")?;
        archive.root = archive.directory(&root).ok_or("The map's directory can't be read")?;
        Ok(archive)
    }

    fn directory(&self, bytes: &[u8]) -> Option<Vec<Entry>> {
        let bytes = if self.gzipped_directories { unzip(bytes)? } else { bytes.to_vec() };
        let mut at = 0;
        let mut next = || varint(&bytes, &mut at);
        let count = usize::try_from(next()?).ok()?;
        let mut entries = vec![Entry { id: 0, offset: 0, length: 0, run: 0 }; count];
        let mut id = 0;
        for e in &mut entries {
            id += next()?;
            e.id = id;
        }
        for e in &mut entries {
            e.run = next()?;
        }
        for e in &mut entries {
            e.length = next()?;
        }
        for k in 0..count {
            let v = next()?;
            entries[k].offset = match (v, k) {
                (0, k) if k > 0 => entries[k - 1].offset + entries[k - 1].length,
                (v, _) => v.checked_sub(1)?,
            };
        }
        Some(entries)
    }

    /// The tile's bytes, unpacked, or `None` when the archive has no such tile.
    pub fn tile(&self, tile: Tile) -> Option<Vec<u8>> {
        let id = tile.id();
        let mut entries = self.root.clone();
        for _ in 0..4 {
            let at = entries.partition_point(|e| e.id <= id).checked_sub(1)?;
            let e = entries[at];
            if e.run > 0 {
                if id >= e.id + e.run {
                    return None;
                }
                let bytes = self.bytes.read(self.data_start + e.offset, e.length)?;
                return if self.gzipped_tiles { unzip(&bytes) } else { Some(bytes) };
            }
            let known = self.leaves.lock().ok()?.get(&e.offset).cloned();
            entries = match known {
                Some(leaf) => leaf,
                None => {
                    let leaf = self.directory(&self.bytes.read(self.leaf_start + e.offset, e.length)?)?;
                    self.leaves.lock().ok()?.insert(e.offset, leaf.clone());
                    leaf
                }
            };
        }
        None
    }
}

fn unzip(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    GzDecoder::new(bytes).read_to_end(&mut out).ok()?;
    Some(out)
}

fn zip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    let _ = encoder.write_all(bytes);
    encoder.finish().unwrap_or_default()
}

fn varint(bytes: &[u8], at: &mut usize) -> Option<u64> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let b = *bytes.get(*at)?;
        *at += 1;
        value |= u64::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            return Some(value);
        }
    }
    None
}

pub fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8 & 0x7f) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

/// An archive holding `tiles`, every one gzipped and the whole directory in the root, for a map small enough that its directory needs no leaves.
pub fn write(mut tiles: Vec<(Tile, Vec<u8>)>, zooms: (u8, u8), center: (f64, f64, u8)) -> Vec<u8> {
    tiles.sort_by_key(|(t, _)| t.id());
    let mut data = Vec::new();
    let mut entries = Vec::new();
    for (tile, bytes) in &tiles {
        let packed = zip(bytes);
        entries.push(Entry { id: tile.id(), offset: data.len() as u64, length: packed.len() as u64, run: 1 });
        data.extend(packed);
    }
    let mut dir = Vec::new();
    put_varint(&mut dir, entries.len() as u64);
    let mut last = 0;
    for e in &entries {
        put_varint(&mut dir, e.id - last);
        last = e.id;
    }
    for e in &entries {
        put_varint(&mut dir, e.run);
    }
    for e in &entries {
        put_varint(&mut dir, e.length);
    }
    for e in &entries {
        put_varint(&mut dir, e.offset + 1);
    }
    let dir = zip(&dir);
    let mut out = Vec::with_capacity(HEADER + dir.len() + data.len());
    out.extend_from_slice(b"PMTiles");
    out.push(3);
    let e7 = |v: f64| ((v * 1e7) as i32).to_le_bytes();
    let (root, data_at) = (HEADER as u64, (HEADER + dir.len()) as u64);
    for v in [root, dir.len() as u64, data_at, 0, data_at, 0, data_at, data.len() as u64, tiles.len() as u64, tiles.len() as u64, tiles.len() as u64] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&[1, 2, 2, 1, zooms.0, zooms.1]);
    for v in [-180.0, -85.0, 180.0, 85.0] {
        out.extend_from_slice(&e7(v));
    }
    out.push(center.2);
    out.extend_from_slice(&e7(center.0));
    out.extend_from_slice(&e7(center.1));
    out.extend(dir);
    out.extend(data);
    out
}
