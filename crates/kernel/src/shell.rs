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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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
    fn quit(&self) -> bool {
        false
    }
}

#[derive(Clone)]
pub struct Waker {
    proxy: EventLoopProxy<()>,
}

impl Waker {
    pub fn wake(&self) {
        let _ = self.proxy.send_event(());
    }
}

pub struct Launch {
    pub stages: Vec<(&'static str, f32)>,
}

pub fn run<F: Face>(config: Config, make: impl FnOnce(Waker) -> F + 'static) -> Result<(), String> {
    let started = Instant::now();
    let event_loop = EventLoop::<()>::with_user_event().build().map_err(|e| e.to_string())?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let proxy = event_loop.create_proxy();
    let mut shell = Shell::<F> { config, proxy, running: None, failed: None, make: Some(Box::new(make)), started, loop_ready: Instant::now() };
    event_loop.run_app(&mut shell).map_err(|e| e.to_string())?;
    shell.failed.map_or(Ok(()), Err)
}

type Make<F> = Box<dyn FnOnce(Waker) -> F>;

struct Shell<F: Face> {
    config: Config,
    proxy: EventLoopProxy<()>,
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
    launch: Vec<(&'static str, f32)>,
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
        self.pace.drawn();
        self.text.begin_frame();
        let theme = self.face.theme(self.appearance);
        self.quads.clear();
        let seen = Seen { mods: Mods::of(self.modifiers), pointer: self.pointer, appearance: self.appearance };
        let mut painter = Painter::new(&mut self.quads, &mut self.text, self.metrics, theme);
        self.face.paint(&mut painter, seen);
        for u in self.text.uploads.drain(..) {
            self.gpu.upload(u.x, u.y, u.w, u.h, &u.data);
        }
        self.gpu.render(&self.quads, theme.bg);
        let mut frame = frame!(self);
        self.face.painted(&mut frame);
        profiling::finish_frame!();
    }

    fn input(&mut self, input: Input) {
        let mut frame = frame!(self);
        if self.face.input(input, &mut frame) && self.pace.want() {
            self.window.request_redraw();
        }
    }
}

impl<F: Face> ApplicationHandler for Shell<F> {
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
        marks.push(("window", Instant::now()));
        let size = window.inner_size();
        let gpu = Gpu::new(Some(window.clone()), (size.width, size.height));
        marks.push(("gpu", Instant::now()));
        let mut text = Text::new();
        let wake = self.proxy.clone();
        text.set_wake(move || {
            let _ = wake.send_event(());
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
        let mut running = Running { face, window, gpu, text, metrics, quads: Vec::with_capacity(8192), pace: Pace::Idle, modifiers: ModifiersState::empty(), pointer: None, clicks: Clicks::default(), appearance, scope, launch: Vec::new() };
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
        self.running = Some(running);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, _: ()) {
        let Some(r) = self.running.as_mut() else { return };
        r.input(Input::Woken);
        if r.face.quit() {
            event_loop.exit();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(r) = self.running.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                r.gpu.resize(size.width, size.height);
                r.metrics = Metrics::measure(&mut r.text, r.metrics.scale, size.width, size.height);
                r.input(Input::Resized);
                r.render();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let size = r.window.inner_size();
                r.metrics = Metrics::measure(&mut r.text, scale_factor as f32, size.width, size.height);
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
                if r.pace.want() {
                    r.window.request_redraw();
                }
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
