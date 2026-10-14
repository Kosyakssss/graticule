use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowId};

use crate::gpu::{Gpu, Quad};
use crate::ink::{Appearance, Theme};
use crate::metrics::Metrics;
use crate::pace::Pace;
use crate::scope::{Cause, Recorder, Stages};
use crate::paint::Painter;
use crate::text::Text;

pub struct Config {
    pub title: String,
    pub size: (f64, f64),
    pub min_size: (f64, f64),
}

impl Config {
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), size: (960.0, 640.0), min_size: (320.0, 200.0) }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Mods {
    pub shift: bool,
    pub alt: bool,
    pub primary: bool,
    pub control: bool,
}

impl Mods {
    fn of(m: ModifiersState) -> Self {
        let mac = cfg!(target_os = "macos");
        Self { shift: m.shift_key(), alt: m.alt_key(), primary: if mac { m.super_key() } else { m.control_key() }, control: if mac { m.control_key() } else { m.super_key() } }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Middle,
    Right,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    Moved { x: f32, y: f32 },
    Left,
    Press { button: Button, clicks: u32 },
    Release { button: Button },
    Wheel { dx: f32, dy: f32 },
    Key(winit::event::KeyEvent),
    Ime(winit::event::Ime),
    Appearance(Appearance),
    Resized,
    Woken,
}

pub struct Frame<'a> {
    pub window: &'a Window,
    pub text: &'a mut Text,
    pub metrics: Metrics,
    pub mods: Mods,
    pub pointer: Option<(f32, f32)>,
    pub appearance: Appearance,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Seen {
    pub mods: Mods,
    pub pointer: Option<(f32, f32)>,
    pub appearance: Appearance,
}

/// What the shell runs: something that paints a frame and answers input. The kernel's app runner is one; Specimen's offscreen pages are another.
pub trait Face: 'static {
    fn theme(&self, appearance: Appearance) -> Theme {
        *Theme::of(appearance)
    }
    fn paint(&mut self, painter: &mut Painter, seen: Seen);
    fn input(&mut self, input: Input, frame: &mut Frame) -> bool;
    fn painted(&mut self, _: &mut Frame) {}
    /// The smallest window this face fits in, in cells, once it knows.
    fn minimum(&self) -> Option<(i32, i32)> {
        None
    }
    /// The kept quads this face paints into, when it keeps them between frames instead of painting every quad each frame.
    fn kept(&mut self) -> Option<&mut crate::kept::Kept> {
        None
    }
    fn quit(&self) -> bool {
        false
    }
    /// The accessibility tree for this frame, asked for only while a screen reader or another assistive tool is listening.
    fn access(&mut self, _: Metrics, _: &str) -> Option<accesskit::TreeUpdate> {
        None
    }
    /// An assistive tool asked a node to act, by its id.
    fn act(&mut self, _: u64, _: accesskit::Action) -> bool {
        false
    }
    /// How long the last paint spent building the view, laying it out and painting, for Scope. A face that doesn't measure these leaves them at zero and its whole paint counts as painting.
    fn stages(&self) -> Stages {
        Stages::default()
    }
}

fn cause(input: &Input) -> Cause {
    match input {
        Input::Moved { .. } | Input::Left => Cause::Pointer,
        Input::Press { .. } | Input::Release { .. } => Cause::Click,
        Input::Wheel { .. } => Cause::Wheel,
        Input::Key(_) => Cause::Key,
        Input::Ime(_) => Cause::Typing,
        Input::Appearance(_) => Cause::Appearance,
        Input::Resized => Cause::Resize,
        Input::Woken => Cause::Woken,
    }
}

fn micros(since: Instant) -> u32 {
    since.elapsed().as_micros().min(u32::MAX as u128) as u32
}

/// What wakes the event loop: the app's own threads, or an assistive tool.
pub enum Signal {
    Wake,
    Access(accesskit_winit::Event),
}

impl From<accesskit_winit::Event> for Signal {
    fn from(e: accesskit_winit::Event) -> Self {
        Signal::Access(e)
    }
}

#[derive(Clone)]
pub struct Waker {
    proxy: EventLoopProxy<Signal>,
}

impl Waker {
    pub fn wake(&self) {
        let _ = self.proxy.send_event(Signal::Wake);
    }
}

