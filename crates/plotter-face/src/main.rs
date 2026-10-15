use std::collections::HashSet;
use std::sync::Arc;

use graticule::kernel::canvas::Weight;
use graticule::kernel::{Camera, Drag, Drawing};
use graticule::prelude::*;
use plotter::{Board, Bounds, End, Gesture, Intent, Kind, Point, Shape, Tool};

#[derive(Facts, Default)]
struct Whiteboard {
    board: Board,
    #[session]
    chosen: Selection<Shape>,
    #[session]
    tool: Tool,
    #[session]
    camera: Camera,
    #[session]
    file: Text,
    #[derived(Layers::of, from = [board])]
    layers: Derived<Layers>,
    #[derived(Chosen::of, from = [board, chosen])]
    around: Derived<Chosen>,
}

/// The box around the chosen shapes, and whether it has corners to drag: only one shape that isn't an arrow does.
#[derive(Default)]
struct Chosen(Option<(Bounds, bool)>);

impl Chosen {
    fn of(board: &Board, chosen: &Selection<Shape>) -> Chosen {
        Chosen::around(chosen.iter().filter_map(|k| board.shapes.find(k)))
    }

    fn around<'s>(shapes: impl Iterator<Item = &'s Shape>) -> Chosen {
        let (mut around, mut count, mut corners) = (None::<Bounds>, 0, false);
        for shape in shapes {
            around = Some(around.map_or(shape.bounds(), |a| a.union(&shape.bounds())));
            count += 1;
            corners = shape.kind != Kind::Arrow;
        }
        Chosen(around.map(|a| (a, count == 1 && corners)))
    }
}

/// The board drawn off the main thread, in slices of the stack, so a drag draws again only the slices holding what it moves.
struct Layers {
    slices: Vec<(Vec<Key<Shape>>, Arc<Drawing>)>,
}

const SLICE: usize = 512;

impl Layers {
    fn of(board: &Board) -> Layers {
        let slices = board
            .stacked()
            .chunks(SLICE)
            .map(|shapes| {
                let mut drawing = Drawing::new();
                for shape in shapes {
                    draw(&mut drawing, shape);
                }
                (shapes.iter().map(|s| s.id).collect(), Arc::new(drawing))
            })
            .collect();
        Layers { slices }
    }
}

#[derive(Commands)]
enum Cmd {
    #[label("Choose")]
    Select,
    #[label("Rectangle")]
    Rect,
    #[label("Ellipse")]
    Ellipse,
    #[label("Arrow")]
    Arrow,
    #[label("Draw")]
    Draw,
    #[label("Remove")]
    Remove,
    #[label("Choose everything")]
    ChooseAll,
    #[label("Bring to front")]
    ToFront,
    #[label("Send to back")]
    ToBack,
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

impl App for Whiteboard {
    type Cmd = Cmd;

    fn available(&self, cmd: &Cmd) -> Available {
        match cmd {
            Cmd::Remove | Cmd::ToFront | Cmd::ToBack => no_if(self.chosen.is_empty(), t!("Choose a shape first")),
            Cmd::ChooseAll if self.chosen.len() == self.board.shapes.len() && !self.board.shapes.is_empty() => No(t!("Everything is chosen already")),
            Cmd::ChooseAll | Cmd::ShowAll => no_if(self.board.shapes.is_empty(), t!("The board is empty")),
            Cmd::Open | Cmd::Save => no_if(self.file.is_empty(), t!("Choose a file first")),
            Cmd::Select => no_if(self.tool == Tool::Select, t!("Choosing already")),
            Cmd::Rect => no_if(self.tool == Tool::Rect, t!("Drawing rectangles already")),
            Cmd::Ellipse => no_if(self.tool == Tool::Ellipse, t!("Drawing ellipses already")),
            Cmd::Arrow => no_if(self.tool == Tool::Arrow, t!("Drawing arrows already")),
            Cmd::Draw => no_if(self.tool == Tool::Draw, t!("Drawing freehand already")),
            Cmd::Opened(_) | Cmd::Saved(_) | Cmd::Drew(_) | Cmd::Edit(_) => Yes,
        }
    }

