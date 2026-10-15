mod plate;
mod scan;

use std::sync::{Arc, OnceLock};

use graticule::kernel::canvas::Weight;
use graticule::kernel::{Camera, Drag, PaintHook};
use graticule::prelude::*;
use plate::{Plate, Shown};
use scan::{Histogram, Scan};

#[derive(Facts)]
struct Lightbox {
    measures: List<Measure>,
    #[session]
    source: Source,
    #[session]
    file: Text,
    #[session]
    camera: Camera,
    #[session]
    tool: Tool,
    #[session]
    levels: (u16, u16),
    #[session]
    gamma: u16,
    #[session]
    shown: Shown,
    #[session]
    chosen: Selection<Measure>,
    #[derived(Loaded::open, from = [source])]
    scan: Derived<Loaded>,
    #[derived(Histogram::of_loaded, from = [scan])]
    histogram: Derived<Histogram>,
}

impl Default for Lightbox {
    fn default() -> Self {
        Lightbox {
            measures: List::default(),
            source: Source::default(),
            file: Text::default(),
            camera: Camera::WHOLE,
            tool: Tool::default(),
            levels: (0, u16::MAX),
            gamma: 100,
            shown: Shown::default(),
            chosen: Selection::default(),
            scan: Derived::default(),
            histogram: Derived::default(),
        }
    }
}

/// A distance measured on the scan, from one pixel to another.
#[derive(Facts)]
struct Measure {
    #[key]
    id: Key<Measure>,
    from: (f64, f64),
    to: (f64, f64),
}

/// Which scan is shown: the sample, or a file.
#[derive(Facts, Debug, Default, PartialEq)]
enum Source {
    #[default]
    Sample,
    File(String),
}

/// What a drag on the scan does.
#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
enum Tool {
    #[default]
    Move,
    Measure,
}

/// The scan read off the main thread, or why it couldn't be.
struct Loaded(Result<Arc<Scan>, String>);

/// The sample, made once however many times it is shown.
static SAMPLE: OnceLock<Arc<Scan>> = OnceLock::new();

impl Loaded {
    fn open(source: &Source) -> Loaded {
        Loaded(match source {
            Source::Sample => Ok(SAMPLE.get_or_init(|| Arc::new(Scan::sample())).clone()),
            Source::File(path) => std::fs::read(path).map_err(|e| e.to_string()).and_then(|bytes| Scan::read(&bytes)).map(Arc::new),
        })
    }

    fn scan(&self) -> Option<&Arc<Scan>> {
        self.0.as_ref().ok()
    }
}

impl Histogram {
    fn of_loaded(scan: &Derived<Loaded>) -> Histogram {
        scan.get().and_then(Loaded::scan).map_or(Histogram(vec![0; 256]), |s| Histogram::of(s))
    }
}

/// The size of a usual scan canvas, in points: what the keys zoom around when the scan shows whole.
const VIEW: (f64, f64) = (640.0, 400.0);

/// How much of the darkest and lightest pixels stretching the levels leaves out.
const CUT: f64 = 0.005;

#[derive(Commands)]
enum Cmd {
    #[label("Move")]
    Move,
    #[label("Measure")]
    Measure,
    #[label("Zoom in")]
    ZoomIn,
    #[label("Zoom out")]
    ZoomOut,
    #[label("Fit")]
    Fit,
    #[label("Actual pixels")]
    ActualPixels,
    #[label("Stretch the levels")]
    Stretch,
    #[label("Reset the levels")]
    ResetLevels,
    #[label("Remove the measurement", short: "Remove", many: "Remove {count} measurements")]
    Remove(#[subject] Key<Measure>),
    #[label("Open the scan")]
    Open,
    #[label("Show the sample scan")]
    Sample,
    #[internal]
    Measured(Drag),
    #[internal]
    Edit(Edit),
}

impl App for Lightbox {
    type Cmd = Cmd;

