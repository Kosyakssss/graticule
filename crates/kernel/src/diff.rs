//! Diffs: what changed between two texts, line by line and, inside a changed line, word by word.

use std::collections::HashMap;
use std::sync::Arc;

/// What a line of a diff is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Change {
    Same,
    Removed,
    Added,
    /// A run of unchanged lines far from any change, shown as one line that says how many.
    Folded,
}

/// One line of a diff: what it is, its numbers in the old and new texts (0 where it has none; for a folded line, how many lines it stands for), its text, and the bytes of it that changed within the line.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Line {
    pub change: Change,
    pub old: u32,
    pub new: u32,
    pub text: String,
    pub marks: Vec<(u32, u32)>,
}

/// What changed between two texts: worked out as soon as it is made, line by line and, inside a line that changed, word by word, with long runs of unchanged lines folded away. The work grows with the texts, so a diff of large ones is made in a derived value, off the main thread. `ui.diff(t!("Changes"), &self.diff, 20)` shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Diff {
    lines: Arc<Vec<Line>>,
    added: u32,
    removed: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    Same(usize, usize),
    Removed(usize),
    Added(usize),
}

/// How many unchanged lines are kept around each change.
const CONTEXT: usize = 3;
type Marks = Vec<(u32, u32)>;

/// How far down the removed lines an added line looks for the one it replaced.
const NEAR: usize = 8;
/// How many changes the search goes through before it shows what is left as all removed, then all added.
const MOST: isize = 1024;

impl Diff {
    /// What changed from `before` to `after`.
    pub fn new(before: &str, after: &str) -> Diff {
        let (old, new): (Vec<&str>, Vec<&str>) = (before.lines().collect(), after.lines().collect());
        let mut ids = HashMap::new();
        let mut id = |s: &str| {
            let next = ids.len() as u32;
            *ids.entry(s.to_string()).or_insert(next)
        };
        let (a, b): (Vec<u32>, Vec<u32>) = (old.iter().map(|s| id(s)).collect(), new.iter().map(|s| id(s)).collect());
        let ops = script(&a, &b);
        let mut lines = Vec::with_capacity(ops.len());
        let (mut added, mut removed) = (0, 0);
        let mut k = 0;
        while k < ops.len() {
            if let Op::Same(i, j) = ops[k] {
                lines.push(Line { change: Change::Same, old: i as u32 + 1, new: j as u32 + 1, text: old[i].to_string(), marks: Vec::new() });
                k += 1;
                continue;
            }
            let end = ops[k..].iter().position(|o| matches!(o, Op::Same(..))).map_or(ops.len(), |p| k + p);
            let gone: Vec<usize> = ops[k..end].iter().filter_map(|o| if let Op::Removed(i) = *o { Some(i) } else { None }).collect();
            let came: Vec<usize> = ops[k..end].iter().filter_map(|o| if let Op::Added(j) = *o { Some(j) } else { None }).collect();
            let (mut gone_marks, mut came_marks) = (vec![Vec::new(); gone.len()], vec![Vec::new(); came.len()]);
            let mut from = 0;
            for (n, &j) in came.iter().enumerate() {
                let found = (from..gone.len().min(from + NEAR)).find_map(|g| words(old[gone[g]], new[j]).map(|m| (g, m)));
                if let Some((g, (a, b))) = found {
                    (gone_marks[g], came_marks[n], from) = (a, b, g + 1);
                }
            }
            for (&i, marks) in gone.iter().zip(gone_marks) {
                lines.push(Line { change: Change::Removed, old: i as u32 + 1, new: 0, text: old[i].to_string(), marks });
            }
            for (&j, marks) in came.iter().zip(came_marks) {
                lines.push(Line { change: Change::Added, old: 0, new: j as u32 + 1, text: new[j].to_string(), marks });
            }
            (removed, added) = (removed + gone.len() as u32, added + came.len() as u32);
            k = end;
        }
        Diff { lines: Arc::new(folded(lines)), added, removed }
    }

    /// How many lines were added.
    pub fn added(&self) -> usize {
        self.added as usize
    }

    /// How many lines were removed.
    pub fn removed(&self) -> usize {
        self.removed as usize
    }

    /// How many lines it shows, folded ones counted once.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub(crate) fn lines(&self) -> &[Line] {
        &self.lines
    }
}

/// The lines with each run of unchanged lines more than `CONTEXT` away from a change folded into one.
fn folded(lines: Vec<Line>) -> Vec<Line> {
    let changed: Vec<usize> = lines.iter().enumerate().filter(|(_, l)| l.change != Change::Same).map(|(k, _)| k).collect();
    let near = |k: usize| {
        let at = changed.partition_point(|&c| c < k);
        changed.get(at).is_some_and(|&c| c - k <= CONTEXT) || at > 0 && k - changed[at - 1] <= CONTEXT
    };
    let keep: Vec<bool> = (0..lines.len()).map(|k| lines[k].change != Change::Same || near(k)).collect();
    let mut out = Vec::with_capacity(lines.len());
    let mut k = 0;
    let mut lines = lines.into_iter().map(Some).collect::<Vec<_>>();
    while k < lines.len() {
        let run = keep[k..].iter().position(|&kept| kept).unwrap_or(keep.len() - k);
        if run > 1 {
            out.push(Line { change: Change::Folded, old: run as u32, new: 0, text: String::new(), marks: Vec::new() });
            k += run;
        } else if let Some(line) = lines[k].take() {
            out.push(line);
            k += 1;
        }
    }
    out
}

