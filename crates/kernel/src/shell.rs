use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowId};

use crate::gpu::{Quad, Screen};
use crate::ink::Appearance;
use crate::look::{Look, Looks, Switch};
use crate::metrics::Metrics;
use crate::pace::Pace;
use crate::paint::Painter;
use crate::reload::{Handoff, Reload};
use crate::scope::{Cause, Recorder, Stages};
use crate::text::Text;

const LATE_MARGIN: Duration = Duration::from_millis(2);

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
    Hovering(std::path::PathBuf),
    Unhovered,
    Dropped(std::path::PathBuf),
    Key(winit::event::KeyEvent),
    Ime(winit::event::Ime),
    Appearance(Appearance),
    Resized,
    Woken,
    Clock,
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
    /// The look this face is drawn in, given the one the user chose; Specimen's style editor draws in the look being edited.
    fn look(&self, look: Look) -> Look {
        look
    }
    /// A change to the look that a command asked for since the shell last asked.
    fn switch(&mut self) -> Option<Switch> {
        None
    }
    /// The looks there are and which is chosen, given at the start and whenever they change.
    fn offered(&mut self, _: &Looks) {}
    fn paint(&mut self, painter: &mut Painter, seen: Seen);
    fn input(&mut self, input: Input, frame: &mut Frame) -> bool;
    fn painted(&mut self, _: &mut Frame) {}
    /// The context menu for what is under the pointer, asked for on a right click: the registry's commands for it, in the order the bar shows them.
    fn menu(&mut self, _: &mut Frame) -> Vec<crate::menu::MenuItem> {
        Vec::new()
    }
    /// A menu entry was chosen, by its id.
    fn choose(&mut self, _: u64) -> bool {
        false
    }
    /// The moment this face would look different with no input at all: when a date written as how long ago next reads differently. The shell sleeps until then and not a moment sooner.
    fn changes(&self) -> Option<crate::app::Moment> {
        None
    }
    /// How long this face has spent working out derived values in line, all told, which a window does on threads of their own; offscreen renders leave it out of the time they print.
    fn worked_aside(&self) -> std::time::Duration {
        std::time::Duration::ZERO
    }
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
    /// What this face hands to the next build of itself when `bindery dev` reloads the app: whatever the new build can't get back on its own, such as facts that aren't saved and where the user was.
    fn keep(&self) -> ledger::Value {
        ledger::Value::None
    }
    /// Takes back what the previous build kept, right after the face is made.
    fn restore(&mut self, _: &ledger::Value) {}
    /// Scope asked for something; the answer comes from `answers` after the next paint.
    fn ask(&mut self, _: crate::scope::Ask) {}
    /// The answers to what Scope asked, worked out in the last paint, each in lines of the wire.
    fn answers(&mut self) -> Vec<String> {
        Vec::new()
    }
}

fn cause(input: &Input) -> Cause {
    match input {
        Input::Moved { .. } | Input::Left | Input::Hovering(_) | Input::Unhovered => Cause::Pointer,
        Input::Press { .. } | Input::Release { .. } | Input::Dropped(_) => Cause::Click,
        Input::Wheel { .. } => Cause::Wheel,
        Input::Key(_) => Cause::Key,
        Input::Ime(_) => Cause::Typing,
        Input::Appearance(_) => Cause::Appearance,
        Input::Resized => Cause::Resize,
        Input::Woken => Cause::Woken,
        Input::Clock => Cause::Clock,
    }
}

fn micros(since: Instant) -> u32 {
    since.elapsed().as_micros().min(u32::MAX as u128) as u32
}

/// What wakes the event loop: the app's own threads, or an assistive tool.
pub enum Signal {
    Wake,
    Presented,
    Reload,
    Looks(Box<Looks>),
    Menu(u64),
    Access(accesskit_winit::Event),
    Scope(crate::scope::Ask),
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
    let reload = Reload::from_env();
    let handoff = reload.as_ref().and_then(Reload::take);
    if let Some(reload) = &reload {
        let proxy = proxy.clone();
        reload.listen(move || {
            let _ = proxy.send_event(Signal::Reload);
        });
    }
    let mut shell = Shell::<F> { config, proxy, running: None, failed: None, make: Some(Box::new(make)), started, loop_ready: Instant::now(), reload, handoff };
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
    reload: Option<Reload>,
    handoff: Option<Handoff>,
}

