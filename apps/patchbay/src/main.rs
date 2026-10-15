use std::collections::HashMap;

use graticule::kernel::{Camera, Drag, Message};
use graticule::prelude::*;
use plotter::view::{Around, Layers, Sight};
use plotter::{Binding, Board, Bounds, End, Group, Index, Intent, Kind, LINE, Page, Point, Shape, Tool, text_width};

#[derive(Facts, Default)]
struct Patchbay {
    board: Board,
    #[session]
    chosen: Selection<Shape>,
    #[session]
    camera: Camera,
    #[session]
    file: Text,
    #[derived(indexed, from = [board])]
    index: Derived<Index>,
    #[derived(Layers::plain, from = [board, index, camera])]
    layers: Derived<Layers>,
    #[derived(Around::of, from = [board, chosen])]
    around: Derived<Around>,
    #[derived(Patch::read, from = [board])]
    patch: Derived<Patch>,
}

fn indexed(board: &Board) -> Index {
    Index::of(board, &Page::FIRST)
}

/// The kinds of module a patch is made of, each with the inputs on its left and the outputs on its right.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Module {
    Oscillator,
    Noise,
    Filter,
    Envelope,
    Amplifier,
    Output,
}

impl Module {
    fn name(self) -> Message {
        match self {
            Module::Oscillator => t!("Oscillator"),
            Module::Noise => t!("Noise"),
            Module::Filter => t!("Filter"),
            Module::Envelope => t!("Envelope"),
            Module::Amplifier => t!("Amplifier"),
            Module::Output => t!("Output"),
        }
    }

    fn inputs(self) -> Vec<Message> {
        match self {
            Module::Oscillator => vec![t!("pitch")],
            Module::Noise => Vec::new(),
            Module::Filter => vec![t!("in"), t!("cutoff")],
            Module::Envelope => vec![t!("gate")],
            Module::Amplifier => vec![t!("in"), t!("gain")],
            Module::Output => vec![t!("left"), t!("right")],
        }
    }

    fn outputs(self) -> Vec<Message> {
        match self {
            Module::Output => Vec::new(),
            _ => vec![t!("out")],
        }
    }
}

/// Which way a signal goes through a port.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Flow {
    In,
    Out,
}

/// The size of a module on the board, in page units: how wide, how far apart its ports are, how tall its title is, and how wide a port is.
const WIDE: f64 = 160.0;
const PITCH: f64 = 24.0;
const TITLE: f64 = 28.0;
const PORT: f64 = 12.0;

/// How close, in screen points, a press must be to a port to take it.
const REACH: f64 = 6.0;

/// The modules and wires a board holds, read off it in the background for the list of wires and for connecting by name. A module is a group holding a rectangle, its title above the other words, and an ellipse for each port with its name beside it; a port on the module's left edge takes a signal in and one on its right sends it out. A wire is an arrow held from an output to an input.
#[derive(Default)]
struct Patch {
    modules: usize,
    ports: Vec<(String, Key<Shape>)>,
    wires: Vec<(String, String)>,
}

impl Patch {
    fn read(board: &Board) -> Patch {
        let mut groups: HashMap<Key<Group>, Vec<&Shape>> = HashMap::new();
        for shape in board.shapes.iter() {
            if let Some(group) = shape.group {
                groups.entry(group).or_default().push(shape);
            }
        }
        let mut names = HashMap::new();
        for shapes in groups.values() {
            let mut words: Vec<(&Shape, &str)> = shapes.iter().filter_map(|s| if let Kind::Text(w) = &s.kind { Some((*s, w.as_str())) } else { None }).collect();
            words.sort_by(|a, b| a.0.y.total_cmp(&b.0.y));
            let Some(((_, title), labels)) = words.split_first() else { continue };
            for port in shapes.iter().filter(|s| s.kind == Kind::Ellipse) {
                let middle = port.bounds().center();
                let far = |s: &Shape| (s.bounds().center() - middle).length();
                if let Some((_, label)) = labels.iter().filter(|(s, _)| (s.bounds().center().y - middle.y).abs() < PITCH / 2.0).min_by(|a, b| far(a.0).total_cmp(&far(b.0))) {
                    names.insert(port.id, format!("{title} {label}"));
                }
            }
        }
        let mut ports: Vec<(String, Key<Shape>)> = names.iter().map(|(k, n)| (n.clone(), *k)).collect();
        ports.sort();
        let mut wires: Vec<(String, String)> = board
            .shapes
            .iter()
            .filter(|s| s.kind == Kind::Arrow)
            .filter_map(|arrow| {
                let end = |end: End| board.bindings.linked(Binding::arrow, arrow.id).find(|b| b.end == end).and_then(|b| names.get(&b.to)).cloned();
                Some((end(End::Start)?, end(End::End)?))
            })
            .collect();
        wires.sort();
        Patch { modules: groups.len(), ports, wires }
    }

