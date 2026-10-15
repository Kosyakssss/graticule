use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use parley::FontContext;
use parley::fontique::{Blob, Collection, CollectionOptions, FallbackKey, FamilyId, Language, Script};

pub const MONO: [&[u8]; 2] = [include_bytes!("../assets/fonts/IBMPlexMono-Var.ttf"), include_bytes!("../assets/fonts/IBMPlexMono-VarItalic.ttf")];

pub const SANS: [&[u8]; 6] = [
    include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf"),
    include_bytes!("../assets/fonts/IBMPlexSans-Italic.ttf"),
    include_bytes!("../assets/fonts/IBMPlexSans-Medium.ttf"),
    include_bytes!("../assets/fonts/IBMPlexSans-SemiBold.ttf"),
    include_bytes!("../assets/fonts/IBMPlexSans-SemiBoldItalic.ttf"),
    include_bytes!("../assets/fonts/IBMPlexSans-Bold.ttf"),
];

/// A font file's bytes and which font in it to use.
pub type FontFile = (Arc<[u8]>, u32);

/// The file of the font family called `name`, upright and of regular weight, and which font in it: one of the Plex families Graticule carries, or else one installed on the system, found by name as the system's font menus show it. Looking for a system font scans them all the first time, so do it off the main thread; each answer is remembered.
pub fn family(name: &str) -> Option<FontFile> {
    static FOUND: std::sync::Mutex<Option<HashMap<String, Option<FontFile>>>> = std::sync::Mutex::new(None);
    let key = name.trim().to_lowercase();
    if let Some(found) = FOUND.lock().ok()?.get_or_insert_with(HashMap::new).get(&key) {
        return found.clone();
    }
    let bundled = match key.as_str() {
        "ibm plex sans" => Some(SANS[0]),
        "ibm plex mono" => Some(MONO[0]),
        _ => EXTRA.iter().find_map(|f| match f.files {
            Files::Bundled(files) if f.name.to_lowercase() == key => files.first().copied(),
            _ => None,
        }),
    };
    let found = match bundled {
        Some(bytes) => Some((Arc::from(bytes), 0)),
        None => {
            let mut collection = Collection::new(CollectionOptions { shared: false, system_fonts: true });
            let family = collection.family_by_name(name.trim())?;
            let font = family
                .match_index(parley::fontique::FontWidth::NORMAL, parley::fontique::FontStyle::Normal, parley::fontique::FontWeight::NORMAL, true)
                .and_then(|i| family.fonts().get(i))
                .or(family.default_font())?;
            font.load(None).map(|blob| (Arc::from(blob.data()), font.index()))
        }
    };
    FOUND.lock().ok()?.get_or_insert_with(HashMap::new).insert(key, found.clone());
    found
}

/// How many bytes of fonts every program carries inside it: the fetched ones are not counted.
pub fn bundled() -> usize {
    let extra: usize = EXTRA
        .iter()
        .map(|f| match f.files {
            Files::Bundled(files) => files.iter().map(|b| b.len()).sum(),
            Files::Fetched(_) => 0,
        })
        .sum();
    MONO.iter().chain(SANS.iter()).map(|b| b.len()).sum::<usize>() + extra
}

pub enum Files {
    Bundled(&'static [&'static [u8]]),
    Fetched(&'static [&'static str]),
}

pub struct Family {
    pub name: &'static str,
    pub scripts: &'static [[u8; 4]],
    pub ranges: &'static [(u32, u32)],
    pub files: Files,
}

const HAN_RANGES: &[(u32, u32)] = &[
    (0x2E80, 0x2FDF),
    (0x3000, 0x303F),
    (0x31C0, 0x31EF),
    (0x3200, 0x33FF),
    (0x3400, 0x4DBF),
    (0x4E00, 0x9FFF),
    (0xF900, 0xFAFF),
    (0xFE30, 0xFE4F),
    (0xFF00, 0xFF60),
    (0xFFE0, 0xFFEF),
    (0x20000, 0x3134F),
];

const JP: usize = 5;
const KR: usize = 6;
const SC: usize = 7;
const TC: usize = 8;