/// The bytes of `old` and of `new` that changed, word by word; nothing when too little of the line stayed for the two to be one line changed.
fn words(old: &str, new: &str) -> Option<(Marks, Marks)> {
    let (a, b) = (tokens(old), tokens(new));
    let ops = script(&a.iter().map(|r| &old[r.0..r.1]).collect::<Vec<_>>(), &b.iter().map(|r| &new[r.0..r.1]).collect::<Vec<_>>());
    let kept: usize = ops.iter().map(|o| if let Op::Same(i, _) = *o { a[i].1 - a[i].0 } else { 0 }).sum();
    if kept * 2 < old.len().max(new.len()) {
        return None;
    }
    let mut marks = (Marks::new(), Marks::new());
    let push = |list: &mut Vec<(u32, u32)>, (s, e): (usize, usize)| match list.last_mut() {
        Some(last) if last.1 == s as u32 => last.1 = e as u32,
        _ => list.push((s as u32, e as u32)),
    };
    for o in ops {
        match o {
            Op::Removed(i) => push(&mut marks.0, a[i]),
            Op::Added(j) => push(&mut marks.1, b[j]),
            Op::Same(..) => {}
        }
    }
    Some(marks)
}

/// A line cut into words, runs of spaces, and single other characters, as byte ranges.
fn tokens(s: &str) -> Vec<(usize, usize)> {
    let class = |c: char| {
        if c.is_alphanumeric() || c == '_' {
            0
        } else if c.is_whitespace() {
            1
        } else {
            2
        }
    };
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut last = None;
    for (i, c) in s.char_indices() {
        let k = class(c);
        match out.last_mut() {
            Some(t) if last == Some(k) && k != 2 => t.1 = i + c.len_utf8(),
            _ => out.push((i, i + c.len_utf8())),
        }
        last = Some(k);
    }
    out
}

/// The shortest way from `a` to `b`, found by Myers's search after the common start and end are set aside.
fn script<T: PartialEq>(a: &[T], b: &[T]) -> Vec<Op> {
    let start = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let end = a[start..].iter().rev().zip(b[start..].iter().rev()).take_while(|(x, y)| x == y).count();
    let mut ops: Vec<Op> = (0..start).map(|i| Op::Same(i, i)).collect();
    middle(&a[start..a.len() - end], &b[start..b.len() - end], start, start, &mut ops);
    let (ea, eb) = (a.len() - end, b.len() - end);
    ops.extend((0..end).map(|k| Op::Same(ea + k, eb + k)));
    ops
}

fn middle<T: PartialEq>(a: &[T], b: &[T], oa: usize, ob: usize, ops: &mut Vec<Op>) {
    let (n, m) = (a.len() as isize, b.len() as isize);
    let max = n + m;
    let mut v = vec![0isize; 2 * max as usize + 3];
    let at = |k: isize| (k + max + 1) as usize;
    let mut trace: Vec<Vec<isize>> = Vec::new();
    let mut reached = false;
    for d in 0..=max.min(MOST) {
        for k in (-d..=d).step_by(2) {
            let mut x = if k == -d || k != d && v[at(k - 1)] < v[at(k + 1)] { v[at(k + 1)] } else { v[at(k - 1)] + 1 };
            let mut y = x - k;
            while x < n && y < m && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }
            v[at(k)] = x;
            if x >= n && y >= m {
                reached = true;
            }
        }
        trace.push(v[at(-d)..=at(d)].to_vec());
        if reached {
            break;
        }
    }
    if !reached {
        ops.extend((0..a.len()).map(|i| Op::Removed(oa + i)));
        ops.extend((0..b.len()).map(|j| Op::Added(ob + j)));
        return;
    }
    let mut back = Vec::new();
    let (mut x, mut y) = (n, m);
    for d in (1..trace.len() as isize).rev() {
        let prev = &trace[d as usize - 1];
        let was = |k: isize| prev[(k + d - 1) as usize];
        let k = x - y;
        let from = if k == -d || k != d && was(k - 1) < was(k + 1) { k + 1 } else { k - 1 };
        let (px, py) = (was(from), was(from) - from);
        while x > px && y > py {
            back.push(Op::Same(oa + x as usize - 1, ob + y as usize - 1));
            (x, y) = (x - 1, y - 1);
        }
        back.push(if from == k + 1 { Op::Added(ob + py as usize) } else { Op::Removed(oa + px as usize) });
        (x, y) = (px, py);
    }
    while x > 0 && y > 0 {
        back.push(Op::Same(oa + x as usize - 1, ob + y as usize - 1));
        (x, y) = (x - 1, y - 1);
    }
    ops.extend(back.into_iter().rev());
}