    fn port(&self, name: &str) -> Option<Key<Shape>> {
        let name = name.trim();
        self.ports.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, k)| *k)
    }
}

#[derive(Commands)]
enum Cmd {
    #[label("Add an oscillator")]
    AddOscillator,
    #[label("Add noise")]
    AddNoise,
    #[label("Add a filter")]
    AddFilter,
    #[label("Add an envelope")]
    AddEnvelope,
    #[label("Add an amplifier")]
    AddAmplifier,
    #[label("Add an output")]
    AddOutput,
    #[label("Connect…")]
    Connect { wire: String },
    #[label("Remove")]
    Remove,
    #[label("Choose the {kind}", short: "Choose")]
    ChooseShape(#[subject] Key<Shape>),
    #[label("Show everything")]
    ShowAll,
    #[label("Open")]
    Open,
    #[internal]
    Opened(Result<String, Failure>),
    #[label("Save")]
    Save,
    #[internal]
    Saved(Result<Path, Failure>),
    #[internal]
    Drew(Drag),
    #[internal]
    Edit(Edit),
}

impl App for Patchbay {
    type Cmd = Cmd;

    fn available(&self, cmd: &Cmd) -> Available {
        match cmd {
            Cmd::Remove => no_if(self.chosen.is_empty(), t!("Choose a module or a wire first")),
            Cmd::ChooseShape(key) if !self.board.shapes.has(*key) => No(t!("That is gone")),
            Cmd::ChooseShape(key) => {
                let whole = self.board.whole(&[*key]);
                no_if(self.chosen.len() == whole.len() && whole.iter().all(|k| self.chosen.contains(*k)), t!("Chosen already"))
            }
            Cmd::Connect { .. } | Cmd::ShowAll => no_if(self.board.shapes.is_empty(), t!("The patch is empty")),
            Cmd::Open | Cmd::Save => no_if(self.file.is_empty(), t!("Choose a file first")),
            Cmd::AddOscillator | Cmd::AddNoise | Cmd::AddFilter | Cmd::AddEnvelope | Cmd::AddAmplifier | Cmd::AddOutput => Yes,
            Cmd::Opened(_) | Cmd::Saved(_) | Cmd::Drew(_) | Cmd::Edit(_) => Yes,
        }
    }

    fn update(&mut self, cmd: Cmd, cx: &mut Cx<Cmd>) -> Outcome {
        match cmd {
            Cmd::AddOscillator => self.add(Module::Oscillator, cx),
            Cmd::AddNoise => self.add(Module::Noise, cx),
            Cmd::AddFilter => self.add(Module::Filter, cx),
            Cmd::AddEnvelope => self.add(Module::Envelope, cx),
            Cmd::AddAmplifier => self.add(Module::Amplifier, cx),
            Cmd::AddOutput => self.add(Module::Output, cx),
            Cmd::Connect { wire } => {
                let Some(patch) = self.patch.get() else { return cx.say(t!("The patch is still being read")) };
                let Some((from, to)) = wire.split_once(" to ") else { return cx.say(t!("Say which output to which input, as “Oscillator out to Filter in”")) };
                let (Some(start), Some(end)) = (patch.port(from), patch.port(to)) else { return cx.say(t!("There is no port by that name")) };
                match wiring(&self.board, Some(start), Some(end)) {
                    Ok(()) => self.board.perform(&mut self.chosen, cable(&self.board, start, end), || cx.key::<Shape>().raw()),
                    Err(why) => cx.say(why),
                }
            }
            Cmd::Remove => {
                let keys = self.board.contents(&self.chosen.iter().collect::<Vec<_>>());
                let mut gone = keys.clone();
                gone.extend(self.board.touching(&keys).into_iter().filter_map(|b| self.board.bindings.find(b)).map(|b| b.arrow));
                let _ = self.chosen.choose(&ledger::Choose::Clear);
                self.board.delete(&gone)
            }
            Cmd::ChooseShape(key) => {
                let whole = self.board.whole(&[key]);
                self.chosen.choose(&ledger::Choose::All(whole.into_iter().map(Key::raw).collect()))
            }
            Cmd::ShowAll => {
                let Some(camera) = self.sight().index().bounds().map(everything) else { return Outcome::Nothing };
                if camera == self.camera {
                    return cx.say(t!("Everything shows already"));
                }
                self.camera = camera;
                Outcome::Changed
            }
            Cmd::Open => {
                let file = self.file.to_string();
                cx.job(t!("Opening {file}"), move || std::fs::read_to_string(&file).map_err(|e| Failure::new(e.to_string())), Cmd::Opened);
                Outcome::Nothing
            }
            Cmd::Opened(Ok(svg)) => match Board::from_svg(&svg) {
                Some(board) => {
                    let _ = self.chosen.choose(&ledger::Choose::Clear);
                    self.board.replace(&board)
                }
                None => cx.say(t!("That file holds no patch")),
            },
            Cmd::Opened(Err(failure)) | Cmd::Saved(Err(failure)) => cx.say_failure(failure),
            Cmd::Save => {
                cx.write(Path(self.file.to_string().into()), |facts: &Patchbay| plotter::svg(&facts.board, Page::FIRST), Cmd::Saved);
                Outcome::Nothing
            }
            Cmd::Saved(Ok(path)) => cx.say(t!("Saved to {path}")),
            Cmd::Drew(drag) => match self.plan(&drag) {
                (intent, Ok(())) => self.board.perform(&mut self.chosen, intent, || cx.key::<Shape>().raw()),
                (_, Err(why)) => cx.say(why),
            },
            Cmd::Edit(edit) => edit.apply(self),
        }
    }