pub struct Launch {
    pub stages: Vec<(&'static str, f32)>,
}

pub fn run<F: Face>(config: Config, make: impl FnOnce(Waker) -> F + 'static) -> Result<(), String> {
    let started = Instant::now();
    let event_loop = EventLoop::<Signal>::with_user_event().build().map_err(|e| e.to_string())?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let proxy = event_loop.create_proxy();
    let mut shell = Shell::<F> { config, proxy, running: None, failed: None, make: Some(Box::new(make)), started, loop_ready: Instant::now() };
    event_loop.run_app(&mut shell).map_err(|e| e.to_string())?;
    shell.failed.map_or(Ok(()), Err)
}

type Make<F> = Box<dyn FnOnce(Waker) -> F>;

struct Shell<F: Face> {
    config: Config,
    proxy: EventLoopProxy<Signal>,
    running: Option<Running<F>>,
    failed: Option<String>,
    make: Option<Make<F>>,
    started: Instant,
    loop_ready: Instant,
}

struct Running<F: Face> {
    face: F,
    window: Arc<Window>,
    gpu: Gpu,
    text: Text,
    metrics: Metrics,
    quads: Vec<Quad>,
    pace: Pace,
    modifiers: ModifiersState,
    pointer: Option<(f32, f32)>,
    clicks: Clicks,
    appearance: Appearance,
    scope: bool,
    recorder: Recorder,
    launch: Vec<(&'static str, f32)>,
    minimum: Option<(i32, i32)>,
    access: accesskit_winit::Adapter,
    title: String,
}

#[derive(Default)]
struct Clicks {
    last: Option<(Instant, f32, f32)>,
    count: u32,
}

impl Clicks {
    fn press(&mut self, x: f32, y: f32) -> u32 {
        let now = Instant::now();
        let again = self.last.is_some_and(|(t, px, py)| now - t < std::time::Duration::from_millis(400) && (px - x).abs() < 6.0 && (py - y).abs() < 6.0);
        self.last = Some((now, x, y));
        self.count = if again { (self.count + 1).min(3) } else { 1 };
        self.count
    }
}

macro_rules! frame {
    ($r:expr) => {
        Frame { window: &$r.window, text: &mut $r.text, metrics: $r.metrics, mods: Mods::of($r.modifiers), pointer: $r.pointer, appearance: $r.appearance }
    };
}

impl<F: Face> Running<F> {
    #[profiling::function]
    fn render(&mut self) {
        let started = Instant::now();
        self.pace.drawn();
        self.text.begin_frame();
        let theme = self.face.theme(self.appearance);
        self.quads.clear();
        let seen = Seen { mods: Mods::of(self.modifiers), pointer: self.pointer, appearance: self.appearance };
        let mut painter = Painter::new(&mut self.quads, &mut self.text, self.metrics, theme);
        self.face.paint(&mut painter, seen);
        let mut stages = self.face.stages();
        if stages == Stages::default() {
            stages.paint = micros(started);
        }
        let painted = Instant::now();
        for u in self.text.uploads.drain(..) {
            self.gpu.upload(u.x, u.y, u.w, u.h, &u.data);
        }
        match self.face.kept() {
            Some(kept) => {
                let [under, over] = &mut kept.layers;
                self.gpu.keep([&under.quads, &over.quads], [under.dirty.take(), over.dirty.take()]);
                self.gpu.render_kept(theme.bg);
            }
            None => self.gpu.render(&self.quads, theme.bg),
        }
        let gpu = micros(painted);
        let rendered = Instant::now();
        if let Some((cols, rows)) = self.face.minimum()
            && self.minimum != Some((cols, rows))
        {
            self.minimum = Some((cols, rows));
            let m = self.metrics;
            self.window.set_min_inner_size(Some(winit::dpi::PhysicalSize::new(cols as f32 * m.cell_w, rows as f32 * m.cell_h)));
        }
        let mut frame = frame!(self);
        self.face.painted(&mut frame);
        self.update_access();
        self.recorder.drawn(stages, gpu, micros(rendered), micros(started));
        profiling::finish_frame!();
    }

    fn wants(&mut self, cause: Cause, since: Instant) {
        self.recorder.wanted(cause, micros(since));
        if self.pace.want() {
            self.window.request_redraw();
        }
    }

    fn update_access(&mut self) {
        let (face, metrics, title) = (&mut self.face, self.metrics, self.title.as_str());
        self.access.update_if_active(|| {
            face.access(metrics, title).unwrap_or_else(|| accesskit::TreeUpdate {
                nodes: vec![(accesskit::NodeId(0), accesskit::Node::new(accesskit::Role::Window))],
                tree: Some(accesskit::TreeInfo::new(accesskit::NodeId(0))),
                tree_id: accesskit::TreeId::ROOT,
                focus: accesskit::NodeId(0),
            })
        });
    }

