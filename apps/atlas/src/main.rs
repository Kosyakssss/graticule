mod mvt;
mod pmtiles;
mod world;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use graticule::kernel::canvas::Weight;
use graticule::kernel::{Camera, Drawing};
use graticule::prelude::*;
use mvt::{Geometry, Layer};
use pmtiles::{Archive, Tile};

#[derive(Facts, Default)]
struct Atlas {
    pins: List<Pin>,
    #[session]
    camera: Camera,
    #[session]
    frame: Frame,
    #[session]
    source: Source,
    #[session]
    file: Text,
    #[session]
    pinned: Selection<Pin>,
    #[derived(Map::open, from = [source])]
    map: Derived<Map>,
    #[derived(Sheet::of, from = [camera, frame, map, sheet])]
    sheet: Derived<Sheet>,
}

/// A place the user keeps, to go back to.
#[derive(Facts)]
struct Pin {
    #[key]
    id: Key<Pin>,
    name: String,
    x: f64,
    y: f64,
    level: u8,
}

/// Which map is shown: the sample, or a PMTiles file.
#[derive(Facts, Debug, Default, PartialEq)]
enum Source {
    #[default]
    Sample,
    File(String),
}

/// What the camera's page units are: points at a zoom level, counted from a point of the world at that level, the anchor. A camera keeps its zoom near 1 and its page near the anchor, so twenty levels of zoom stay as precise at the deepest as at the shallowest.
#[derive(Facts, Copy, Debug, PartialEq)]
struct Frame {
    level: u8,
    ax: f64,
    ay: f64,
}

impl Default for Frame {
    fn default() -> Self {
        let (x, y) = world::mercator(-45.0, 35.15);
        let size = world_size(7);
        Frame { level: 7, ax: x * size - VIEW.0 / 2.0, ay: y * size - VIEW.1 / 2.0 }
    }
}

/// How many points a tile takes across at its own level, and the deepest level.
const TILE: f64 = 512.0;
const DEEPEST: u8 = 20;

/// The size of a usual map canvas, in points: what the keys zoom around and where a pin puts the middle of the map.
const VIEW: (f64, f64) = (700.0, 360.0);

/// The most a canvas shows, in points, so the tiles it might show are read before it shows them.
const SIGHT: (f64, f64) = (2_400.0, 1_500.0);

/// How far the arrow keys pan, in points.
const STEP: f64 = 160.0;

fn world_size(level: u8) -> f64 {
    TILE * f64::from(1u32 << level)
}

/// The map file opened off the main thread.
struct Map(Result<Arc<Archive>, String>);

impl Map {
    fn open(source: &Source) -> Map {
        Map(match source {
            Source::Sample => Archive::held(world::sample()),
            Source::File(path) => Archive::open(path),
        }
        .map(Arc::new))
    }
}

/// The map in sight drawn off the main thread: the tiles that cover it read and decoded, kept while they stay in sight, then drawn in the frame's page units with their places named where the names don't overlap.
#[derive(Clone)]
struct Sheet {
    frame: Frame,
    wanted: Vec<Tile>,
    tiles: HashMap<Tile, Arc<Vec<Layer>>>,
    drawing: Arc<Drawing>,
}

/// The order layers are drawn in, bottom first, and how wide a letter of a label is taken to be, in points.
const LAYERS: [&str; 6] = ["earth", "landcover", "landuse", "water", "buildings", "roads"];
const LETTER: f64 = 8.4;

