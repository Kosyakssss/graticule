use crate::geometry::Bounds;

/// A line something snapped to: vertical at `at` when `vertical`, horizontal otherwise, in page units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guide {
    pub vertical: bool,
    pub at: f64,
}

/// How far to nudge a moving box so it lines up, and what it lined up with.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapped {
    pub dx: f64,
    pub dy: f64,
    pub guides: Vec<Guide>,
}

/// The smallest nudge, on each axis, that lines up an edge or the centre of `moving` with an edge or centre of one of `others`, if one is within `threshold` page units. Give the threshold in screen units divided by the zoom, so snapping feels the same at every zoom.
pub fn snap<'a>(moving: Bounds, others: impl IntoIterator<Item = &'a Bounds>, threshold: f64) -> Snapped {
    let lines = |b: &Bounds| ([b.x, b.x + b.w / 2.0, b.right()], [b.y, b.y + b.h / 2.0, b.bottom()]);
    let (mine_x, mine_y) = lines(&moving);
    let (mut best_x, mut best_y) = (None::<(f64, f64)>, None::<(f64, f64)>);
    let closer = |best: Option<(f64, f64)>, nudge: f64, at: f64| match best {
        Some((b, _)) if b.abs() <= nudge.abs() => best,
        _ if nudge.abs() <= threshold => Some((nudge, at)),
        _ => best,
    };
    for other in others {
        let (theirs_x, theirs_y) = lines(other);
        for (&m, &t) in mine_x.iter().flat_map(|m| theirs_x.iter().map(move |t| (m, t))) {
            best_x = closer(best_x, t - m, t);
        }
        for (&m, &t) in mine_y.iter().flat_map(|m| theirs_y.iter().map(move |t| (m, t))) {
            best_y = closer(best_y, t - m, t);
        }
    }
    let mut snapped = Snapped::default();
    if let Some((dx, at)) = best_x {
        snapped.dx = dx;
        snapped.guides.push(Guide { vertical: true, at });
    }
    if let Some((dy, at)) = best_y {
        snapped.dy = dy;
        snapped.guides.push(Guide { vertical: false, at });
    }
    snapped
}