    fn available(&self, cmd: &Cmd) -> Available {
        let loaded = self.scan.get().and_then(Loaded::scan).is_some();
        match cmd {
            Cmd::Move => no_if(self.tool == Tool::Move, t!("Moving already")),
            Cmd::Measure => no_if(self.tool == Tool::Measure, t!("Measuring already")),
            _ if !loaded && matches!(cmd, Cmd::ZoomIn | Cmd::ZoomOut | Cmd::Fit | Cmd::ActualPixels | Cmd::Stretch) => No(t!("No scan is open")),
            Cmd::ZoomIn => no_if(self.camera.zoom >= Camera::ZOOMS.1, t!("As near as it goes")),
            Cmd::ZoomOut => no_if(self.camera != Camera::WHOLE && self.camera.zoom <= Camera::ZOOMS.0, t!("As far as it goes")),
            Cmd::Fit => no_if(self.camera == Camera::WHOLE, t!("The whole scan shows already")),
            Cmd::ActualPixels => no_if(self.camera.zoom == 1.0, t!("Showing actual pixels already")),
            Cmd::Stretch => match self.histogram.get() {
                Some(h) => no_if(h.ends(CUT) == self.levels, t!("Stretched already")),
                None => No(t!("The levels are still being counted")),
            },
            Cmd::ResetLevels => no_if(self.levels == (0, u16::MAX) && self.gamma == 100, t!("The levels are as the file has them")),
            Cmd::Remove(m) => no_if(self.measures.find(*m).is_none(), t!("That measurement is gone")),
            Cmd::Open => match self.file.is_empty() {
                true => No(t!("Choose a file first")),
                false => no_if(self.source == Source::File(self.file.to_string()), t!("That scan is open")),
            },
            Cmd::Sample => no_if(self.source == Source::Sample, t!("The sample is showing")),
            Cmd::Measured(_) | Cmd::Edit(_) => Yes,
        }
    }

    fn update(&mut self, cmd: Cmd, cx: &mut Cx<Cmd>) -> Outcome {
        match cmd {
            Cmd::Move => self.hold(Tool::Move),
            Cmd::Measure => self.hold(Tool::Measure),
            Cmd::ZoomIn => self.zoom(2.0),
            Cmd::ZoomOut => self.zoom(0.5),
            Cmd::Fit => {
                self.camera = Camera::WHOLE;
                Outcome::Changed
            }
            Cmd::ActualPixels => {
                let middle = self.seen().to_page((VIEW.0 / 2.0, VIEW.1 / 2.0));
                self.camera = Camera { x: middle.0 - VIEW.0 / 2.0, y: middle.1 - VIEW.1 / 2.0, zoom: 1.0 };
                Outcome::Changed
            }
            Cmd::Stretch => {
                let Some(ends) = self.histogram.get().map(|h| h.ends(CUT)) else { return Outcome::Nothing };
                self.levels = ends;
                Outcome::Changed
            }
            Cmd::ResetLevels => {
                (self.levels, self.gamma) = ((0, u16::MAX), 100);
                Outcome::Changed
            }
            Cmd::Remove(m) => self.measures.remove(m),
            Cmd::Open => self.show(Source::File(self.file.to_string())),
            Cmd::Sample => self.show(Source::Sample),
            Cmd::Measured(drag) => {
                if !drag.moved {
                    return Outcome::Nothing;
                }
                let id = cx.key();
                let _ = self.measures.push(Measure { id, from: drag.from, to: drag.to });
                self.chosen.choose(&ledger::Choose::Only(id.raw()))
            }
            Cmd::Edit(edit) => edit.apply(self),
        }
    }