impl Sheet {
    fn of(camera: &Camera, frame: &Frame, map: &Derived<Map>, last: &Derived<Sheet>) -> Sheet {
        let Some(Map(Ok(archive))) = map.get() else { return Sheet { frame: *frame, wanted: Vec::new(), tiles: HashMap::new(), drawing: Arc::new(Drawing::new()) } };
        let data = frame.level.clamp(archive.min_zoom, archive.max_zoom);
        let side = TILE * 2f64.powi(i32::from(frame.level) - i32::from(data));
        let [left, right, top, bottom] = camera.showing(SIGHT.0, SIGHT.1);
        let last_tile = f64::from((1u32 << data) - 1);
        let span = |a: f64, b: f64| (a / side).floor().clamp(0.0, last_tile) as u32..=(b / side).floor().clamp(0.0, last_tile) as u32;
        let wanted: Vec<Tile> = span(frame.ax + left, frame.ax + right).flat_map(|x| span(frame.ay + top, frame.ay + bottom).map(move |y| Tile { z: data, x, y })).collect();
        let last = last.get();
        if let Some(last) = last.filter(|l| l.frame == *frame && l.wanted == wanted) {
            return last.clone();
        }
        let tiles: HashMap<Tile, Arc<Vec<Layer>>> = wanted
            .iter()
            .map(|&t| {
                let known = last.and_then(|l| l.tiles.get(&t).cloned());
                (t, known.unwrap_or_else(|| Arc::new(archive.tile(t).map(|b| mvt::decode(&b)).unwrap_or_default())))
            })
            .collect();
        let drawing = draw(frame, side, &wanted, &tiles, camera.zoom);
        Sheet { frame: *frame, wanted, tiles, drawing: Arc::new(drawing) }
    }
}

fn draw(frame: &Frame, side: f64, wanted: &[Tile], tiles: &HashMap<Tile, Arc<Vec<Layer>>>, zoom: f64) -> Drawing {
    let mut d = Drawing::new();
    let corner = |t: Tile| (f64::from(t.x) * side - frame.ax, f64::from(t.y) * side - frame.ay);
    for &t in wanted {
        let (x, y) = corner(t);
        d.rect((x as f32, y as f32), ((x + side) as f32, (y + side) as f32)).ink(Ink::Selection);
    }
    let mut places = Vec::new();
    for name in LAYERS.iter().chain(["boundaries", "places"].iter()) {
        for &t in wanted {
            let Some(layer) = tiles.get(&t).and_then(|ls| ls.iter().find(|l| l.name == *name)) else { continue };
            let (ox, oy) = corner(t);
            let k = side / f64::from(layer.extent.max(1));
            let at = |(px, py): (i32, i32)| ((ox + f64::from(px) * k) as f32, (oy + f64::from(py) * k) as f32);
            for feature in &layer.features {
                match &feature.geometry {
                    Geometry::Areas(rings) => {
                        let ink = match (*name, feature.kind.as_str()) {
                            ("earth", _) => Ink::Background,
                            ("water", _) => Ink::Selection,
                            ("buildings", _) => Ink::Field,
                            _ => Ink::Hover,
                        };
                        d.path(|pen| {
                            for ring in rings {
                                let Some((&first, rest)) = ring.split_first() else { continue };
                                pen.move_to(at(first));
                                for &p in rest {
                                    pen.line_to(at(p));
                                }
                                pen.close();
                            }
                        })
                        .fill()
                        .ink(ink);
                    }
                    Geometry::Lines(lines) => {
                        let (ink, weight) = match (*name, feature.kind.as_str()) {
                            ("roads", "highway") => (Ink::Ink, Weight::Line),
                            ("roads", "major_road") => (Ink::Muted, Weight::Line),
                            ("roads", "medium_road") => (Ink::Muted, Weight::Hair),
                            ("boundaries", _) => (Ink::Rule, Weight::Hair),
                            ("water", _) => (Ink::Link, Weight::Hair),
                            _ => (Ink::Faint, Weight::Hair),
                        };
                        for line in lines {
                            d.polyline(line.iter().copied().map(at)).ink(ink).stroke(weight);
                        }
                    }
                    Geometry::Points(points) if *name == "places" => {
                        if let (Some(name), Some(&p)) = (&feature.name, points.first()) {
                            places.push((feature.rank, feature.kind == "locality", name.clone(), at(p)));
                        }
                    }
                    Geometry::Points(_) => {}
                }
            }
        }
    }
    places.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.2.cmp(&b.2)));
    let mut seen = HashSet::new();
    let mut taken: Vec<[f32; 4]> = Vec::new();
    let tall = (LETTER * 2.2 / zoom) as f32;
    for (_, town, name, (x, y)) in places {
        if !seen.insert(name.clone()) {
            continue;
        }
        let wide = (LETTER * name.chars().count() as f64 / zoom) as f32;
        let gap = (6.0 / zoom) as f32;
        let from = if town { x + gap } else { x - wide / 2.0 };
        let b = [from - gap, from + wide + gap, y - tall / 2.0, y + tall / 2.0];
        if taken.iter().any(|t| t[0] < b[1] && b[0] < t[1] && t[2] < b[3] && b[2] < t[3]) {
            continue;
        }
        taken.push(b);
        if town {
            d.dot((x, y)).ink(Ink::Ink);
        }
        d.text((from, y), &name).ink(if town { Ink::Ink } else { Ink::Muted });
    }
    d.stamp();
    d
}

