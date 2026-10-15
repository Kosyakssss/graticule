use kernel::grid::Area;
use kernel::ink::{Appearance, Ink, Sight, contrast};
use kernel::runner::Runner;
use kernel::shell::Face;
use kernel::tree::Tree;

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
    for file in kernel::look::THEMES.iter() {
        for appearance in [Appearance::Light, Appearance::Dark] {
            let theme = file.theme(appearance);
            for ink in Ink::ALL {
                let Some(needs) = ink.needs() else { continue };
                let look = if appearance == Appearance::Light { "light" } else { "dark" };
                let ratio = contrast(theme.get(ink), theme.bg);
                if ratio < needs {
                    taste.findings.push(Finding {
                        law: Law::Contrast,
                        said: format!("{} on the background reaches {ratio:.2}:1 in {}'s {look} appearance; it needs {needs}:1", ink.name(), file.name),
                        from: String::new(),
                        steps: Vec::new(),
                    });
                }
                for sight in Sight::ALL {
                    let ratio = contrast(sight.sees(theme.get(ink)), sight.sees(theme.bg));
                    if ratio < needs {
                        taste.findings.push(Finding {
                            law: Law::ColorBlind,
                            said: format!("{} on the background reaches {ratio:.2}:1 in {}'s {look} appearance as seen with {}; it needs {needs}:1", ink.name(), file.name, sight.name()),
                            from: String::new(),
                            steps: Vec::new(),
                        });
                    }
                }
            }
        }
    }
    taste
}

fn grid<C>(tree: &Tree<C>, window: Area, at: &str, findings: &mut Vec<Finding>) {
    let flaws = kernel::taste::judge(tree, window);
    for law in kernel::taste::Law::ALL {
        if let Some(flaw) = flaws.iter().find(|f| f.law == law) {
            let law = match law {
                kernel::taste::Law::OnTheGrid => Law::OnTheGrid,
                kernel::taste::Law::Reachable => Law::Reachable,
                kernel::taste::Law::Named => Law::Named,
            };
            findings.push(Finding { law, said: flaw.said.clone(), from: at.to_string(), steps: Vec::new() });
        }
    }
}

#[cfg(feature = "count-allocations")]
fn steady<A: kernel::App>(screen: &mut Screen, runner: &mut Runner<A>) -> Option<u64> {
    screen.paint(runner);
    screen.paint(runner);
    Some(
        allocation_counter::measure(|| {
            screen.paint(runner);
        })
        .count_total,
    )
}

#[cfg(not(feature = "count-allocations"))]
fn steady<A: kernel::App>(_: &mut Screen, _: &mut Runner<A>) -> Option<u64> {
    None
}