pub const EXTRA: [Family; 9] = [
    Family {
        name: "IBM Plex Sans Arabic",
        scripts: &[*b"Arab"],
        ranges: &[(0x0600, 0x06FF), (0x0750, 0x077F), (0x0870, 0x08FF), (0xFB50, 0xFDFF), (0xFE70, 0xFEFF)],
        files: Files::Bundled(&[
            include_bytes!("../assets/fonts/IBMPlexSansArabic-Regular.ttf"),
            include_bytes!("../assets/fonts/IBMPlexSansArabic-Medium.ttf"),
            include_bytes!("../assets/fonts/IBMPlexSansArabic-SemiBold.ttf"),
            include_bytes!("../assets/fonts/IBMPlexSansArabic-Bold.ttf"),
        ]),
    },
    Family {
        name: "IBM Plex Sans Devanagari",
        scripts: &[*b"Deva"],
        ranges: &[(0x0900, 0x097F), (0x1CD0, 0x1CFF), (0xA8E0, 0xA8FF)],
        files: Files::Bundled(&[
            include_bytes!("../assets/fonts/IBMPlexSansDevanagari-Regular.ttf"),
            include_bytes!("../assets/fonts/IBMPlexSansDevanagari-Medium.ttf"),
            include_bytes!("../assets/fonts/IBMPlexSansDevanagari-SemiBold.ttf"),
            include_bytes!("../assets/fonts/IBMPlexSansDevanagari-Bold.ttf"),
        ]),
    },
    Family {
        name: "IBM Plex Sans Hebrew",
        scripts: &[*b"Hebr"],
        ranges: &[(0x0590, 0x05FF), (0xFB1D, 0xFB4F)],
        files: Files::Bundled(&[
            include_bytes!("../assets/fonts/IBMPlexSansHebrew-Regular.ttf"),
            include_bytes!("../assets/fonts/IBMPlexSansHebrew-Medium.ttf"),
            include_bytes!("../assets/fonts/IBMPlexSansHebrew-SemiBold.ttf"),
            include_bytes!("../assets/fonts/IBMPlexSansHebrew-Bold.ttf"),
        ]),
    },
    Family {
        name: "IBM Plex Sans Thai",
        scripts: &[*b"Thai"],
        ranges: &[(0x0E00, 0x0E7F)],
        files: Files::Bundled(&[
            include_bytes!("../assets/fonts/IBMPlexSansThai-Regular.ttf"),
            include_bytes!("../assets/fonts/IBMPlexSansThai-Medium.ttf"),
            include_bytes!("../assets/fonts/IBMPlexSansThai-SemiBold.ttf"),
            include_bytes!("../assets/fonts/IBMPlexSansThai-Bold.ttf"),
        ]),
    },
    Family {
        name: "IBM Plex Math",
        scripts: &[*b"Latn", *b"Cyrl", *b"Grek", *b"Zyyy", *b"Zmth"],
        ranges: &[(0x2100, 0x214F), (0x2190, 0x23FF), (0x25A0, 0x25FF), (0x27C0, 0x2AFF), (0x1D400, 0x1D7FF)],
        files: Files::Bundled(&[include_bytes!("../assets/fonts/IBMPlexMath-Regular.ttf")]),
    },
    Family {
        name: "IBM Plex Sans JP",
        scripts: &[*b"Hira", *b"Kana", *b"Jpan"],
        ranges: &[(0x3040, 0x30FF), (0x31F0, 0x31FF), (0xFF61, 0xFF9F)],
        files: Files::Fetched(&["IBMPlexSansJP-Regular.ttf", "IBMPlexSansJP-SemiBold.ttf"]),
    },
    Family {
        name: "IBM Plex Sans KR",
        scripts: &[*b"Hang", *b"Kore"],
        ranges: &[(0x1100, 0x11FF), (0x3130, 0x318F), (0xA960, 0xA97F), (0xAC00, 0xD7FF), (0xFFA0, 0xFFDC)],
        files: Files::Fetched(&["IBMPlexSansKR-Regular.ttf", "IBMPlexSansKR-SemiBold.ttf"]),
    },
    Family { name: "IBM Plex Sans SC", scripts: &[*b"Hans"], ranges: &[], files: Files::Fetched(&["IBMPlexSansSC-Regular.ttf", "IBMPlexSansSC-SemiBold.ttf"]) },
    Family {
        name: "IBM Plex Sans TC",
        scripts: &[*b"Bopo", *b"Hant"],
        ranges: &[(0x3100, 0x312F), (0x31A0, 0x31BF)],
        files: Files::Fetched(&["IBMPlexSansTC-Regular.ttf", "IBMPlexSansTC-SemiBold.ttf"]),
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Han {
    Chinese,
    Japanese,
    Korean,
}

impl Han {
    pub fn of(text: &str) -> Self {
        let mut korean = false;
        for c in text.chars() {
            match c as u32 {
                0x3040..=0x30FF | 0x31F0..=0x31FF | 0xFF61..=0xFF9F => return Han::Japanese,
                0x1100..=0x11FF | 0x3130..=0x318F | 0xAC00..=0xD7AF => korean = true,
                _ => {}
            }
        }
        if korean { Han::Korean } else { Han::Chinese }
    }

    pub fn locale(self) -> Option<Language> {
        match self {
            Han::Chinese => None,
            Han::Japanese => Language::parse("ja").ok(),
            Han::Korean => Language::parse("ko").ok(),
        }
    }

    fn order(self) -> [usize; 4] {
        match self {
            Han::Chinese => [SC, TC, JP, KR],
            Han::Japanese => [JP, SC, TC, KR],
            Han::Korean => [KR, SC, TC, JP],
        }
    }
}

pub fn fetched_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(contents) = exe.parent().and_then(|p| p.parent())
    {
        dirs.push(contents.join("Resources").join("Fonts"));
    }
    if let Some(data) = dirs::data_dir() {
        dirs.push(data.join("Graticule").join("Fonts"));
    }
    dirs
}

#[derive(Clone)]
enum Opened {
    NotYet,
    Missing,
    Ready(Vec<Blob<u8>>),
}

#[derive(Clone, Copy)]
enum Cover {
    Core,
    Extra(usize),
    Missing,
}

fn maps(file: &[u8], c: char) -> bool {
    swash::FontRef::from_index(file, 0).is_some_and(|f| f.charmap().map(c) != 0)
}

fn within(ranges: &[(u32, u32)], code: u32) -> bool {
    ranges.iter().any(|(a, b)| (*a..=*b).contains(&code))
}

pub struct Seed {
    opened: Vec<Opened>,
    loaded: [bool; EXTRA.len()],
}

pub struct Fonts {
    pub cx: FontContext,
    pub mono: String,
    pub sans: String,
    opened: Vec<Opened>,
    ids: [Vec<FamilyId>; EXTRA.len()],
    loaded: [bool; EXTRA.len()],
    covered: HashMap<(char, Han), Cover>,
}

impl Fonts {
    pub fn new(system_fonts: bool) -> Self {
        let mut cx = FontContext { collection: Collection::new(CollectionOptions { shared: false, system_fonts }), source_cache: Default::default() };
        let blobs = |files: &[&'static [u8]]| files.iter().map(|f| Blob::new(Arc::new(*f))).collect::<Vec<_>>();
        let mono = register(&mut cx, &blobs(&MONO)).1;
        let sans = register(&mut cx, &blobs(&SANS)).1;
        let opened = EXTRA
            .iter()
            .map(|f| match f.files {
                Files::Bundled(files) => Opened::Ready(blobs(files)),
                Files::Fetched(_) => Opened::NotYet,
            })
            .collect();
        Self { cx, mono, sans, opened, ids: Default::default(), loaded: [false; EXTRA.len()], covered: HashMap::new() }
    }

    pub fn seed(&self) -> Seed {
        Seed { opened: self.opened.clone(), loaded: self.loaded }
    }

    pub fn with_system(seed: Seed) -> Self {
        let mut fonts = Self::new(true);
        fonts.opened = seed.opened;
        for (key, _) in keys() {
            fonts.cx.collection.fallback_families(key).count();
        }
        for (i, loaded) in seed.loaded.iter().enumerate() {
            if *loaded {
                fonts.load(i);
            }
        }
        fonts
    }

    fn open(&mut self, i: usize) -> Option<&[Blob<u8>]> {
        if let (Opened::NotYet, Files::Fetched(names)) = (&self.opened[i], &EXTRA[i].files) {
            let dirs = fetched_dirs();
            let read: Option<Vec<Blob<u8>>> = names.iter().map(|name| dirs.iter().find_map(|d| std::fs::read(d.join(name)).ok()).map(|bytes| Blob::new(Arc::new(bytes)))).collect();
            self.opened[i] = read.map_or(Opened::Missing, Opened::Ready);
        }
        match &self.opened[i] {
            Opened::Ready(files) => Some(files),
            _ => None,
        }
    }

    fn maps(&mut self, i: usize, c: char) -> bool {
        self.open(i).and_then(|files| files.first()).is_some_and(|f| maps(f.as_ref(), c))
    }

    fn cover(&mut self, c: char, han: Han) -> Cover {
        if let Some(cover) = self.covered.get(&(c, han)) {
            return *cover;
        }
        let code = c as u32;
        let cover = if maps(MONO[0], c) || maps(SANS[0], c) {
            Cover::Core
        } else if within(HAN_RANGES, code) {
            han.order().into_iter().find(|i| self.maps(*i, c)).map_or(Cover::Missing, Cover::Extra)
        } else {
            (0..EXTRA.len()).find(|i| within(EXTRA[*i].ranges, code) && self.maps(*i, c)).map_or(Cover::Missing, Cover::Extra)
        };
        self.covered.insert((c, han), cover);
        cover
    }

    fn load(&mut self, i: usize) {
        if self.loaded[i] {
            return;
        }
        let Some(files) = self.open(i).map(<[_]>::to_vec) else { return };
        self.loaded[i] = true;
        self.ids[i] = register(&mut self.cx, &files).0;
        let ours: Vec<FamilyId> = self.ids.iter().flatten().copied().collect();
        for (key, owners) in keys() {
            let order = owners.iter().copied().chain((0..EXTRA.len()).filter(|j| !owners.contains(j)));
            let first: Vec<FamilyId> = order.filter(|j| self.loaded[*j]).flat_map(|j| self.ids[j].iter().copied()).collect();
            let rest: Vec<FamilyId> = self.cx.collection.fallback_families(key).filter(|id| !ours.contains(id)).collect();
            self.cx.collection.set_fallbacks(key, first.into_iter().chain(rest));
        }
    }

    pub fn prepare(&mut self, text: &str, han: Han) -> bool {
        let mut complete = true;
        for c in text.chars() {
            if c.is_ascii() || crate::chars::drawn(c) {
                continue;
            }
            match self.cover(c, han) {
                Cover::Core => {}
                Cover::Extra(i) => self.load(i),
                Cover::Missing => complete = false,
            }
        }
        complete
    }
}

fn keys() -> Vec<(FallbackKey, Vec<usize>)> {
    let mut keys: Vec<(FallbackKey, Vec<usize>)> = Vec::new();
    for (i, family) in EXTRA.iter().enumerate() {
        for script in family.scripts {
            let key = FallbackKey::new(Script::from_bytes(*script), None);
            match keys.iter_mut().find(|(k, _)| *k == key) {
                Some((_, owners)) => owners.push(i),
                None => keys.push((key, vec![i])),
            }
        }
    }
    for han in [Han::Chinese, Han::Japanese, Han::Korean] {
        keys.push((FallbackKey::new(Script::from_bytes(*b"Hani"), han.locale().as_ref()), han.order().to_vec()));
    }
    keys
}

fn register(cx: &mut FontContext, files: &[Blob<u8>]) -> (Vec<FamilyId>, String) {
    let mut ids = Vec::new();
    let mut name = String::new();
    for file in files {
        for (family, _) in cx.collection.register_fonts(file.clone(), None) {
            if !ids.contains(&family) {
                ids.push(family);
            }
            if name.is_empty() {
                name = cx.collection.family_name(family).unwrap_or_default().to_string();
            }
        }
    }
    (ids, name)
}