    fn view(&self, ui: &mut Ui<Cmd>) {
        ui.page(t!("Lightbox"), |ui| {
            ui.row(|ui| {
                ui.group(|ui| {
                    ui.toggle(self.tool == Tool::Move, Cmd::Move);
                    ui.toggle(self.tool == Tool::Measure, Cmd::Measure);
                });
                ui.button(Cmd::ZoomIn);
                ui.button(Cmd::ZoomOut);
                ui.button(Cmd::Fit);
                ui.button(Cmd::ActualPixels);
            });
            let scan = self.scan.get().and_then(Loaded::scan);
            ui.split(|ui| {
                ui.pane(t!("Scan"), Fill, |ui| {
                    let label = match scan {
                        Some(s) => {
                            let (w, h) = (s.width, s.height);
                            t!("Scan, {w} by {h} pixels")
                        }
                        None => t!("No scan"),
                    };
                    ui.canvas(label, 0, |canvas| {
                        if let Some(s) = scan {
                            canvas.fits([0.0, f64::from(s.width), 0.0, f64::from(s.height)]);
                        }
                        canvas.camera(Lightbox::camera);
                        match self.tool {
                            Tool::Move => canvas.pans(),
                            Tool::Measure => canvas.on_drag(Cmd::Measured),
                        }
                        let camera = canvas.looking().unwrap_or_default();
                        if let Some(s) = scan {
                            let width = canvas.size().map_or(0.0, |(w, _)| w);
                            let plate = Plate { scan: s.clone(), camera, levels: self.levels, gamma: self.gamma, shown: self.shown, width };
                            canvas.paint(&PaintHook::new(plate));
                        }
                        let dragging = canvas.drag().filter(|d| d.moved && self.tool == Tool::Measure).cloned();
                        let tick = (6.0 / camera.zoom.max(1e-6)) as f32;
                        let mut ruler = |from: (f64, f64), to: (f64, f64), chosen: bool| {
                            let (a, b) = ((from.0 as f32, from.1 as f32), (to.0 as f32, to.1 as f32));
                            let ink = if chosen { Ink::Link } else { Ink::Warn };
                            canvas.line(a, b).ink(ink).stroke(Weight::Bold);
                            for end in [a, b] {
                                canvas.line((end.0 - tick, end.1), (end.0 + tick, end.1)).ink(ink).stroke(Weight::Line);
                                canvas.line((end.0, end.1 - tick), (end.0, end.1 + tick)).ink(ink).stroke(Weight::Line);
                            }
                            let middle = ((a.0 + b.0) / 2.0 + tick, (a.1 + b.1) / 2.0 - tick);
                            canvas.text(middle, length(from, to, scan.and_then(|s| s.dpi)).as_str()).ink(ink).backed();
                        };
                        for m in self.measures.iter() {
                            ruler(m.from, m.to, self.chosen.contains(m.id));
                        }
                        if let Some(drag) = dragging {
                            ruler(drag.from, drag.to, true);
                        }
                    });
                    match (self.scan.get(), scan) {
                        (Some(Loaded(Err(why))), _) => ui.text(t!("Lightbox can't read this scan: {why}")),
                        (_, Some(s)) => ui.muted(described(s)),
                        _ => ui.muted(t!("Opening the scan")),
                    }
                });
                ui.pane(t!("Levels"), 30, |ui| {
                    ui.canvas(t!("How many pixels are at each level"), 6, |canvas| {
                        let Some(h) = self.histogram.get() else { return };
                        let most = h.0.iter().copied().max().unwrap_or(1).max(1) as f32;
                        canvas.space(0.0..256.0, 1.0..0.0);
                        for (k, n) in h.0.iter().enumerate().filter(|(_, n)| **n > 0) {
                            let x = k as f32;
                            canvas.rect((x, 0.0), (x + 1.0, (*n as f32 / most).sqrt())).ink(Ink::Muted);
                        }
                        for end in [self.levels.0, self.levels.1] {
                            let x = f32::from(end) / 256.0;
                            canvas.line((x, 0.0), (x, 1.0)).ink(Ink::Link).stroke(Weight::Line);
                        }
                    });
                    ui.choice(t!("Show as"), Lightbox::shown);
                    ui.labelled(t!("Black and white"), |ui| ui.range_slider(Lightbox::levels, 0..=i64::from(u16::MAX)));
                    ui.labelled(t!("Gamma, in hundredths"), |ui| ui.slider(Lightbox::gamma, 20..=500));
                    ui.row(|ui| {
                        ui.button(Cmd::Stretch);
                        ui.button(Cmd::ResetLevels);
                    });
                    if self.measures.is_empty() {
                        ui.muted(t!("Choose Measure and drag across the scan to measure it"));
                    }
                    ui.list(&self.measures, Lightbox::chosen, |ui, m| {
                        ui.text(length(m.from, m.to, scan.and_then(|s| s.dpi)));
                        ui.link(Cmd::Remove(m.id));
                    });
                });
            });
            ui.row(|ui| {
                ui.file_field(t!("Scan file"), Lightbox::file);
                ui.button(Cmd::Open);
                ui.button(Cmd::Sample);
            });
        });
    }

