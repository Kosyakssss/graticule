use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path as FsPath, PathBuf};

use graticule::kernel::image::Image;
use graticule::ledger::{self, Choose};
use graticule::prelude::*;

#[derive(Facts)]
struct Twin {
    #[session]
    left: Text,
    #[session]
    right: Text,
    #[session]
    active: Side,
    #[session]
    hidden: Hidden,
    #[session]
    reads: u64,
    #[session]
    disk: Disk,
    #[session]
    left_chosen: Selection<Entry>,
    #[session]
    right_chosen: Selection<Entry>,
    #[derived(Listing::read, from = [left, hidden, reads, disk])]
    left_listing: Derived<Listing>,
    #[derived(Listing::read, from = [right, hidden, reads, disk])]
    right_listing: Derived<Listing>,
    #[derived(Preview::of, from = [active, left_chosen, right_chosen, left_listing, right_listing, disk])]
    preview: Derived<Preview>,
}

/// One of the two sides.
#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
enum Side {
    #[default]
    Left,
    Right,
}

/// Whether files whose names start with a dot show.
#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
enum Hidden {
    #[default]
    Hide,
    Show,
}

/// Where the folders are read from: the computer's own disk, or the sample the named states use, which never changes.
#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
enum Disk {
    #[default]
    Real,
    Sample,
}

/// A file or folder in a listing.
#[derive(Facts)]
struct Entry {
    #[key]
    id: Key<Entry>,
    name: String,
    path: String,
    folder: bool,
    kind: String,
    size: u64,
}

/// A folder's entries, folders first and then by name, read off the main thread.
#[derive(Default)]
struct Listing {
    entries: List<Entry>,
    failure: Option<String>,
}

/// What the chosen entry on the active side holds, read off the main thread.
#[derive(Default)]
enum Preview {
    #[default]
    Nothing,
    Folder(usize),
    Text(String),
    Picture(Image),
    Other(String),
}