    fn view(&self, ui: &mut Ui<Cmd>) {
        ui.page(t!("Patchbay"), |ui| {
            ui.row(|ui| {
                ui.menu(t!("Add"), |ui| {
                    ui.button(Cmd::AddOscillator);
                    ui.button(Cmd::AddNoise);
                    ui.button(Cmd::AddFilter);
                    ui.button(Cmd::AddEnvelope);
                    ui.button(Cmd::AddAmplifier);
                    ui.button(Cmd::AddOutput);
                });
                ui.button(Cmd::Connect { wire: String::new() });
                ui.button(Cmd::Remove);
                ui.button(Cmd::ShowAll);
            });
            let patch = self.patch.get();
            let modules = patch.map_or(0, |p| p.modules);
            ui.split(|ui| {
                ui.pane(t!("Patch"), Fill, |ui| {
                    ui.canvas(t!("Patch, {modules} modules"), 0, |canvas| {
                        canvas.camera(Patchbay::camera);
                        canvas.on_drag(Cmd::Drew);
                        let intent = canvas.drag().map(|drag| match self.plan(drag) {
                            (Intent::Create { mut shape, start, end, holds }, Err(_)) => {
                                shape.ink = plotter::Ink::Danger;
                                Intent::Create { shape, start, end, holds }
                            }
                            (intent, _) => intent,
                        });
                        self.sight().show(canvas, intent, Vec::new(), Cmd::ChooseShape);
                    });
                });
                ui.pane(t!("Wires"), 24, |ui| match patch {
                    Some(patch) if !patch.wires.is_empty() => {
                        for (from, to) in &patch.wires {
                            ui.text(t!("{from} to {to}"));
                        }
                    }
                    _ => ui.muted(t!("Drag from an output on a module's right to an input on another's left")),
                });
            });
            ui.row(|ui| {
                ui.file_field(t!("File"), Patchbay::file);
                ui.button(Cmd::Open);
                ui.button(Cmd::Save);
            });
        });
    }

    fn states() -> States<Self> {
        States::new()
            .add("Empty", Patchbay::default())
            .add("A patch", Patchbay::voice())
            .add("A module chosen", {
                let mut p = Patchbay::voice();
                let first = p.board.stacked()[0].id;
                let whole = p.board.whole(&[first]);
                let _ = p.chosen.choose(&ledger::Choose::All(whole.into_iter().map(Key::raw).collect()));
                p
            })
            .add("Modules not yet wired", {
                let mut p = Patchbay::default();
                let mut n = 0;
                let mut next = || {
                    n += 1;
                    Key::<Shape>::fixed(n).raw()
                };
                for (i, module) in [Module::Noise, Module::Filter, Module::Output].into_iter().enumerate() {
                    let _ = p.build(module, Point::new(40.0 + 220.0 * i as f64, 40.0), &mut next);
                }
                p
            })
    }
}

impl Patchbay {
    /// What the patch canvas shows.
    fn sight(&self) -> Sight<'_> {
        Sight {
            board: &self.board,
            page: Page::FIRST,
            index: &self.index,
            layers: &self.layers,
            around: &self.around,
            pictures: None,
            chosen: &self.chosen,
            tool: Tool::Select,
            zoom: self.camera.zoom,
            resizing: false,
        }
    }

