use kernel::grid::Area;
use kernel::ink::{Appearance, Ink, contrast};
use kernel::runner::Runner;
use kernel::shell::Face;
use kernel::tree::{NONE, Tree};

use crate::screen::{HEIGHT, Screen, WIDTH};
use crate::{Finding, Law};

pub struct Taste {
    pub findings: Vec<Finding>,
    pub allocations: Vec<(String, u64)>,
}

pub fn run<A: kernel::App>(starts: &[(String, A)]) -> Taste {
    let mut taste = Taste { findings: Vec::new(), allocations: Vec::new() };
    let mut screen = Screen::new();
    for (name, facts) in starts {
        screen.resize(WIDTH, HEIGHT);
        let mut runner = Runner::new(facts.clone());
        screen.settle(&mut runner);
        grid(&runner.tree, screen.metrics.screen(), &format!("{name}, at its normal size"), &mut taste.findings);
        if let Some(count) = steady(&mut screen, &mut runner) {
            taste.allocations.push((format!("{name}, at its normal size"), count));
        }
        let Some((cols, rows)) = runner.minimum() else { continue };
        screen.cells(cols, rows);
        let mut runner = Runner::new(facts.clone());
        screen.settle(&mut runner);
        grid(&runner.tree, screen.metrics.screen(), &format!("{name}, at its smallest size ({cols} by {rows} cells)"), &mut taste.findings);
    }
    if let Some((_, facts)) = starts.first() {
        let runner = Runner::new(facts.clone());
        for appearance in [Appearance::Light, Appearance::Dark] {
            let theme = runner.theme(appearance);
            for ink in Ink::ALL {
                let Some(needs) = ink.needs() else { continue };
                let ratio = contrast(theme.get(ink), theme.bg);
                if ratio < needs {
                    let look = if appearance == Appearance::Light { "light" } else { "dark" };
                    taste.findings.push(Finding { law: Law::Contrast, said: format!("{} on the background reaches {ratio:.2}:1 in the {look} appearance; it needs {needs}:1", ink.name()), from: String::new(), steps: Vec::new() });
                }
            }
        }
    }
    taste
}

fn grid<C>(tree: &Tree<C>, window: Area, at: &str, findings: &mut Vec<Finding>) {
    let within = |inner: Area, outer: Area| inner.col >= outer.col && inner.row >= outer.row && inner.right() <= outer.right() && inner.bottom() <= outer.bottom();
    for n in &tree.nodes {
        if n.area.is_empty() {
            continue;
        }
        let said = if !within(n.area, window) {
            format!("a {} {} leaves the window, {} by {} cells", n.kind.role(), at_cells(n.area), window.cols, window.rows)
        } else if n.parent != NONE && !within(n.area, tree.nodes[n.parent as usize].area) {
            let parent = tree.nodes[n.parent as usize];
            format!("a {} {} leaves its parent, a {} {}", n.kind.role(), at_cells(n.area), parent.kind.role(), at_cells(parent.area))
        } else {
            continue;
        };
        findings.push(Finding { law: Law::OnTheGrid, said, from: at.to_string(), steps: Vec::new() });
        return;
    }
}

#[cfg(feature = "count-allocations")]
fn steady<A: kernel::App>(screen: &mut Screen, runner: &mut Runner<A>) -> Option<u64> {
    screen.paint(runner);
    screen.paint(runner);
    Some(allocation_counter::measure(|| {
        screen.paint(runner);
    })
    .count_total)
}

#[cfg(not(feature = "count-allocations"))]
fn steady<A: kernel::App>(_: &mut Screen, _: &mut Runner<A>) -> Option<u64> {
    None
}

fn at_cells(a: Area) -> String {
    format!("at column {}, row {}, {} by {} cells", a.col, a.row, a.cols, a.rows)
}