#[derive(Commands)]
enum Cmd {
    /// Goes into a folder, or opens a file in the app the system opens it with.
    #[label("Open", many: "Open {count} files")]
    Open(#[subject] Key<Entry>),
    /// Goes to the folder that holds this side's folder.
    #[label("Go up a folder")]
    GoUp,
    /// Copies the chosen files and folders into the folder on the other side, in the background.
    #[label("Copy to the other side", many: "Copy {count} items to the other side")]
    Copy(#[subject] Key<Entry>),
    /// Moves the chosen files and folders into the folder on the other side, in the background.
    #[label("Move to the other side", many: "Move {count} items to the other side")]
    Move(#[subject] Key<Entry>),
    #[label("Rename…")]
    Rename {
        #[subject]
        entry: Key<Entry>,
        name: String,
    },
    #[label("New folder…")]
    NewFolder { name: String },
    #[label("Show the same folder on the other side")]
    Mirror,
    #[label("Swap the sides")]
    Swap,
    #[label("Show hidden files")]
    ShowHidden,
    #[label("Hide hidden files")]
    HideHidden,
    #[label("Read the folders again")]
    Reread,
    #[internal]
    Dropped(Path),
    #[internal]
    Did(Result<String, Failure>),
    #[internal]
    Edit(Edit),
}

impl App for Twin {
    type Cmd = Cmd;

    fn available(&self, cmd: &Cmd) -> Available {
        match cmd {
            Cmd::Open(entry) | Cmd::Copy(entry) | Cmd::Move(entry) | Cmd::Rename { entry, .. } => no_if(self.locate(*entry).is_none(), t!("That file is gone")),
            Cmd::GoUp => no_if(parent(self.folder(self.active)).is_none(), t!("This is the top folder")),
            Cmd::Mirror => no_if(self.left == self.right, t!("Both sides show the same folder already")),
            Cmd::Swap => no_if(self.left == self.right, t!("Both sides show the same folder")),
            Cmd::ShowHidden => no_if(self.hidden == Hidden::Show, t!("Hidden files show already")),
            Cmd::HideHidden => no_if(self.hidden == Hidden::Hide, t!("Hidden files are hidden already")),
            Cmd::NewFolder { .. } | Cmd::Reread | Cmd::Dropped(_) | Cmd::Did(_) | Cmd::Edit(_) => Yes,
        }
    }

    fn update(&mut self, cmd: Cmd, cx: &mut Cx<Cmd>) -> Outcome {
        match cmd {
            Cmd::Open(key) => {
                let Some((side, entry)) = self.locate(key) else { return Outcome::Nothing };
                let (path, folder) = (entry.path.clone(), entry.folder);
                self.active = side;
                if folder {
                    self.go(side, &path);
                    return Outcome::Nothing;
                }
                if self.disk == Disk::Sample {
                    return cx.say(t!("This is the sample folder, so nothing opens"));
                }
                cx.open(path);
                Outcome::Nothing
            }
            Cmd::GoUp => {
                if let Some(up) = parent(self.folder(self.active)) {
                    self.go(self.active, &up);
                }
                Outcome::Nothing
            }
            Cmd::Copy(key) | Cmd::Move(key) => {
                let moving = matches!(cmd, Cmd::Move(_));
                let Some((side, entry)) = self.locate(key) else { return Outcome::Nothing };
                let (from, name) = (PathBuf::from(&entry.path), entry.name.clone());
                let to = PathBuf::from(self.folder(side.other()).as_str());
                match moving {
                    true => self.work(cx, t!("Moving “{name}”"), move || move_into(&from, &to)),
                    false => self.work(cx, t!("Copying “{name}”"), move || copy_into(&from, &to)),
                }
                Outcome::Nothing
            }
            Cmd::Rename { entry, name } => {
                let Some((_, entry)) = self.locate(entry) else { return Outcome::Nothing };
                let from = PathBuf::from(&entry.path);
                if name.is_empty() || name.contains(['/', '\\']) {
                    return cx.say(t!("A name can't be empty or hold a slash"));
                }
                self.work(cx, t!("Renaming “{name}”"), move || rename(&from, &name));
                Outcome::Nothing
            }
            Cmd::NewFolder { name } => {
                if name.is_empty() || name.contains(['/', '\\']) {
                    return cx.say(t!("A name can't be empty or hold a slash"));
                }
                let at = PathBuf::from(self.folder(self.active).as_str()).join(&name);
                self.work(cx, t!("Making the folder “{name}”"), move || new_folder(&at));
                Outcome::Nothing
            }
            Cmd::Dropped(path) => {
                let to = PathBuf::from(self.folder(self.active).as_str());
                let name = path.0.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                self.work(cx, t!("Copying “{name}”"), move || copy_into(&path.0, &to));
                Outcome::Nothing
            }
            Cmd::Did(done) => {
                self.reads += 1;
                match done {
                    Ok(said) => cx.say(t!("{said}")),
                    Err(failure) => cx.say_failure(failure),
                }
            }
            Cmd::Mirror => {
                let here = self.folder(self.active).clone();
                self.go(self.active.other(), here.as_str());
                Outcome::Nothing
            }
            Cmd::Swap => {
                let (left, right) = (self.left.to_string(), self.right.to_string());
                self.go(Side::Left, &right);
                self.go(Side::Right, &left);
                Outcome::Nothing
            }
            Cmd::ShowHidden | Cmd::HideHidden => {
                self.hidden = if matches!(cmd, Cmd::ShowHidden) { Hidden::Show } else { Hidden::Hide };
                Outcome::Nothing
            }
            Cmd::Reread => {
                self.reads += 1;
                Outcome::Nothing
            }
            Cmd::Edit(edit) => {
                if edit.touches(Twin::right_chosen) || edit.touches(Twin::right) {
                    self.active = Side::Right;
                } else if edit.touches(Twin::left_chosen) || edit.touches(Twin::left) {
                    self.active = Side::Left;
                }
                edit.apply(self)
            }
        }
    }

    fn view(&self, ui: &mut Ui<Cmd>) {
        ui.page(t!("Twin"), |ui| {
            ui.on_files(Cmd::Dropped);
            ui.split(|ui| {
                ui.pane(t!("Left"), Fill, |ui| self.side(ui, Side::Left));
                ui.pane(t!("Right"), Fill, |ui| self.side(ui, Side::Right));
                ui.pane(t!("Preview"), Fill, |ui| self.show_preview(ui));
            });
        });
    }

    fn states() -> States<Self> {
        let sample = |left: &str, right: &str| Twin { left: Text::from(left), right: Text::from(right), disk: Disk::Sample, ..Twin::default() };
        States::new()
            .add("Two folders", sample("Sample", "Sample/Documents"))
            .add("A letter chosen", {
                let mut twin = sample("Sample/Documents", "Sample");
                let _ = twin.left_chosen.choose(&Choose::Only(key_of("Sample/Documents/letter.txt").raw()));
                twin
            })
            .add("Hidden files shown", Twin { hidden: Hidden::Show, ..sample("Sample", "Sample/Code") })
            .add("A huge folder", sample("Sample/A huge folder", "Sample"))
            .add("A folder that isn't there", sample("Sample/Nowhere", "Sample"))
    }
}

impl Default for Twin {
    fn default() -> Self {
        let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_else(|_| ".".into());
        Twin {
            left: Text::from(home.as_str()),
            right: Text::from(home.as_str()),
            active: Side::Left,
            hidden: Hidden::Hide,
            reads: 0,
            disk: Disk::Real,
            left_chosen: Selection::new(),
            right_chosen: Selection::new(),
            left_listing: Derived::default(),
            right_listing: Derived::default(),
            preview: Derived::default(),
        }
    }
}

impl Side {
    fn other(self) -> Side {
        match self {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }
}

impl Twin {
    fn folder(&self, side: Side) -> &Text {
        match side {
            Side::Left => &self.left,
            Side::Right => &self.right,
        }
    }

    fn listing(&self, side: Side) -> Option<&Listing> {
        match side {
            Side::Left => self.left_listing.get(),
            Side::Right => self.right_listing.get(),
        }
    }

    fn locate(&self, key: Key<Entry>) -> Option<(Side, &Entry)> {
        [self.active, self.active.other()].into_iter().find_map(|side| self.listing(side).and_then(|l| l.entries.find(key)).map(|e| (side, e)))
    }

    fn work(&self, cx: &mut Cx<Cmd>, name: Message, work: impl FnOnce() -> Result<String, Failure> + Send + 'static) {
        let sample = self.disk == Disk::Sample;
        cx.job(name, move || if sample { Ok("This is the sample folder, so nothing on disk changes".to_string()) } else { work() }, Cmd::Did);
    }

    fn go(&mut self, side: Side, folder: &str) {
        match side {
            Side::Left => {
                let _ = self.left.set(folder);
                let _ = self.left_chosen.choose(&Choose::Clear);
            }
            Side::Right => {
                let _ = self.right.set(folder);
                let _ = self.right_chosen.choose(&Choose::Clear);
            }
        }
    }

    fn side(&self, ui: &mut Ui<Cmd>, side: Side) {
        let (folder, chosen) = match side {
            Side::Left => (Twin::left, Twin::left_chosen),
            Side::Right => (Twin::right, Twin::right_chosen),
        };
        ui.field(folder, t!("Folder"));
        let Some(listing) = self.listing(side) else {
            ui.muted(t!("Reading the folder…"));
            return;
        };
        if let Some(failure) = &listing.failure {
            ui.muted(t!("{failure}"));
            return;
        }
        let count = listing.entries.len();
        ui.muted(match count {
            0 => t!("Nothing here"),
            1 => t!("One item"),
            _ => t!("{count} items"),
        });
        ui.table(&listing.entries, chosen, |row, entry| {
            row.cell("Name", &entry.name).width(Fill.weight(2).min(6));
            row.cell("Kind", &entry.kind);
            row.cell("Size", if entry.folder { String::new() } else { size(entry.size) });
        });
    }

    fn show_preview(&self, ui: &mut Ui<Cmd>) {
        match self.preview.get() {
            None | Some(Preview::Nothing) => ui.muted(t!("Choose a file to see what it holds")),
            Some(Preview::Folder(count)) => ui.muted(t!("A folder of {count} items")),
            Some(Preview::Text(text)) => ui.text(text),
            Some(Preview::Picture(image)) => ui.image(t!("The picture"), image, 12),
            Some(Preview::Other(kind)) => ui.muted(t!("{kind}, which can't be shown here")),
        };
    }
}

impl Listing {
    fn read(folder: &Text, hidden: &Hidden, _: &u64, disk: &Disk) -> Listing {
        let folder = folder.as_str();
        let found = match disk {
            Disk::Real => real(folder),
            Disk::Sample => sample(folder).ok_or_else(|| format!("There is no folder “{folder}”")),
        };
        let mut found = match found {
            Ok(found) => found,
            Err(failure) => return Listing { entries: List::new(), failure: Some(failure) },
        };
        found.retain(|(name, ..)| *hidden == Hidden::Show || !name.starts_with('.'));
        found.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase())).then_with(|| a.0.cmp(&b.0)));
        let entries = found
            .into_iter()
            .map(|(name, folder_too, size)| {
                let path = join(folder, &name);
                Entry { id: key_of(&path), kind: kind(&name, folder_too), path, name, folder: folder_too, size }
            })
            .collect();
        Listing { entries, failure: None }
    }
}

impl Preview {
    fn of(active: &Side, left: &Selection<Entry>, right: &Selection<Entry>, left_listing: &Derived<Listing>, right_listing: &Derived<Listing>, disk: &Disk) -> Preview {
        let (chosen, listing) = match active {
            Side::Left => (left, left_listing),
            Side::Right => (right, right_listing),
        };
        let Some(entry) = chosen.focused().and_then(|key| listing.get()?.entries.find(key)) else { return Preview::Nothing };
        if entry.folder {
            let count = match disk {
                Disk::Real => std::fs::read_dir(&entry.path).map_or(0, Iterator::count),
                Disk::Sample => sample(&entry.path).map_or(0, |f| f.len()),
            };
            return Preview::Folder(count);
        }
        let bytes = match disk {
            Disk::Real => {
                use std::io::Read;
                let mut head = Vec::new();
                let read = std::fs::File::open(&entry.path).and_then(|f| f.take(PREVIEW_BYTES).read_to_end(&mut head));
                match read {
                    Ok(_) => head,
                    Err(e) => return Preview::Other(e.to_string()),
                }
            }
            Disk::Sample => SAMPLE_TEXTS.iter().find(|(name, _)| *name == entry.name).map_or_else(Vec::new, |(_, text)| text.as_bytes().to_vec()),
        };
        if entry.kind.ends_with("image") && (bytes.len() as u64) < PREVIEW_BYTES {
            return Image::decode(&bytes).map_or_else(|e| Preview::Other(e.to_string()), Preview::Picture);
        }
        match std::str::from_utf8(&bytes) {
            Ok(text) => Preview::Text(text.to_string()),
            Err(e) if e.valid_up_to() + 4 >= bytes.len() && e.valid_up_to() > 0 => Preview::Text(String::from_utf8_lossy(&bytes[..e.valid_up_to()]).into_owned()),
            Err(_) => Preview::Other(entry.kind.clone()),
        }
    }
}

/// How much of a file the preview reads.
const PREVIEW_BYTES: u64 = 16 * 1024;

type Found = Vec<(String, bool, u64)>;

fn real(folder: &str) -> Result<Found, String> {
    let read = std::fs::read_dir(folder).map_err(|e| format!("“{folder}” can't be read: {e}"))?;
    Ok(read
        .filter_map(Result::ok)
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let meta = e.metadata().ok();
            let folder_too = meta.as_ref().is_some_and(std::fs::Metadata::is_dir);
            (name, folder_too, meta.map_or(0, |m| m.len()))
        })
        .collect())
}