    fn states() -> States<Self> {
        States::new()
            .add("The sample scan", Lightbox::default())
            .add("Levels pulled in on the faint rings", Lightbox { levels: (32_000, 33_600), camera: Camera { x: 1_700.0, y: 820.0, zoom: 0.9 }, ..Lightbox::default() })
            .add("The star's middle up close", Lightbox { camera: Camera { x: 1_447.0, y: 867.0, zoom: 6.0 }, ..Lightbox::default() })
            .add("A negative, measured", {
                let mut l = Lightbox { shown: Shown::Negative, tool: Tool::Measure, ..Lightbox::default() };
                let _ = l.measures.push(Measure { id: Key::fixed(1), from: (100.0, 300.0), to: (2_300.0, 300.0) });
                let _ = l.measures.push(Measure { id: Key::fixed(2), from: (1_500.0, 480.0), to: (1_500.0, 1_320.0) });
                let _ = l.chosen.choose(&ledger::Choose::Only(Key::<Measure>::fixed(2).raw()));
                l
            })
            .add("The step wedge in false color", Lightbox { shown: Shown::FalseColor, levels: (0, 40_000), camera: Camera { x: 60.0, y: 40.0, zoom: 0.55 }, ..Lightbox::default() })
            .add("A file that isn't a scan", {
                let mut l = Lightbox { source: Source::File("missing.png".into()), ..Lightbox::default() };
                let _ = l.file.set("missing.png");
                l
            })
    }
}

impl Lightbox {
    fn hold(&mut self, tool: Tool) -> Outcome {
        self.tool = tool;
        Outcome::Changed
    }

    fn show(&mut self, source: Source) -> Outcome {
        self.source = source;
        self.camera = Camera::WHOLE;
        let _ = self.measures.retain(|_| false);
        Outcome::Changed
    }

    /// The camera as a usual canvas shows it, the whole scan worked out for one when the camera fits it.
    fn seen(&self) -> Camera {
        match self.scan.get().and_then(Loaded::scan) {
            Some(s) if self.camera == Camera::WHOLE => Camera::fitting([0.0, f64::from(s.width), 0.0, f64::from(s.height)], VIEW),
            _ => self.camera,
        }
    }

    fn zoom(&mut self, by: f64) -> Outcome {
        let to = self.seen().zoomed(by, (VIEW.0 / 2.0, VIEW.1 / 2.0));
        if to == self.camera {
            return Outcome::Nothing;
        }
        self.camera = to;
        Outcome::Changed
    }
}

/// A measured distance in words: in millimetres when the scan says how many pixels it has to the inch, and in pixels.
fn length(from: (f64, f64), to: (f64, f64), dpi: Option<f64>) -> Message {
    let px = (to.0 - from.0).hypot(to.1 - from.1);
    let pixels = format!("{px:.1}");
    match dpi {
        Some(dpi) => {
            let mm = format!("{:.3}", px / dpi * 25.4);
            t!("{mm} mm, {pixels} px")
        }
        None => t!("{pixels} px"),
    }
}

/// What a scan is, in words.
fn described(s: &Scan) -> Message {
    let (w, h, depth) = (s.width, s.height, s.depth);
    let kind = if s.color { t!("color") } else { t!("gray") };
    match s.dpi {
        Some(dpi) => {
            let dpi = dpi.round() as u32;
            t!("{w} by {h} pixels, {depth}-bit {kind}, {dpi} pixels to the inch")
        }
        None => t!("{w} by {h} pixels, {depth}-bit {kind}"),
    }
}

fn main() {
    graticule::run::<Lightbox>(t!("Lightbox"));
}
