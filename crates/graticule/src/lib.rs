//! The one crate a Graticule app imports: `use graticule::prelude::*;` and `graticule::run::<App>(t!("Name"))`.

pub use kernel;
pub use ledger;

/// Everything an app's code uses, in one import.
pub mod prelude {
    pub use graticule_macros::{Commands, Facts, t};
    pub use kernel::Available::{self, No, Yes};
    pub use kernel::{App, Cx, Failure, Fill, Message, Moment, Path, States, Ui, cols, no_if};
    pub use ledger::{Edit, Key, List, Outcome, Selection, Text};
}

/// Opens the app's window with the facts it kept last time (fresh ones on the first launch) and runs it until the window closes, saving every change. With `--render <folder>`, it instead renders every named state in both appearances into that folder as pictures, and prints how long each took to paint.
pub fn run<A: kernel::App + Default + Send>(title: kernel::Message) {
    let mut args = std::env::args().skip(1);
    if let Some("--render") = args.next().as_deref() {
        let folder = std::path::PathBuf::from(args.next().unwrap_or_else(|| ".".into()));
        if let Err(e) = render::<A>(&folder) {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    let config = kernel::shell::Config::new(title.as_str());
    if let Err(e) = kernel::shell::run(config, |_| kernel::runner::Runner::open(&kernel::runner::folder(), A::default)) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn render<A: kernel::App>(folder: &std::path::Path) -> Result<(), String> {
    use kernel::ink::Appearance;
    use kernel::shell::Face;
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let mut screen = kernel::offscreen::Offscreen::new(1920, 1280, 2.0);
    for (name, facts) in A::states().list.iter() {
        for (look, appearance) in [("light", Appearance::Light), ("dark", Appearance::Dark)] {
            screen.resize(1920, 1280);
            let mut runner = kernel::runner::Runner::new(facts.clone());
            let picture = screen.render_face(appearance, &mut runner);
            picture.save_png(&folder.join(format!("{name}, default, {look}.png")))?;
            println!("{name}, default, {look}: painted in {:.2} ms", picture.paint_ms);
            let Some((cols, rows)) = runner.minimum() else { continue };
            let m = screen.metrics;
            screen.resize((cols as f32 * m.cell_w) as u32, (rows as f32 * m.cell_h) as u32);
            let mut runner = kernel::runner::Runner::new(facts.clone());
            let picture = screen.render_face(appearance, &mut runner);
            picture.save_png(&folder.join(format!("{name}, smallest, {look}.png")))?;
            println!("{name}, smallest ({cols} by {rows} cells), {look}: painted in {:.2} ms", picture.paint_ms);
        }
    }
    Ok(())
}