fn sample(folder: &str) -> Option<Found> {
    let file = |name: &str| {
        let size = SAMPLE_TEXTS.iter().find(|(n, _)| *n == name).map_or(2_048, |(_, t)| t.len() as u64);
        (name.to_string(), false, size)
    };
    let folder_of = |name: &str| (name.to_string(), true, 0);
    Some(match folder.trim_end_matches(['/', '\\']).replace('\\', "/").as_str() {
        "Sample" => vec![folder_of("Documents"), folder_of("Code"), folder_of("Pictures"), folder_of("A huge folder"), file("notes.txt"), file(".settings")],
        "Sample/Documents" => vec![file("letter.txt"), file("plan.md"), file("report.pdf"), file("budget.csv")],
        "Sample/Code" => vec![file("main.rs"), file("Cargo.toml"), file(".gitignore"), folder_of("target")],
        "Sample/Code/target" => Vec::new(),
        "Sample/Pictures" => vec![file("harbour.png"), file("street.jpg"), file("scan.tiff")],
        "Sample/A huge folder" => (1..=100_000).map(|n| (format!("record {n:06}.txt"), false, 100 + n % 900)).collect(),
        _ => return None,
    })
}

const SAMPLE_TEXTS: &[(&str, &str)] = &[
    ("letter.txt", "Dear reader,\n\nThis folder is a sample that Twin's named states show, so the pictures of them never change.\n\nWith thanks,\nTwin\n"),
    ("plan.md", "# Plan\n\n- Read the folder off the main thread\n- Copy and move in the background\n"),
    ("notes.txt", "Two sides, one folder each.\n"),
    ("main.rs", "fn main() {\n    println!(\"Hello from the sample\");\n}\n"),
    ("budget.csv", "item,cost\npaper,12\nink,30\n"),
];