    fn input(&mut self, input: Input) {
        let since = Instant::now();
        let why = cause(&input);
        let mut frame = frame!(self);
        if self.face.input(input, &mut frame) {
            self.wants(why, since);
        } else {
            self.recorder.spent(micros(since));
        }
    }
}

impl<F: Face> ApplicationHandler<Signal> for Shell<F> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.running.is_some() {
            return;
        }
        let mut marks = vec![("event loop", self.loop_ready), ("system", Instant::now())];
        let attributes = Window::default_attributes()
            .with_title(&self.config.title)
            .with_inner_size(LogicalSize::new(self.config.size.0, self.config.size.1))
            .with_min_inner_size(LogicalSize::new(self.config.min_size.0, self.config.min_size.1))
            .with_visible(false);
        let window = match event_loop.create_window(attributes) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.failed = Some(e.to_string());
                event_loop.exit();
                return;
            }
        };
        let access = accesskit_winit::Adapter::with_event_loop_proxy(event_loop, &window, self.proxy.clone());
        marks.push(("window", Instant::now()));
        let size = window.inner_size();
        let gpu = Gpu::new(Some(window.clone()), (size.width, size.height));
        marks.push(("gpu", Instant::now()));
        let mut text = Text::new();
        let wake = self.proxy.clone();
        text.set_wake(move || {
            let _ = wake.send_event(Signal::Wake);
        });
        let metrics = Metrics::measure(&mut text, window.scale_factor() as f32, size.width, size.height);
        marks.push(("fonts", Instant::now()));
        let Some(make) = self.make.take() else { return };
        let face = make(Waker { proxy: self.proxy.clone() });
        marks.push(("app", Instant::now()));
        let appearance = match window.theme() {
            Some(winit::window::Theme::Dark) => Appearance::Dark,
            _ => Appearance::Light,
        };
        let scope = std::env::var_os("GRATICULE_SCOPE").is_some_and(|v| v != "0");
        let mut running = Running {
            face,
            window,
            gpu,
            text,
            metrics,
            quads: Vec::with_capacity(8192),
            pace: Pace::Idle,
            modifiers: ModifiersState::empty(),
            pointer: None,
            clicks: Clicks::default(),
            appearance,
            scope,
            recorder: Recorder::new(&self.config.title, self.started),
            launch: Vec::new(),
            minimum: None,
            access,
            title: self.config.title.clone(),
        };
        running.render();
        running.window.set_visible(true);
        marks.push(("first frame", Instant::now()));
        let mut last = self.started;
        running.launch = marks
            .into_iter()
            .map(|(stage, at)| {
                let ms = (at - last).as_secs_f32() * 1000.0;
                last = at;
                (stage, ms)
            })
            .collect();
        if running.scope {
            let stages: Vec<String> = running.launch.iter().map(|(stage, ms)| format!("{stage} {ms:.1}")).collect();
            eprintln!("launch {:.1} ms from main: {}", (last - self.started).as_secs_f32() * 1000.0, stages.join(", "));
        }
        running.recorder.launched(&running.launch);
        self.running = Some(running);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, signal: Signal) {
        let Some(r) = self.running.as_mut() else { return };
        match signal {
            Signal::Wake => r.input(Input::Woken),
            Signal::Access(e) => match e.window_event {
                accesskit_winit::WindowEvent::InitialTreeRequested => r.update_access(),
                accesskit_winit::WindowEvent::ActionRequested(request) => {
                    let since = Instant::now();
                    if r.face.act(request.target_node.0, request.action) {
                        r.wants(Cause::Assistive, since);
                    }
                }
                accesskit_winit::WindowEvent::AccessibilityDeactivated => {}
            },
        }
        if r.face.quit() {
            event_loop.exit();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(r) = self.running.as_mut() else { return };
        r.access.process_event(&r.window, &event);
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                r.gpu.resize(size.width, size.height);
                r.metrics = Metrics::measure(&mut r.text, r.metrics.scale, size.width, size.height);
                r.input(Input::Resized);
                r.recorder.wanted(Cause::Resize, 0);
                r.render();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let size = r.window.inner_size();
                r.metrics = Metrics::measure(&mut r.text, scale_factor as f32, size.width, size.height);
                r.minimum = None;
                r.input(Input::Resized);
            }
            WindowEvent::RedrawRequested => r.render(),
            WindowEvent::ModifiersChanged(m) => r.modifiers = m.state(),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => r.input(Input::Key(event)),
            WindowEvent::Ime(ime) => r.input(Input::Ime(ime)),
            WindowEvent::CursorMoved { position, .. } => {
                let (x, y) = (position.x as f32, position.y as f32);
                r.pointer = Some((x, y));
                r.input(Input::Moved { x, y });
            }
            WindowEvent::CursorLeft { .. } => {
                r.pointer = None;
                r.input(Input::Left);
            }
            WindowEvent::ThemeChanged(t) => {
                r.appearance = if t == winit::window::Theme::Dark { Appearance::Dark } else { Appearance::Light };
                r.input(Input::Appearance(r.appearance));
            }
            WindowEvent::Occluded(false) => {
                r.wants(Cause::Uncovered, Instant::now());
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    MouseButton::Left => Button::Left,
                    MouseButton::Middle => Button::Middle,
                    MouseButton::Right => Button::Right,
                    _ => return,
                };
                match state {
                    ElementState::Pressed => {
                        let (x, y) = r.pointer.unwrap_or_default();
                        let clicks = if button == Button::Left { r.clicks.press(x, y) } else { 1 };
                        r.input(Input::Press { button, clicks });
                    }
                    ElementState::Released => r.input(Input::Release { button }),
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x * r.metrics.cell_w * 3.0, y * r.metrics.cell_h * 3.0),
                    MouseScrollDelta::PixelDelta(p) => (p.x as f32, p.y as f32),
                };
                r.input(Input::Wheel { dx, dy });
            }
            _ => {}
        }
        if r.face.quit() {
            event_loop.exit();
        }
    }
}