    /// What letting go of a drag does, and why it can't when it can't: a drag from a port draws a wire, which must run from an output to a free input on another module, and any other drag chooses or moves modules, which keep their size.
    fn plan(&self, drag: &Drag) -> (Intent, Result<(), Message>) {
        let sight = self.sight();
        let index = sight.index();
        let from = Point::new(drag.from.0, drag.from.1);
        let reach = REACH / self.camera.zoom;
        let near = Bounds::new(from.x - reach, from.y - reach, reach * 2.0, reach * 2.0);
        let on_port = index.query(near).into_iter().filter_map(|e| self.board.shapes.find(e.key)).any(|s| s.geometry().hit(from, reach) && port(&self.board, s.id).is_some());
        if drag.moved && on_port {
            let intent = Sight { tool: Tool::Arrow, ..sight }.intent(drag);
            let checked = match &intent {
                Intent::Create { start, end, .. } => wiring(&self.board, *start, *end),
                _ => Ok(()),
            };
            return (intent, checked);
        }
        match sight.intent(drag) {
            Intent::Resize { .. } => {
                let none = Selection::new();
                (Sight { chosen: &none, ..self.sight() }.intent(drag), Ok(()))
            }
            intent => (intent, Ok(())),
        }
    }

    fn add(&mut self, module: Module, cx: &mut Cx<Cmd>) -> Outcome {
        let n = (self.board.groups.len() % 8) as f64;
        let at = Point::new(self.camera.x + 40.0 + n * PITCH, self.camera.y + 40.0 + n * PITCH);
        let made = self.build(module, at, || cx.key::<Shape>().raw());
        let whole = self.board.whole(&made.iter().map(|(_, k)| *k).take(1).collect::<Vec<_>>());
        self.chosen.choose(&ledger::Choose::All(whole.into_iter().map(Key::raw).collect()))
    }

    /// Puts a module on the board with its corner at `at`, as one group, and gives back its ports.
    fn build(&mut self, module: Module, at: Point, mut next: impl FnMut() -> u128) -> Vec<(Flow, Key<Shape>)> {
        let (inputs, outputs) = (module.inputs(), module.outputs());
        let rows = inputs.len().max(outputs.len()) as f64;
        let body = Bounds::new(at.x, at.y, WIDE, TITLE + rows * PITCH + 8.0);
        let mut keys = Vec::new();
        let mut put = |board: &mut Board, kind: Kind, b: Bounds, ink: plotter::Ink| {
            let key = Key::from_raw(next());
            let _ = board.add(Shape { ink, ..Shape::new(key, kind, b) });
            keys.push(key);
            key
        };
        let _ = put(&mut self.board, Kind::Rect, body, plotter::Ink::Ink);
        let title = module.name().to_string();
        let _ = put(&mut self.board, Kind::Text(title.clone()), Bounds::new(at.x + 12.0, at.y + 6.0, text_width(&title), LINE), plotter::Ink::Ink);
        let mut ports = Vec::new();
        for (flow, names) in [(Flow::In, inputs), (Flow::Out, outputs)] {
            for (i, name) in names.into_iter().enumerate() {
                let name = name.to_string();
                let middle = at.y + TITLE + PITCH * (i as f64 + 0.5);
                let (edge, label) = match flow {
                    Flow::In => (at.x, at.x + PORT + 4.0),
                    Flow::Out => (at.x + WIDE, at.x + WIDE - PORT - 4.0 - text_width(&name)),
                };
                let _ = put(&mut self.board, Kind::Text(name.clone()), Bounds::new(label, middle - LINE / 2.0, text_width(&name), LINE), plotter::Ink::Muted);
                let ink = if flow == Flow::In { plotter::Ink::Link } else { plotter::Ink::Ok };
                ports.push((flow, put(&mut self.board, Kind::Ellipse, Bounds::new(edge - PORT / 2.0, middle - PORT / 2.0, PORT, PORT), ink)));
            }
        }
        let _ = self.board.group(&keys, Key::from_raw(next()));
        ports
    }