fn parent(folder: &Text) -> Option<String> {
    let up = FsPath::new(folder.as_str()).parent()?;
    let up = up.to_string_lossy();
    (!up.is_empty()).then(|| up.into_owned())
}

fn join(folder: &str, name: &str) -> String {
    match folder.ends_with(['/', '\\']) {
        true => format!("{folder}{name}"),
        false if folder.starts_with("Sample") => format!("{folder}/{name}"),
        false => FsPath::new(folder).join(name).to_string_lossy().into_owned(),
    }
}

fn key_of(path: &str) -> Key<Entry> {
    let mut a = DefaultHasher::new();
    path.hash(&mut a);
    let mut b = DefaultHasher::new();
    (path, 1u8).hash(&mut b);
    Key::from_raw((u128::from(a.finish()) << 64) | u128::from(b.finish()))
}

fn kind(name: &str, folder: bool) -> String {
    if folder {
        return "Folder".into();
    }
    let ext = FsPath::new(name).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let known = match ext.as_str() {
        "txt" => "Text",
        "md" => "Markdown",
        "rs" => "Rust",
        "toml" => "TOML",
        "json" => "JSON data",
        "csv" => "CSV table",
        "html" | "htm" => "Web page",
        "pdf" => "PDF",
        "png" => "PNG image",
        "jpg" | "jpeg" => "JPEG image",
        "gif" => "GIF image",
        "tif" | "tiff" => "TIFF image",
        "svg" => "SVG picture",
        "zip" => "Zip archive",
        "exe" => "Program",
        "mp3" | "wav" | "flac" => "Sound",
        "mp4" | "mov" => "Video",
        "" => "File",
        _ => return format!("{} file", ext.to_uppercase()),
    };
    known.into()
}