    fn update(&mut self, cmd: Cmd, cx: &mut Cx<Cmd>) -> Outcome {
        match cmd {
            Cmd::Select => self.hold(Tool::Select),
            Cmd::Rect => self.hold(Tool::Rect),
            Cmd::Ellipse => self.hold(Tool::Ellipse),
            Cmd::Arrow => self.hold(Tool::Arrow),
            Cmd::Draw => self.hold(Tool::Draw),
            Cmd::Remove => {
                let keys: Vec<_> = self.chosen.iter().collect();
                let _ = self.chosen.choose(&ledger::Choose::Clear);
                self.board.delete(&keys)
            }
            Cmd::ChooseAll => self.chosen.choose(&ledger::Choose::All(self.board.shapes.keys().map(Key::raw).collect())),
            Cmd::ToFront => {
                let keys: Vec<_> = self.chosen.iter().collect();
                match self.board.to_front(&keys) {
                    Outcome::Nothing => cx.say(t!("At the front already")),
                    changed => changed,
                }
            }
            Cmd::ToBack => {
                let keys: Vec<_> = self.chosen.iter().collect();
                match self.board.to_back(&keys) {
                    Outcome::Nothing => cx.say(t!("At the back already")),
                    changed => changed,
                }
            }
            Cmd::ShowAll => {
                let Some(area) = self.board.shapes.iter().map(Shape::bounds).reduce(|a, b| a.union(&b)) else { return Outcome::Nothing };
                let zoom = (720.0 / area.w.max(1.0)).min(400.0 / area.h.max(1.0)).clamp(Camera::ZOOMS.0, 1.0);
                let camera = Camera { x: area.x - 24.0 / zoom, y: area.y - 24.0 / zoom, zoom };
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
                    let keys: Vec<_> = self.board.shapes.keys().collect();
                    let _ = self.board.delete(&keys);
                    for shape in board.shapes.iter() {
                        let _ = self.board.shapes.push(shape.clone());
                    }
                    for binding in board.bindings.iter() {
                        let _ = self.board.bindings.push(binding.clone());
                    }
                    Outcome::Changed
                }
                None => cx.say(t!("That file holds no board")),
            },
            Cmd::Opened(Err(failure)) | Cmd::Saved(Err(failure)) => cx.say_failure(failure),
            Cmd::Save => {
                cx.write(Path(self.file.to_string().into()), |facts: &Whiteboard| plotter::svg(&facts.board), Cmd::Saved);
                Outcome::Nothing
            }
            Cmd::Saved(Ok(path)) => cx.say(t!("Saved to {path}")),
            Cmd::Drew(drag) => {
                let intent = Intent::of(&self.board, &self.chosen, self.tool, &gesture(&drag), self.camera.zoom);
                let made = matches!(intent, Intent::Create { .. });
                let outcome = self.board.perform(&mut self.chosen, intent, || cx.key::<Shape>().raw());
                if made {
                    self.tool = Tool::Select;
                }
                outcome
            }
            Cmd::Edit(edit) => edit.apply(self),
        }
    }

