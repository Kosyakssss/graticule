use proptest::prelude::*;

use crate::ink::LIGHT;
use crate::metrics::Metrics;
use crate::pace::Pace;
use crate::paint::Painter;
use crate::text::Text;

#[derive(Clone, Copy, Debug)]
enum Happening {
    Changed,
    Unchanged,
    Redraw,
}

fn happening() -> impl Strategy<Value = Happening> {
    prop_oneof![Just(Happening::Changed), Just(Happening::Unchanged), Just(Happening::Redraw)]
}

proptest! {
    #[test]
    fn idle_is_zero(happenings in prop::collection::vec(happening(), 0..200)) {
        let mut pace = Pace::Idle;
        let (mut asked, mut changes, mut pending) = (0, 0, false);
        for h in happenings {
            match h {
                Happening::Changed => {
                    changes += 1;
                    if pace.want() {
                        prop_assert!(!pending);
                        asked += 1;
                        pending = true;
                    }
                }
                Happening::Unchanged => {}
                Happening::Redraw if pending => {
                    pace.drawn();
                    pending = false;
                }
                Happening::Redraw => {}
            }
        }
        prop_assert!(asked <= changes);
        if pending {
            pace.drawn();
        }
        prop_assert_eq!(pace, Pace::Idle);
    }
}

fn paint(quads: &mut Vec<crate::gpu::Quad>, text: &mut Text, m: Metrics) {
    quads.clear();
    text.begin_frame();
    let mut p = Painter::new(quads, text, m, *LIGHT);
    p.grid();
    let mono = p.m.mono();
    let sans = p.m.sans(400.0);
    p.text("Graticule ┌──┐ 格子", mono, (0.0, 0.0), None, p.theme.ink);
    p.text("A frame on the grid", sans, (0.0, 20.0), Some(80.0), p.theme.muted);
}

#[test]
fn no_allocation_per_frame() {
    let mut text = Text::new();
    let m = Metrics::measure(&mut text, 2.0, 1600, 1000);
    let mut quads = Vec::new();
    paint(&mut quads, &mut text, m);
    paint(&mut quads, &mut text, m);
    let info = allocation_counter::measure(|| paint(&mut quads, &mut text, m));
    assert_eq!(info.count_total, 0, "a steady frame allocated {} times", info.count_total);
}
