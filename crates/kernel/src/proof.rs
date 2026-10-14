//! Proofs: a screen described as text, one component per line, so a change to it can be found and said in words.

use crate::grid::Area;
use crate::tree::{Kind, NONE, Node, Tone, Tree};

/// One component as a proof records it: how deep it sits, what it is, what it says, where it is on the grid and the state it is in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub depth: usize,
    pub role: String,
    pub label: String,
    pub area: Area,
    pub state: String,
}

/// Every component on screen, in tree order. Components with no room are left out, since nothing of them shows.
pub fn describe<C>(tree: &Tree<C>) -> Vec<Line> {
    let mut lines = Vec::new();
    if !tree.nodes.is_empty() {
        walk(tree, 0, 0, &mut lines);
    }
    lines
}

fn walk<C>(tree: &Tree<C>, i: u32, depth: usize, lines: &mut Vec<Line>) {
    let n = &tree.nodes[i as usize];
    if n.area.cols <= 0 || n.area.rows <= 0 {
        return;
    }
    let (text, name) = (tree.str(n.text), tree.str(n.name));
    let label = match n.kind {
        Kind::Field { .. } => name,
        _ if text.is_empty() => name,
        _ => text,
    };
    lines.push(Line { depth, role: n.kind.role().to_string(), label: label.to_string(), area: n.area, state: state(tree, n) });
    let mut c = n.first;
    while c != NONE {
        walk(tree, c, depth + 1, lines);
        c = tree.nodes[c as usize].next;
    }
}

fn state<C>(tree: &Tree<C>, n: &Node) -> String {
    let mut parts: Vec<String> = Vec::new();
    match n.kind {
        Kind::Text { mono, tone, strong } => {
            parts.extend(mono.then(|| "mono".to_string()));
            parts.extend(strong.then(|| "strong".to_string()));
            let tone = match tone {
                Tone::Ink => None,
                Tone::Muted => Some("muted"),
                Tone::Faint => Some("faint"),
                Tone::Danger => Some("danger"),
                Tone::Ok => Some("ok"),
                Tone::Warn => Some("warn"),
            };
            parts.extend(tone.map(str::to_string));
        }
        Kind::Field { .. } if !tree.str(n.text).is_empty() => parts.push(format!("holds “{}”", tree.str(n.text))),
        Kind::Check { on: true } => parts.push("ticked".to_string()),
        Kind::ListRow { selected, focused, .. } => {
            parts.extend(selected.then(|| "chosen".to_string()));
            parts.extend(focused.then(|| "focused".to_string()));
        }
        Kind::Choice { chosen: true } => parts.push("highlighted".to_string()),
        Kind::List { count, .. } => parts.push(format!("{count} items")),
        Kind::Gauge { filled, .. } => parts.push(format!("{} %", f32::from(filled) / 10.0)),
        Kind::Callout { ink } => parts.push(ink.name().to_string()),
        _ => {}
    }
    if n.kind.acts() && !n.enabled {
        let reason = tree.str(n.reason);
        parts.push(if reason.is_empty() { "unavailable".to_string() } else { format!("unavailable ({reason})") });
    }
    parts.join(", ")
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\t', "\\t").replace('\n', "\\n")
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match (c, c == '\\') {
            (_, true) => match chars.next() {
                Some('t') => out.push('\t'),
                Some('n') => out.push('\n'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            },
            (c, false) => out.push(c),
        }
    }
    out
}

/// The proof as text: one line per component, indented by depth, with its role, label, area (column, row, width, height) and state separated by tabs.
pub fn write(lines: &[Line]) -> String {
    let mut out = String::new();
    for l in lines {
        let a = l.area;
        out.push_str(&format!("{}{}\t{}\t{} {} {} {}\t{}\n", "  ".repeat(l.depth), l.role, escape(&l.label), a.col, a.row, a.cols, a.rows, escape(&l.state)));
    }
    out
}

/// Reads a proof written by `write`. Lines it can't read are skipped.
pub fn read(text: &str) -> Vec<Line> {
    text.lines()
        .filter_map(|line| {
            let body = line.trim_start_matches(' ');
            let depth = (line.len() - body.len()) / 2;
            let mut fields = body.split('\t');
            let role = fields.next()?.to_string();
            let label = unescape(fields.next()?);
            let numbers: Vec<i32> = fields.next()?.split(' ').map(|n| n.parse().ok()).collect::<Option<_>>()?;
            let [col, row, cols, rows] = numbers[..] else { return None };
            let state = unescape(fields.next().unwrap_or(""));
            Some(Line { depth, role, label, area: Area::new(col, row, cols, rows), state })
        })
        .collect()
}

