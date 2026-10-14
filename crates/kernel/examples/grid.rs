use kernel::ink::{Appearance, Theme};
use kernel::offscreen::Offscreen;

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "grid.png".into());
    let mut offscreen = Offscreen::new(800, 500, 2.0);
    for (appearance, suffix) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        let picture = offscreen.render(*Theme::of(appearance), |p| {
            p.grid();
            let (mono, sans) = (p.m.mono(), p.m.sans(400.0));
            let (ink, muted) = (p.theme.ink, p.theme.muted);
            p.write(kernel::grid::Area::new(1, 1, 40, 1), "Graticule ┌──┬──┐ 格子 0123", mono, ink);
            p.write(kernel::grid::Area::new(1, 2, 40, 1), "Plex Sans shares the baseline", sans, muted);
        });
        let path = out.replace(".png", &format!("-{suffix}.png"));
        picture.save_png(std::path::Path::new(&path)).expect("png");
        println!("{path}: painted in {:.2} ms", picture.paint_ms);
    }
}