struct Running<F: Face> {
    face: F,
    window: Arc<Window>,
    gpu: Screen,
    held: bool,
    refresh: Option<Duration>,
    painting: Duration,
    draw_at: Option<Instant>,
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
    looks: Looks,
    _watcher: Option<notify::RecommendedWatcher>,
    _menu: Option<crate::menu::MenuBar>,
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
    fn render(&mut self, now: bool) {
        if !now && self.gpu.waiting() {
            self.held = true;
            return;
        }
        self.draw_at = None;
        let started = Instant::now();
        self.pace.drawn();
        self.text.begin_frame();
        let look = self.face.look(self.looks.look(self.appearance));
        let theme = look.theme;
        self.quads.clear();
        let seen = Seen { mods: Mods::of(self.modifiers), pointer: self.pointer, appearance: self.appearance };
        let mut painter = Painter::new(&mut self.quads, &mut self.text, self.metrics, look);
        self.face.paint(&mut painter, seen);
        let mut stages = self.face.stages();
        if stages == Stages::default() {
            stages.paint = micros(started);
        }
        let painted = Instant::now();
        let (text, kept, quads) = (&mut self.text, self.face.kept(), &self.quads);
        self.gpu.stage(|gpu| {
            for u in text.uploads.drain(..) {
                gpu.upload(u.x, u.y, u.w, u.h, &u.data);
            }
            for id in text.released.drain(..) {
                gpu.remove_image(id);
            }
            for r in text.rasters.drain(..) {
                gpu.raster(r);
            }
            for image in text.images.drain(..) {
                gpu.picture(image);
            }
            for h in text.hooks.drain(..) {
                gpu.hook(h);
            }
            match kept {
                Some(kept) => {
                    let [a, b, c, d] = &mut kept.layers;
                    gpu.keep([&a.quads, &b.quads, &c.quads, &d.quads], [a.dirty.take(), b.dirty.take(), c.dirty.take(), d.dirty.take()]);
                    gpu.show_kept(theme.bg);
                }
                None => gpu.show(quads, theme.bg),
            }
        });
        if now {
            self.gpu.present_now();
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
        self.painting = started.elapsed();
        self.recorder.drawn(stages, gpu, micros(rendered), micros(started));
        for answer in self.face.answers() {
            self.recorder.answer(answer);
        }
        profiling::finish_frame!();
    }

    fn wants(&mut self, cause: Cause, since: Instant) {
        self.recorder.wanted(cause, micros(since));
        if self.pace.want() {
            self.owe();
        }
    }

    fn owe(&mut self) {
        match self.late() {
            Some(at) => self.draw_at = Some(at),
            None => self.window.request_redraw(),
        }
    }

    fn late(&self) -> Option<Instant> {
        let (refresh, shown) = (self.refresh?, self.gpu.shown()?);
        let now = Instant::now();
        if now.duration_since(shown) > refresh * 2 {
            return None;
        }
        let mut deadline = shown + refresh;
        while deadline <= now {
            deadline += refresh;
        }
        let at = deadline.checked_sub(LATE_MARGIN + self.painting * 2)?;
        (at > now).then_some(at)
    }

    fn measure_refresh(&mut self) {
        self.refresh = self.window.current_monitor().and_then(|m| m.refresh_rate_millihertz()).filter(|&mhz| mhz > 0).map(|mhz| Duration::from_secs_f64(1000.0 / mhz as f64));
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
        let mut changed = self.face.input(input, &mut frame);
        if let Some(switch) = self.face.switch() {
            self.looks.switch(switch);
            let folder = crate::look::folder();
            if let Err(e) = std::fs::create_dir_all(&folder).and_then(|_| std::fs::write(folder.join("look.txt"), self.looks.chosen())) {
                eprintln!("The choice of look couldn't be saved in {}: {e}", folder.display());
            }
            self.face.offered(&self.looks);
            changed = true;
        }
        if changed {
            self.wants(why, since);
        } else {
            self.recorder.spent(micros(since));
        }
    }
}

impl<F: Face> ApplicationHandler<Signal> for Shell<F> {
    fn new_events(&mut self, _: &ActiveEventLoop, cause: winit::event::StartCause) {
        if let (winit::event::StartCause::ResumeTimeReached { .. }, Some(r)) = (cause, self.running.as_mut()) {
            match r.draw_at {
                Some(at) if at <= Instant::now() => {
                    r.draw_at = None;
                    r.window.request_redraw();
                }
                _ => {
                    r.input(Input::Clock);
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(r) = self.running.as_ref() else { return };
        let clock = r.face.changes().map(|at| Instant::now() + Duration::from_millis((at.0 - crate::app::Moment::now().0).max(0) as u64));
        let flow = match clock.into_iter().chain(r.draw_at).min() {
            Some(at) => ControlFlow::WaitUntil(at),
            None => ControlFlow::Wait,
        };
        event_loop.set_control_flow(flow);
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.running.is_some() {
            return;
        }
        let mut marks = vec![("event loop", self.loop_ready), ("system", Instant::now())];
        let handoff = self.handoff.take();
        let mut attributes = Window::default_attributes()
            .with_title(&self.config.title)
            .with_inner_size(LogicalSize::new(self.config.size.0, self.config.size.1))
            .with_min_inner_size(LogicalSize::new(self.config.min_size.0, self.config.min_size.1))
            .with_visible(false);
        if let Some((position, size)) = handoff.as_ref().and_then(|h| h.frame) {
            attributes = attributes.with_position(position).with_inner_size(size);
        }
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
        let presented = self.proxy.clone();
        let gpu = Screen::new(window.clone(), (size.width, size.height), move || {
            let _ = presented.send_event(Signal::Presented);
        });
        marks.push(("gpu", Instant::now()));
        let mut text = Text::new();
        let wake = self.proxy.clone();
        text.set_wake(move || {
            let _ = wake.send_event(Signal::Wake);
        });
        let metrics = Metrics::measure(&mut text, window.scale_factor() as f32, size.width, size.height);
        marks.push(("fonts", Instant::now()));
        let Some(make) = self.make.take() else { return };
        let mut face = make(Waker { proxy: self.proxy.clone() });
        if let Some(handoff) = &handoff {
            face.restore(&handoff.face);
        }
        marks.push(("app", Instant::now()));
        let appearance = match window.theme() {
            Some(winit::window::Theme::Dark) => Appearance::Dark,
            _ => Appearance::Light,
        };
        let looks = Looks::load(&crate::look::folder());
        let to = self.proxy.clone();
        let watcher = crate::look::watch(&crate::look::folder(), move |looks| {
            let _ = to.send_event(Signal::Looks(Box::new(looks)));
        });
        let to = self.proxy.clone();
        let menu = crate::menu::install(event_loop, &self.config.title, move |id| {
            if let Ok(id) = id.parse() {
                let _ = to.send_event(Signal::Menu(id));
            }
        });
        face.offered(&looks);
        let scope = std::env::var_os("GRATICULE_SCOPE").is_some_and(|v| v != "0");
        let mut running = Running {
            face,
            window,
            gpu,
            held: false,
            refresh: None,
            painting: Duration::ZERO,
            draw_at: None,
            text,
            metrics,
            quads: Vec::with_capacity(8192),
            pace: Pace::Idle,
            modifiers: ModifiersState::empty(),
            pointer: None,
            clicks: Clicks::default(),
            appearance,
            scope,
            recorder: {
                let to = self.proxy.clone();
                Recorder::new(&self.config.title, self.started, move |ask| {
                    let _ = to.send_event(Signal::Scope(ask));
                })
            },
            launch: Vec::new(),
            minimum: None,
            access,
            title: self.config.title.clone(),
            looks,
            _watcher: watcher,
            _menu: menu,
        };
        running.measure_refresh();
        running.render(true);
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
            Signal::Presented => {
                if r.held {
                    r.held = false;
                    r.owe();
                }
            }
            Signal::Looks(looks) => {
                if *looks != r.looks {
                    let since = Instant::now();
                    r.looks = *looks;
                    r.face.offered(&r.looks);
                    r.wants(Cause::Appearance, since);
                }
            }
            Signal::Scope(ask) => {
                let since = Instant::now();
                r.face.ask(ask);
                r.wants(Cause::Scope, since);
            }
            Signal::Menu(id) => {
                let since = Instant::now();
                if r.face.choose(id) {
                    r.wants(Cause::Click, since);
                }
            }
            Signal::Reload => {
                if let Some(reload) = &self.reload {
                    match reload.save(&r.window, r.face.keep()) {
                        Ok(()) => event_loop.exit(),
                        Err(e) => eprintln!("reload: {e}"),
                    }
                }
            }
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
            WindowEvent::Moved(_) => r.measure_refresh(),
            WindowEvent::Resized(size) => {
                r.measure_refresh();
                r.gpu.resize(size.width, size.height);
                r.metrics = Metrics::measure(&mut r.text, r.metrics.scale, size.width, size.height);
                r.input(Input::Resized);
                r.recorder.wanted(Cause::Resize, 0);
                r.render(true);
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let size = r.window.inner_size();
                r.metrics = Metrics::measure(&mut r.text, scale_factor as f32, size.width, size.height);
                r.minimum = None;
                r.input(Input::Resized);
            }
            WindowEvent::RedrawRequested => r.render(false),
            WindowEvent::ModifiersChanged(m) => r.modifiers = m.state(),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => r.input(Input::Key(event)),
            WindowEvent::Ime(ime) => r.input(Input::Ime(ime)),
            WindowEvent::CursorMoved { position, .. } => {
                let (x, y) = (position.x as f32, position.y as f32);
                r.pointer = Some((x, y));
                r.input(Input::Moved { x, y });
            }
            WindowEvent::HoveredFile(path) => r.input(Input::Hovering(path)),
            WindowEvent::HoveredFileCancelled => r.input(Input::Unhovered),
            WindowEvent::DroppedFile(path) => r.input(Input::Dropped(path)),
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
                        if button == Button::Right {
                            let since = Instant::now();
                            let items = r.face.menu(&mut frame!(r));
                            r.recorder.wanted(Cause::Click, micros(since));
                            r.render(true);
                            crate::menu::popup(&r.window, &items);
                        }
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