/// What changed between two proofs of the same screen, in plain words, in the order the components appear. Nothing when they are the same.
pub fn changes(before: &[Line], after: &[Line]) -> Vec<String> {
    let key = |l: &Line| (l.depth, l.role.clone(), l.label.clone());
    let (n, m) = (before.len(), after.len());
    let mut common = vec![0u32; (n + 1) * (m + 1)];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            common[i * (m + 1) + j] = if key(&before[i]) == key(&after[j]) { common[(i + 1) * (m + 1) + j + 1] + 1 } else { common[(i + 1) * (m + 1) + j].max(common[i * (m + 1) + j + 1]) };
        }
    }
    let mut said = Vec::new();
    let mut moves: Vec<(String, i32, i32)> = Vec::new();
    let (mut gone, mut came): (Vec<&Line>, Vec<&Line>) = (Vec::new(), Vec::new());
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && key(&before[i]) == key(&after[j]) {
            settle(&mut gone, &mut came, &mut said);
            compare(&before[i], &after[j], &mut said, &mut moves);
            i += 1;
            j += 1;
        } else if j < m && (i == n || common[i * (m + 1) + j + 1] >= common[(i + 1) * (m + 1) + j]) {
            came.push(&after[j]);
            j += 1;
        } else {
            gone.push(&before[i]);
            i += 1;
        }
    }
    settle(&mut gone, &mut came, &mut said);
    said.extend(grouped(&moves));
    let mut once: Vec<(String, usize)> = Vec::new();
    for s in said {
        match once.iter_mut().find(|(seen, _)| *seen == s) {
            Some((_, count)) => *count += 1,
            None => once.push((s, 1)),
        }
    }
    once.into_iter().map(|(s, count)| if count > 1 { format!("{s}, in {count} places") } else { s }).collect()
}

fn named(l: &Line) -> String {
    if l.label.is_empty() {
        return format!("the {} at column {}, row {}", l.role, l.area.col, l.area.row);
    }
    let label: String = l.label.chars().take(40).collect();
    let more = if l.label.chars().count() > 40 { "…" } else { "" };
    format!("the {} “{label}{more}”", l.role)
}

fn plural(n: i32, one: &str) -> String {
    if n.abs() == 1 { format!("1 {one}") } else { format!("{} {one}s", n.abs()) }
}

fn settle(gone: &mut Vec<&Line>, came: &mut Vec<&Line>, said: &mut Vec<String>) {
    for g in gone.drain(..) {
        match came.iter().position(|c| c.role == g.role && c.depth == g.depth) {
            Some(k) => {
                let c = came.remove(k);
                said.push(format!("{} now says “{}”", named(g), c.label));
            }
            None => said.push(format!("{} is gone", named(g))),
        }
    }
    for c in came.drain(..) {
        said.push(format!("{} appeared at column {}, row {}", named(c), c.area.col, c.area.row));
    }
}

fn container(role: &str) -> bool {
    matches!(role, "group" | "column" | "page" | "list item" | "list" | "toolbar" | "status" | "alert")
}

fn compare(b: &Line, a: &Line, said: &mut Vec<String>, moves: &mut Vec<(String, i32, i32)>) {
    let name = named(a);
    if a.role == "window" {
        if (a.area.cols, a.area.rows) != (b.area.cols, b.area.rows) {
            said.push(format!("the window is now {} by {} cells, was {} by {}", a.area.cols, a.area.rows, b.area.cols, b.area.rows));
        }
    } else if !container(&a.role) {
        let (dc, dr) = (a.area.col - b.area.col, a.area.row - b.area.row);
        if (dc, dr) != (0, 0) {
            moves.push((name.clone(), dc, dr));
        }
        let mut size = Vec::new();
        let (dw, dh) = (a.area.cols - b.area.cols, a.area.rows - b.area.rows);
        if dw != 0 {
            size.push(format!("{} {}", plural(dw, "column"), if dw > 0 { "wider" } else { "narrower" }));
        }
        if dh != 0 {
            size.push(format!("{} {}", plural(dh, "row"), if dh > 0 { "taller" } else { "shorter" }));
        }
        if !size.is_empty() {
            said.push(format!("{name} is {}", size.join(" and ")));
        }
    }
    let parts = |s: &str| s.split(", ").filter(|p| !p.is_empty()).map(str::to_string).collect::<Vec<_>>();
    let (was, now) = (parts(&b.state), parts(&a.state));
    let lost: Vec<&String> = was.iter().filter(|p| !now.contains(p)).collect();
    let got: Vec<&String> = now.iter().filter(|p| !was.contains(p)).collect();
    let word = |ps: &[&String]| ps.iter().map(|p| p.as_str()).collect::<Vec<_>>().join(" and ");
    match (lost.is_empty(), got.is_empty()) {
        (true, true) => {}
        (_, false) => said.push(format!("{name} is now {}", word(&got))),
        (false, true) => said.push(format!("{name} is no longer {}", word(&lost))),
    }
}

fn grouped(moves: &[(String, i32, i32)]) -> Vec<String> {
    let mut said = Vec::new();
    let mut k = 0;
    while k < moves.len() {
        let (name, dc, dr) = &moves[k];
        let run = moves[k..].iter().take_while(|(_, c, r)| (c, r) == (dc, dr)).count();
        let mut way = Vec::new();
        if *dc != 0 {
            way.push(format!("{} {}", plural(*dc, "column"), if *dc > 0 { "right" } else { "left" }));
        }
        if *dr != 0 {
            way.push(format!("{} {}", plural(*dr, "row"), if *dr > 0 { "down" } else { "up" }));
        }
        let who = if run > 1 { format!("{name} and {} more after it", run - 1) } else { name.clone() };
        said.push(format!("{who} moved {}", way.join(" and ")));
        k += run;
    }
    said
}