    fn view(&self, ui: &mut Ui<Cmd>) {
        ui.page(t!("Plotter"), |ui| {
            ui.row(|ui| {
                ui.group(|ui| {
                    ui.toggle(self.tool == Tool::Select, Cmd::Select);
                    ui.toggle(self.tool == Tool::Rect, Cmd::Rect);
                    ui.toggle(self.tool == Tool::Ellipse, Cmd::Ellipse);
                    ui.toggle(self.tool == Tool::Arrow, Cmd::Arrow);
                    ui.toggle(self.tool == Tool::Draw, Cmd::Draw);
                });
                ui.button(Cmd::Remove);
                ui.button(Cmd::ShowAll);
            });
            let count = self.board.shapes.len();
            ui.canvas(t!("Board, {count} shapes"), 0, |canvas| {
                canvas.camera(Whiteboard::camera);
                canvas.on_drag(Cmd::Drew);
                let intent = canvas.drag().map(|drag| Intent::of(&self.board, &self.chosen, self.tool, &gesture(drag), self.camera.zoom));
                let moved = intent.as_ref().map(|i| i.preview(&self.board)).unwrap_or_default();
                let moving: HashSet<Key<Shape>> = moved.iter().map(|m| m.id).collect();
                let shown = |key: Key<Shape>| moved.iter().find(|m| m.id == key).or(self.board.shapes.find(key));
                match self.layers.get().filter(|_| !self.layers.coming()) {
                    Some(layers) => {
                        for (keys, drawing) in &layers.slices {
                            if keys.iter().any(|k| moving.contains(k)) {
                                let mut redrawn = Drawing::new();
                                for shape in keys.iter().filter_map(|k| shown(*k)) {
                                    draw(&mut redrawn, shape);
                                }
                                canvas.drawing(&Arc::new(redrawn));
                            } else {
                                canvas.drawing(drawing);
                            }
                        }
                    }
                    None => {
                        for shape in self.board.stacked() {
                            draw(canvas, shown(shape.id).unwrap_or(shape));
                        }
                    }
                }
                for shape in moved.iter().filter(|m| !self.board.shapes.has(m.id)) {
                    draw(canvas, shape);
                }
                let around = match self.around.get().filter(|_| moved.is_empty() && !self.around.coming()) {
                    Some(Chosen(around)) => *around,
                    None => Chosen::around(self.chosen.iter().filter_map(shown)).0,
                };
                if let Some((b, corners)) = around {
                    outline(canvas, b, corners);
                }
                match &intent {
                    Some(Intent::Brush { area, .. }) => {
                        corners(canvas, *area, Ink::Muted);
                    }
                    Some(Intent::Translate { guides, .. }) => {
                        let reach = moved.iter().map(Shape::bounds).reduce(|a, b| a.union(&b)).unwrap_or_default().expand(400.0);
                        for g in guides {
                            let (from, to) = if g.vertical { ((g.at, reach.y), (g.at, reach.bottom())) } else { ((reach.x, g.at), (reach.right(), g.at)) };
                            canvas.line(at(from.0, from.1), at(to.0, to.1)).ink(Ink::Danger).stroke(Weight::Hair);
                        }
                    }
                    _ => {}
                }
            });
            ui.row(|ui| {
                ui.file_field(t!("File"), Whiteboard::file);
                ui.button(Cmd::Open);
                ui.button(Cmd::Save);
            });
        });
    }

    fn states() -> States<Self> {
        States::new()
            .add("Empty", Whiteboard::default())
            .add("A diagram", Whiteboard::diagram())
            .add("A shape chosen", {
                let mut w = Whiteboard::diagram();
                let first = w.board.stacked()[0].id;
                let _ = w.chosen.choose(&ledger::Choose::Only(first.raw()));
                w
            })
            .add("Many shapes", Whiteboard::many(2_000))
    }
}

impl Whiteboard {
    fn hold(&mut self, tool: Tool) -> Outcome {
        self.tool = tool;
        Outcome::Changed
    }

    fn diagram() -> Whiteboard {
        let mut w = Whiteboard::default();
        let boxes = [(40.0, 40.0, "Facts"), (280.0, 40.0, "Commands"), (160.0, 200.0, "View")];
        let mut n = 0;
        let mut next = || {
            n += 1;
            Key::<Shape>::fixed(n).raw()
        };
        let mut keys = Vec::new();
        for (x, y, _) in boxes {
            let key = Key::from_raw(next());
            let _ = w.board.add(Shape::new(key, if y > 100.0 { Kind::Ellipse } else { Kind::Rect }, Bounds::new(x, y, 160.0, 80.0)));
            keys.push(key);
        }
        for (x, y, label) in boxes {
            let _ = w.board.add(Shape::new(Key::from_raw(next()), Kind::Text(label.to_string()), Bounds::new(if y > 100.0 { x + 56.0 } else { x + 16.0 }, y + 32.0, 120.0, 16.0)));
        }
        let (facts, commands, view) = (keys[0], keys[1], keys[2]);
        for (from, to) in [(commands, facts), (facts, view)] {
            let (Some(a), Some(b)) = (w.board.shapes.find(from).map(|s| s.bounds().center()), w.board.shapes.find(to).map(|s| s.bounds().center())) else { continue };
            let drawn = Gesture { from: a, to: b, trail: vec![a, b], moved: true, shift: false };
            let intent = Intent::of(&w.board, &w.chosen, Tool::Arrow, &drawn, 1.0);
            let _ = w.board.perform(&mut w.chosen, intent, &mut next);
        }
        let _ = w.chosen.choose(&ledger::Choose::Clear);
        let stroke: Vec<f64> = (0..=40)
            .flat_map(|i| {
                let t = f64::from(i) / 40.0;
                [t * 160.0, 20.0 * (t * std::f64::consts::TAU).sin() + 20.0]
            })
            .collect();
        let _ = w.board.add(Shape::new(Key::from_raw(next()), Kind::Draw(stroke), Bounds::new(400.0, 220.0, 160.0, 40.0)));
        w
    }