#[derive(Commands)]
enum Cmd {
    #[label("Zoom in")]
    ZoomIn,
    #[label("Zoom out")]
    ZoomOut,
    #[label("Pan left")]
    PanLeft,
    #[label("Pan right")]
    PanRight,
    #[label("Pan up")]
    PanUp,
    #[label("Pan down")]
    PanDown,
    #[label("Show the whole map")]
    Home,
    #[label("Pin this place…")]
    Pin { name: String },
    #[label("Go to “{name}”", short: "Go")]
    GoTo(#[subject] Key<Pin>),
    #[label("Remove “{name}”", short: "Remove", many: "Remove {count} pins")]
    Unpin(#[subject] Key<Pin>),
    #[label("Open the map")]
    Open,
    #[label("Show the sample map")]
    Sample,
    #[internal]
    Edit(Edit),
}

impl App for Atlas {
    type Cmd = Cmd;

    fn available(&self, cmd: &Cmd) -> Available {
        match cmd {
            Cmd::ZoomIn => no_if(self.frame.level == DEEPEST && self.camera.zoom >= 1.4, t!("The map goes no nearer")),
            Cmd::ZoomOut => no_if(self.frame.level == 0 && self.camera.zoom <= 0.5, t!("The whole world shows")),
            Cmd::Home => match self.map.get() {
                Some(Map(Ok(archive))) => no_if(self.there(world::mercator(archive.center.0, archive.center.1), archive.center.2.min(DEEPEST)), t!("The whole map shows already")),
                Some(Map(Err(_))) => No(t!("The map can't be read")),
                None => No(t!("The map is still opening")),
            },
            Cmd::GoTo(pin) => match self.pins.find(*pin) {
                Some(pin) => no_if(self.there((pin.x, pin.y), pin.level), t!("It shows there already")),
                None => No(t!("That pin is gone")),
            },
            Cmd::Unpin(pin) => no_if(self.pins.find(*pin).is_none(), t!("That pin is gone")),
            Cmd::Open => match self.file.is_empty() {
                true => No(t!("Choose a file first")),
                false => no_if(self.source == Source::File(self.file.to_string()), t!("That map is open")),
            },
            Cmd::Sample => no_if(self.source == Source::Sample, t!("The sample is showing")),
            Cmd::PanLeft | Cmd::PanRight | Cmd::PanUp | Cmd::PanDown | Cmd::Pin { .. } | Cmd::Edit(_) => Yes,
        }
    }

    fn update(&mut self, cmd: Cmd, cx: &mut Cx<Cmd>) -> Outcome {
        match cmd {
            Cmd::ZoomIn => self.look(self.camera.zoomed(2.0, (VIEW.0 / 2.0, VIEW.1 / 2.0))),
            Cmd::ZoomOut => self.look(self.camera.zoomed(0.5, (VIEW.0 / 2.0, VIEW.1 / 2.0))),
            Cmd::PanLeft => self.look(self.camera.panned(STEP, 0.0)),
            Cmd::PanRight => self.look(self.camera.panned(-STEP, 0.0)),
            Cmd::PanUp => self.look(self.camera.panned(0.0, STEP)),
            Cmd::PanDown => self.look(self.camera.panned(0.0, -STEP)),
            Cmd::Home => {
                let Some(Map(Ok(archive))) = self.map.get() else { return Outcome::Nothing };
                let (lon, lat, level) = archive.center;
                let level = level.min(DEEPEST);
                self.go(world::mercator(lon, lat), level)
            }
            Cmd::Pin { name } => {
                let name = name.trim();
                if name.is_empty() {
                    return cx.say(t!("A pin needs a name"));
                }
                let (x, y) = self.middle();
                let id = cx.key();
                let _ = self.pins.push(Pin { id, name: name.to_string(), x, y, level: self.frame.level });
                self.pinned.choose(&ledger::Choose::Only(id.raw()))
            }
            Cmd::GoTo(pin) => {
                let Some(pin) = self.pins.find(pin) else { return Outcome::Nothing };
                let (at, level) = ((pin.x, pin.y), pin.level);
                self.go(at, level)
            }
            Cmd::Unpin(pin) => self.pins.remove(pin),
            Cmd::Open => {
                self.source = Source::File(self.file.to_string());
                Outcome::Changed
            }
            Cmd::Sample => {
                self.source = Source::Sample;
                Outcome::Changed
            }
            Cmd::Edit(edit) => {
                let moved = edit.touches(Atlas::camera);
                let outcome = edit.apply(self);
                if moved {
                    self.settle();
                }
                outcome
            }
        }
    }

    fn view(&self, ui: &mut Ui<Cmd>) {
        ui.page(t!("Atlas"), |ui| {
            ui.row(|ui| {
                ui.button(Cmd::ZoomIn);
                ui.button(Cmd::ZoomOut);
                ui.button(Cmd::Home);
                ui.button(Cmd::Pin { name: String::new() });
            });
            ui.split(|ui| {
                ui.pane(t!("Map"), Fill, |ui| {
                    let (lon, lat) = world::degrees(self.middle());
                    let (ns, ew) = (if lat < 0.0 { t!("S") } else { t!("N") }, if lon < 0.0 { t!("W") } else { t!("E") });
                    let (lat, lon, level) = (format!("{:.3}", lat.abs()), format!("{:.3}", lon.abs()), self.frame.level);
                    ui.canvas(t!("Map at {lat}° {ns}, {lon}° {ew}, level {level}"), 0, |canvas| {
                        match self.sheet.get() {
                            Some(sheet) if sheet.frame != self.frame => {
                                canvas.look(reframed(self.camera, self.frame, sheet.frame));
                                canvas.drawing(&sheet.drawing);
                            }
                            sheet => {
                                canvas.camera(Atlas::camera);
                                canvas.pans();
                                if let Some(sheet) = sheet {
                                    canvas.drawing(&sheet.drawing);
                                }
                            }
                        }
                        let size = world_size(self.frame.level);
                        for pin in self.pins.iter() {
                            let at = ((pin.x * size - self.frame.ax) as f32, (pin.y * size - self.frame.ay) as f32);
                            canvas.dot(at).ink(Ink::Danger);
                            canvas.text((at.0 + 8.0 / self.camera.zoom as f32, at.1), &pin.name).ink(Ink::Danger);
                        }
                    });
                    match self.map.get() {
                        Some(Map(Err(why))) => ui.text(t!("Atlas can't read this map: {why}")),
                        None => ui.muted(t!("Opening the map")),
                        Some(Map(Ok(_))) if self.source == Source::Sample => ui.muted(t!("A made-up archipelago, drawn from vector tiles made when Atlas starts")),
                        Some(Map(Ok(_))) => ui.muted(t!("Map data © OpenStreetMap contributors")),
                    }
                });
                ui.pane(t!("Pins"), 24, |ui| {
                    if self.pins.is_empty() {
                        ui.empty(t!("No pins yet"), Cmd::Pin { name: String::new() });
                    }
                    ui.list(&self.pins, Atlas::pinned, |ui, pin| {
                        ui.text(&pin.name);
                        ui.link(Cmd::GoTo(pin.id));
                        ui.link(Cmd::Unpin(pin.id));
                    });
                });
            });
            ui.row(|ui| {
                ui.file_field(t!("Map file"), Atlas::file);
                ui.button(Cmd::Open);
                ui.button(Cmd::Sample);
            });
        });
    }

    fn states() -> States<Self> {
        let town = world::town("Port Ellery").unwrap_or_default();
        States::new()
            .add("The archipelago", Atlas::default())
            .add("A town up close", Atlas::at(town, 15))
            .add("Far out", Atlas::at(world::mercator(-45.0, 35.0), 5))
            .add("Pins", {
                let mut atlas = Atlas::default();
                for (n, (name, lon, lat, level)) in [("Harbour walk", -44.9, 35.1, 12), ("Lighthouse", -45.3, 35.4, 11)].into_iter().enumerate() {
                    let (x, y) = world::mercator(lon, lat);
                    let _ = atlas.pins.push(Pin { id: Key::fixed(n as u64 + 1), name: name.to_string(), x, y, level });
                }
                atlas
            })
            .add("A file that isn't a map", {
                let mut atlas = Atlas::default();
                let _ = atlas.file.set("missing.pmtiles");
                atlas.source = Source::File("missing.pmtiles".into());
                atlas
            })
    }
}

impl Atlas {
    fn at(point: (f64, f64), level: u8) -> Atlas {
        let mut atlas = Atlas::default();
        let _ = atlas.go(point, level);
        atlas
    }

    /// The point of the world in the middle of a usual canvas, in Web Mercator units.
    fn middle(&self) -> (f64, f64) {
        let size = world_size(self.frame.level);
        let (x, y) = self.camera.to_page((VIEW.0 / 2.0, VIEW.1 / 2.0));
        ((self.frame.ax + x) / size, (self.frame.ay + y) / size)
    }

    /// Shows `point` in the middle of the canvas, at `level`.
    fn go(&mut self, point: (f64, f64), level: u8) -> Outcome {
        let (frame, camera) = looking(point, level);
        if (frame, camera) == (self.frame, self.camera) {
            return Outcome::Nothing;
        }
        (self.frame, self.camera) = (frame, camera);
        Outcome::Changed
    }

    /// Whether `point` shows in the middle at `level` already.
    fn there(&self, point: (f64, f64), level: u8) -> bool {
        looking(point, level) == (self.frame, self.camera)
    }

    fn look(&mut self, camera: Camera) -> Outcome {
        if camera == self.camera {
            return Outcome::Nothing;
        }
        self.camera = camera;
        self.settle();
        Outcome::Changed
    }

    /// Moves to the level nearest the camera's zoom, and the anchor to the middle when the camera has gone far from it, keeping what shows where it is.
    fn settle(&mut self) {
        let (c, f) = (self.camera, self.frame);
        let level = (f64::from(f.level) + c.zoom.log2().round()).clamp(0.0, f64::from(DEEPEST)) as u8;
        let scale = 2f64.powi(i32::from(level) - i32::from(f.level));
        let least = if level == 0 { 0.5 } else { 0.0 };
        let most = if level == DEEPEST { 1.4 } else { f64::MAX };
        let zoom = (c.zoom / scale).clamp(least, most);
        let far = c.x.abs().max(c.y.abs()) > 16.0 * TILE;
        if level == f.level && zoom == c.zoom && !far {
            return;
        }
        let (mx, my) = c.to_page((VIEW.0 / 2.0, VIEW.1 / 2.0));
        let (mx, my) = ((f.ax + mx) * scale, (f.ay + my) * scale);
        self.frame = Frame { level, ax: mx.round(), ay: my.round() };
        self.camera = Camera { x: mx - mx.round() - VIEW.0 / 2.0 / zoom, y: my - my.round() - VIEW.1 / 2.0 / zoom, zoom };
    }
}

/// The frame and camera that show `point` in the middle of a usual canvas at `level`.
fn looking((x, y): (f64, f64), level: u8) -> (Frame, Camera) {
    let size = world_size(level);
    (Frame { level, ax: x * size - VIEW.0 / 2.0, ay: y * size - VIEW.1 / 2.0 }, Camera::default())
}

/// The camera that shows through `to`'s page units what `camera` shows through `from`'s, for a sheet drawn before the frame last moved.
fn reframed(camera: Camera, from: Frame, to: Frame) -> Camera {
    let scale = 2f64.powi(i32::from(to.level) - i32::from(from.level));
    Camera { x: (from.ax + camera.x) * scale - to.ax, y: (from.ay + camera.y) * scale - to.ay, zoom: camera.zoom / scale }
}

fn main() {
    graticule::run::<Atlas>(t!("Atlas"));
}