fn size(bytes: u64) -> String {
    match bytes {
        0..1_000 => format!("{bytes} bytes"),
        1_000..1_000_000 => format!("{:.1} kB", bytes as f64 / 1e3),
        1_000_000..1_000_000_000 => format!("{:.1} MB", bytes as f64 / 1e6),
        _ => format!("{:.1} GB", bytes as f64 / 1e9),
    }
}

fn named(path: &FsPath) -> String {
    path.file_name().map_or_else(|| path.to_string_lossy().into_owned(), |n| n.to_string_lossy().into_owned())
}

fn free(to: &FsPath, name: &str) -> Result<PathBuf, Failure> {
    let target = to.join(name);
    match target.exists() {
        true => Err(Failure::new(format!("There is already a “{name}” in {}", to.display()))),
        false => Ok(target),
    }
}

fn copy_into(from: &FsPath, to: &FsPath) -> Result<String, Failure> {
    let name = named(from);
    let target = free(to, &name)?;
    copy_all(from, &target).map_err(|e| Failure::new(format!("“{name}” couldn't be copied: {e}")))?;
    Ok(format!("Copied “{name}” to {}", to.display()))
}

fn copy_all(from: &FsPath, to: &FsPath) -> std::io::Result<()> {
    if from.is_dir() {
        std::fs::create_dir(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            copy_all(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(from, to).map(|_| ())
    }
}

fn move_into(from: &FsPath, to: &FsPath) -> Result<String, Failure> {
    let name = named(from);
    let target = free(to, &name)?;
    if std::fs::rename(from, &target).is_err() {
        copy_all(from, &target).map_err(|e| Failure::new(format!("“{name}” couldn't be moved: {e}")))?;
        let removed = if from.is_dir() { std::fs::remove_dir_all(from) } else { std::fs::remove_file(from) };
        removed.map_err(|e| Failure::new(format!("“{name}” was copied but the original couldn't be removed: {e}")))?;
    }
    Ok(format!("Moved “{name}” to {}", to.display()))
}

fn rename(from: &FsPath, name: &str) -> Result<String, Failure> {
    let old = named(from);
    let target = free(from.parent().unwrap_or(FsPath::new("")), name)?;
    std::fs::rename(from, &target).map_err(|e| Failure::new(format!("“{old}” couldn't be renamed: {e}")))?;
    Ok(format!("Renamed “{old}” to “{name}”"))
}

fn new_folder(at: &FsPath) -> Result<String, Failure> {
    let name = named(at);
    std::fs::create_dir(at).map_err(|e| Failure::new(format!("“{name}” couldn't be made: {e}")))?;
    Ok(format!("Made the folder “{name}”"))
}

impl ledger::Find<Entry> for Twin {
    fn find(&self, key: Key<Entry>) -> Option<&Entry> {
        self.locate(key).map(|(_, e)| e)
    }
}

fn main() {
    graticule::run::<Twin>(t!("Twin"));
}
