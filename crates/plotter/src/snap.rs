use crate::geometry::Bounds;

/// A line something snapped to: vertical at `at` when `vertical`, horizontal otherwise, in page units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guide {
    pub vertical: bool,
    pub at: f64,
}

/// One of the equal gaps a box snapped to: from `from` to `to` across the page when `across`, or down it otherwise, drawn at `at` on the other axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gap {
    pub across: bool,
    pub from: f64,
    pub to: f64,
    pub at: f64,
}

/// How far to nudge a moving box so it lines up, and what it lined up with.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapped {
    pub dx: f64,
    pub dy: f64,
    pub guides: Vec<Guide>,
    pub gaps: Vec<Gap>,
}

/// The smallest nudge, on each axis, that lines up an edge or the centre of `moving` with an edge or centre of one of `others`, or spaces it evenly with them, if one is within `threshold` page units. Spacing evenly puts the box midway between its nearest neighbours in a row or column, or one gap past the last of a run as wide as the gap between its last two. Give the threshold in screen units divided by the zoom, so snapping feels the same at every zoom.
pub fn snap<'a>(moving: Bounds, others: impl IntoIterator<Item = &'a Bounds>, threshold: f64) -> Snapped {
    let others: Vec<&Bounds> = others.into_iter().collect();
    let mut snapped = Snapped::default();
    for across in [true, false] {
        let nudge = match (lines(&moving, &others, across, threshold), spacing(&moving, &others, across, threshold)) {
            (Some((n, _)), Some((g, gaps))) if g.abs() < n.abs() => {
                snapped.gaps.extend(gaps);
                g
            }
            (Some((n, at)), _) => {
                snapped.guides.push(Guide { vertical: across, at });
                n
            }
            (None, Some((g, gaps))) => {
                snapped.gaps.extend(gaps);
                g
            }
            (None, None) => 0.0,
        };
        if across {
            snapped.dx = nudge;
        } else {
            snapped.dy = nudge;
        }
    }
    snapped
}

/// The nudges that line up the edges of a box being resized with the edges and centres of `others` within `threshold` page units: `x` is where the edge dragged across is, if one is, and `y` the one dragged down.
pub fn snap_edges<'a>(x: Option<f64>, y: Option<f64>, others: impl IntoIterator<Item = &'a Bounds>, threshold: f64) -> Snapped {
    let others: Vec<&Bounds> = others.into_iter().collect();
    let mut snapped = Snapped::default();
    for (edge, across) in [(x, true), (y, false)] {
        let Some(edge) = edge else { continue };
        let marks = |b: &Bounds| if across { [b.x, b.x + b.w / 2.0, b.right()] } else { [b.y, b.y + b.h / 2.0, b.bottom()] };
        let best = others.iter().flat_map(|o| marks(o)).map(|t| (t - edge, t)).filter(|(n, _)| n.abs() <= threshold).min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()));
        if let Some((nudge, at)) = best {
            snapped.guides.push(Guide { vertical: across, at });
            if across {
                snapped.dx = nudge;
            } else {
                snapped.dy = nudge;
            }
        }
    }
    snapped
}

fn lines(moving: &Bounds, others: &[&Bounds], across: bool, threshold: f64) -> Option<(f64, f64)> {
    let marks = |b: &Bounds| if across { [b.x, b.x + b.w / 2.0, b.right()] } else { [b.y, b.y + b.h / 2.0, b.bottom()] };
    let mut best = None::<(f64, f64)>;
    for other in others {
        for m in marks(moving) {
            for t in marks(other) {
                let nudge = t - m;
                if nudge.abs() <= threshold && best.is_none_or(|(b, _)| nudge.abs() < b.abs()) {
                    best = Some((nudge, t));
                }
            }
        }
    }
    best
}

/// The nudge that spaces `moving` evenly with the boxes beside it along one axis, with the gaps that are then equal.
fn spacing(moving: &Bounds, others: &[&Bounds], across: bool, threshold: f64) -> Option<(f64, Vec<Gap>)> {
    let span = |b: &Bounds| if across { (b.x, b.right()) } else { (b.y, b.bottom()) };
    let side = |b: &Bounds| if across { (b.y, b.bottom()) } else { (b.x, b.right()) };
    let beside = |a: &Bounds, b: &Bounds| side(a).0 < side(b).1 && side(b).0 < side(a).1;
    let middle = |a: &Bounds, b: &Bounds| (side(a).0.max(side(b).0) + side(a).1.min(side(b).1)) / 2.0;
    let (start, end) = span(moving);
    let centre = (start + end) / 2.0;
    let row: Vec<&Bounds> = others.iter().copied().filter(|b| beside(b, moving)).collect();
    let before = |limit: f64, of: &Bounds| row.iter().copied().filter(|b| span(b).1 <= limit && beside(b, of)).max_by(|a, b| span(a).1.total_cmp(&span(b).1));
    let after = |limit: f64, of: &Bounds| row.iter().copied().filter(|b| span(b).0 >= limit && beside(b, of)).min_by(|a, b| span(a).0.total_cmp(&span(b).0));
    let gap = |a: &Bounds, b: &Bounds, from: f64, to: f64| Gap { across, from, to, at: middle(a, b) };
    let mut options: Vec<(f64, Vec<Gap>)> = Vec::new();
    let left = row.iter().copied().filter(|b| (span(b).0 + span(b).1) / 2.0 < centre).max_by(|a, b| span(a).1.total_cmp(&span(b).1));
    let right = row.iter().copied().filter(|b| (span(b).0 + span(b).1) / 2.0 > centre).min_by(|a, b| span(a).0.total_cmp(&span(b).0));
    if let (Some(a), Some(b)) = (left, right) {
        let room = span(b).0 - span(a).1;
        if room > end - start {
            let to = span(a).1 + (room - (end - start)) / 2.0;
            options.push((to - start, vec![gap(a, moving, span(a).1, to), gap(moving, b, to + (end - start), span(b).0)]));
        }
    }
    if let Some(a) = left
        && let Some(z) = before(span(a).0, a)
    {
        let g = span(a).0 - span(z).1;
        let to = span(a).1 + g;
        options.push((to - start, vec![gap(z, a, span(z).1, span(a).0), gap(a, moving, span(a).1, to)]));
    }
    if let Some(b) = right
        && let Some(z) = after(span(b).1, b)
    {
        let g = span(z).0 - span(b).1;
        let to = span(b).0 - g - (end - start);
        options.push((to - start, vec![gap(moving, b, to + (end - start), span(b).0), gap(b, z, span(b).1, span(z).0)]));
    }
    options.into_iter().filter(|(n, _)| n.abs() <= threshold).min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()))
}
