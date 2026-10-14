//! The one crate a Graticule app imports.

pub use kernel;

pub mod prelude {}

/// Opens an app's window. Until the first slice's tree lands, the window shows the empty grid.
pub fn run(title: &str) {
    if let Err(e) = kernel::shell::run(kernel::shell::Config::new(title), |_| Empty) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

struct Empty;

impl kernel::shell::Face for Empty {
    fn paint(&mut self, painter: &mut kernel::paint::Painter, _: kernel::shell::Seen) {
        painter.grid();
    }

    fn input(&mut self, input: kernel::shell::Input, _: &mut kernel::shell::Frame) -> bool {
        matches!(input, kernel::shell::Input::Appearance(_) | kernel::shell::Input::Resized | kernel::shell::Input::Woken)
    }
}