    fn many(count: u64) -> Whiteboard {
        let mut w = Whiteboard::default();
        for n in 0..count {
            let (col, row) = ((n % 50) as f64, (n / 50) as f64);
            let kind = if n % 2 == 0 { Kind::Rect } else { Kind::Ellipse };
            let _ = w.board.add(Shape::new(Key::fixed(n + 1), kind, Bounds::new(col * 40.0, row * 40.0, 30.0, 30.0)));
        }
        w.camera = Camera { zoom: 0.5, ..Camera::default() };
        w
    }
}

fn gesture(drag: &Drag) -> Gesture {
    let point = |(x, y): (f64, f64)| Point::new(x, y);
    Gesture { from: point(drag.from), to: point(drag.to), trail: drag.trail.iter().copied().map(point).collect(), moved: drag.moved, shift: drag.shift }
}

fn at(x: f64, y: f64) -> (f32, f32) {
    (x as f32, y as f32)
}

fn ink(ink: plotter::Ink) -> Ink {
    match ink {
        plotter::Ink::Ink => Ink::Ink,
        plotter::Ink::Muted => Ink::Muted,
        plotter::Ink::Link => Ink::Link,
        plotter::Ink::Ok => Ink::Ok,
        plotter::Ink::Warn => Ink::Warn,
        plotter::Ink::Danger => Ink::Danger,
    }
}

fn draw(canvas: &mut Drawing, shape: &Shape) {
    let color = ink(shape.ink);
    let b = Bounds::new(shape.x, shape.y, shape.w, shape.h);
    match &shape.kind {
        Kind::Rect => {
            corners(canvas, b, color);
        }
        Kind::Ellipse => {
            let (c, rx, ry) = (b.center(), b.w / 2.0, b.h / 2.0);
            let k = 0.552_284_75;
            canvas
                .path(|pen| {
                    pen.move_to(at(c.x + rx, c.y))
                        .curve_to(at(c.x + rx, c.y + ry * k), at(c.x + rx * k, c.y + ry), at(c.x, c.y + ry))
                        .curve_to(at(c.x - rx * k, c.y + ry), at(c.x - rx, c.y + ry * k), at(c.x - rx, c.y))
                        .curve_to(at(c.x - rx, c.y - ry * k), at(c.x - rx * k, c.y - ry), at(c.x, c.y - ry))
                        .curve_to(at(c.x + rx * k, c.y - ry), at(c.x + rx, c.y - ry * k), at(c.x + rx, c.y))
                        .close();
                })
                .stroke(Weight::Line)
                .ink(color);
        }
        Kind::Text(text) => {
            canvas.text(at(b.x, b.y + b.h / 2.0), text).ink(color);
        }
        Kind::Draw(points) => {
            canvas.polyline(points.chunks_exact(2).map(|p| at(shape.x + p[0], shape.y + p[1]))).ink(color).stroke(Weight::Bold);
        }
        Kind::Arrow => {
            let (a, z) = (shape.end(End::Start), shape.end(End::End));
            canvas.line(at(a.x, a.y), at(z.x, z.y)).ink(color);
            let along = z - a;
            let length = along.length();
            if length > 0.0 {
                let (ux, uy) = (along.x / length, along.y / length);
                let head = 12.0_f64.min(length / 2.0);
                let wing = |side: f64| at(z.x - head * ux - side * head * 0.5 * uy, z.y - head * uy + side * head * 0.5 * ux);
                canvas.polyline([wing(1.0), at(z.x, z.y), wing(-1.0)]).ink(color);
            }
        }
    }
}

fn corners(canvas: &mut Drawing, b: Bounds, color: Ink) {
    canvas.polyline([at(b.x, b.y), at(b.right(), b.y), at(b.right(), b.bottom()), at(b.x, b.bottom()), at(b.x, b.y)]).ink(color);
}

fn outline(canvas: &mut Drawing, b: Bounds, corners: bool) {
    let b = b.expand(4.0);
    canvas.polyline([at(b.x, b.y), at(b.right(), b.y), at(b.right(), b.bottom()), at(b.x, b.bottom()), at(b.x, b.y)]).ink(Ink::Link).stroke(Weight::Hair);
    if corners {
        for (x, y) in [(b.x, b.y), (b.right(), b.y), (b.right(), b.bottom()), (b.x, b.bottom())] {
            canvas.dot(at(x, y)).ink(Ink::Link);
        }
    }
}

fn main() {
    graticule::run::<Whiteboard>(t!("Plotter"));
}