    /// A voice: an oscillator through a filter and an amplifier to the output, the amplifier's gain shaped by an envelope.
    fn voice() -> Patchbay {
        let mut p = Patchbay::default();
        let mut n = 0;
        let mut next = || {
            n += 1;
            Key::<Shape>::fixed(n).raw()
        };
        let oscillator = p.build(Module::Oscillator, Point::new(40.0, 40.0), &mut next);
        let filter = p.build(Module::Filter, Point::new(260.0, 40.0), &mut next);
        let envelope = p.build(Module::Envelope, Point::new(260.0, 180.0), &mut next);
        let amplifier = p.build(Module::Amplifier, Point::new(480.0, 40.0), &mut next);
        let output = p.build(Module::Output, Point::new(700.0, 40.0), &mut next);
        let out = |ports: &[(Flow, Key<Shape>)]| ports.iter().find(|(f, _)| *f == Flow::Out).map(|(_, k)| *k);
        let input = |ports: &[(Flow, Key<Shape>)], i: usize| ports.iter().filter(|(f, _)| *f == Flow::In).nth(i).map(|(_, k)| *k);
        for (from, to) in [(out(&oscillator), input(&filter, 0)), (out(&filter), input(&amplifier, 0)), (out(&envelope), input(&amplifier, 1)), (out(&amplifier), input(&output, 0))] {
            let (Some(from), Some(to)) = (from, to) else { continue };
            let wire = cable(&p.board, from, to);
            let _ = p.board.perform(&mut p.chosen, wire, &mut next);
        }
        let _ = p.chosen.choose(&ledger::Choose::Clear);
        p.camera = Index::of(&p.board, &Page::FIRST).bounds().map(everything).unwrap_or_default();
        p
    }
}

/// The camera that shows all of `area`.
fn everything(area: Bounds) -> Camera {
    let zoom = (600.0 / area.w.max(1.0)).min(360.0 / area.h.max(1.0)).clamp(Camera::ZOOMS.0, 1.0);
    Camera { x: area.x - 24.0 / zoom, y: area.y - 24.0 / zoom, zoom }
}

/// Which way a port's signal goes, when `key` is a port: an ellipse in a module, on its left edge or its right.
fn port(board: &Board, key: Key<Shape>) -> Option<Flow> {
    let shape = board.shapes.find(key).filter(|s| s.kind == Kind::Ellipse && s.group.is_some())?;
    let body = board.whole(&[key]).into_iter().filter_map(|k| board.shapes.find(k)).find(|s| s.kind == Kind::Rect)?;
    Some(if shape.bounds().center().x < body.bounds().center().x { Flow::In } else { Flow::Out })
}

/// Whether a wire may run from `start` to `end`, and why not when it may not.
fn wiring(board: &Board, start: Option<Key<Shape>>, end: Option<Key<Shape>>) -> Result<(), Message> {
    let module = |key: Key<Shape>| board.shapes.find(key).and_then(|s| s.group);
    match (start, end) {
        (None, _) => Err(t!("A wire starts at an output, on a module's right")),
        (Some(start), _) if port(board, start) != Some(Flow::Out) => Err(t!("A wire starts at an output, on a module's right")),
        (_, Some(end)) if port(board, end) == Some(Flow::In) => match (start.and_then(module), module(end)) {
            (a, b) if a == b => Err(t!("A module can't feed itself")),
            _ if board.bindings.linked(Binding::to, end).any(|b| b.end == End::End) => Err(t!("That input has a wire already")),
            _ => Ok(()),
        },
        _ => Err(t!("A wire ends at an input, on a module's left")),
    }
}

/// A wire from an output to an input, its ends on the ports' outer edges.
fn cable(board: &Board, from: Key<Shape>, to: Key<Shape>) -> Intent {
    let at = |key: Key<Shape>, side: f64| board.shapes.find(key).map(|s| s.bounds().center() + Point::new(side * PORT / 2.0, 0.0)).unwrap_or_default();
    Intent::Create { shape: Shape::arrow(Key::default(), at(from, 1.0), at(to, -1.0)), start: Some(from), end: Some(to), holds: Vec::new() }
}

impl ledger::Find<Shape> for Patchbay {
    fn find(&self, key: Key<Shape>) -> Option<&Shape> {
        self.board.shapes.find(key)
    }
}

fn main() {
    graticule::run::<Patchbay>(t!("Patchbay"));
}
