//! Node graphs: where each node goes, worked out in layers from left to right so wires run forward and few of them cross.

use std::sync::Arc;

/// A place in a layer: a node, or a wire passing through on its way to a later layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Slot {
    Node(u32),
    Through,
}

/// A wire between two nodes, as the places it passes from left to right, each a layer and a place in it. `back` when the edge runs against the flow, so it arrives at its left end.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Wire {
    pub path: Vec<(u32, u32)>,
    pub back: bool,
}

#[derive(Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct Shape {
    pub count: u32,
    pub layers: Vec<Vec<Slot>>,
    pub wires: Vec<Wire>,
}

/// A graph's nodes, numbered from 0, and its edges, with where each node goes worked out as soon as it is made: in layers from left to right so edges run forward, and in an order within each layer that keeps wires from crossing. The work grows with the graph, so a large one is made in a derived value, off the main thread. `ui.graph(t!("Build"), &self.graph, |ui, k| ui.text(&steps[k].name))` shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Graph {
    shape: Arc<Shape>,
}

impl Graph {
    /// `count` nodes and the edges between them, each from one node's number to another's. Edges from a node to itself, or to a number past the last node, are left out; an edge that closes a loop is drawn against the flow.
    pub fn new(count: usize, edges: impl IntoIterator<Item = (usize, usize)>) -> Graph {
        let mut edges: Vec<(usize, usize)> = edges.into_iter().filter(|&(a, b)| a < count && b < count && a != b).collect();
        edges.sort_unstable();
        edges.dedup();
        let back = loops(count, &edges);
        let forward: Vec<(usize, usize)> = edges.iter().zip(&back).map(|(&(a, b), &back)| if back { (b, a) } else { (a, b) }).collect();
        let layer = layers(count, &forward);
        let mut items: Vec<(u32, Slot)> = (0..count).map(|v| (layer[v], Slot::Node(v as u32))).collect();
        let mut paths = Vec::with_capacity(forward.len());
        for &(a, b) in &forward {
            let mut path = vec![a];
            for l in layer[a] + 1..layer[b] {
                path.push(items.len());
                items.push((l, Slot::Through));
            }
            path.push(b);
            paths.push(path);
        }
        let depth = items.iter().map(|i| i.0 as usize + 1).max().unwrap_or(0);
        let order = ordered(&items, &paths, depth);
        let mut place = vec![0u32; items.len()];
        let layers = order
            .iter()
            .map(|row| {
                for (k, &it) in row.iter().enumerate() {
                    place[it] = k as u32;
                }
                row.iter().map(|&it| items[it].1).collect()
            })
            .collect();
        let wires = paths.iter().zip(&back).map(|(path, &back)| Wire { path: path.iter().map(|&it| (items[it].0, place[it])).collect(), back }).collect();
        Graph { shape: Arc::new(Shape { count: count as u32, layers, wires }) }
    }

    pub fn len(&self) -> usize {
        self.shape.count as usize
    }

    pub fn is_empty(&self) -> bool {
        self.shape.count == 0
    }

    pub(crate) fn shape(&self) -> &Arc<Shape> {
        &self.shape
    }
}

/// Which edges close a loop, found by walking the graph depth first: those that lead back to a node still being walked.
fn loops(count: usize, edges: &[(usize, usize)]) -> Vec<bool> {
    let mut out = vec![Vec::new(); count];
    for (k, &(a, _)) in edges.iter().enumerate() {
        out[a].push(k);
    }
    let (mut state, mut back) = (vec![0u8; count], vec![false; edges.len()]);
    for root in 0..count {
        if state[root] != 0 {
            continue;
        }
        state[root] = 1;
        let mut stack = vec![(root, 0usize)];
        while let Some(top) = stack.last_mut() {
            let (v, next) = *top;
            match out[v].get(next) {
                Some(&e) => {
                    top.1 += 1;
                    let w = edges[e].1;
                    match state[w] {
                        0 => {
                            state[w] = 1;
                            stack.push((w, 0));
                        }
                        1 => back[e] = true,
                        _ => {}
                    }
                }
                None => {
                    state[v] = 2;
                    stack.pop();
                }
            }
        }
    }
    back
}

/// Each node's layer: one past the furthest of the nodes that lead to it, and a node that nothing leads to sits just before the nearest node it leads to.
fn layers(count: usize, edges: &[(usize, usize)]) -> Vec<u32> {
    let (mut into, mut next, mut prev) = (vec![0usize; count], vec![Vec::new(); count], vec![Vec::new(); count]);
    for &(a, b) in edges {
        into[b] += 1;
        next[a].push(b);
        prev[b].push(a);
    }
    let mut queue: Vec<usize> = (0..count).filter(|&v| into[v] == 0).collect();
    let mut layer = vec![0u32; count];
    let mut head = 0;
    while let Some(&v) = queue.get(head) {
        head += 1;
        for &w in &next[v] {
            layer[w] = layer[w].max(layer[v] + 1);
            into[w] -= 1;
            if into[w] == 0 {
                queue.push(w);
            }
        }
    }
    for &v in queue.iter().rev() {
        if prev[v].is_empty()
            && let Some(nearest) = next[v].iter().map(|&w| layer[w]).min()
        {
            layer[v] = nearest.saturating_sub(1);
        }
    }
    layer
}

/// The items of each layer in order, top to bottom: each placed by the average place of its neighbours in the layer before, then the layer after, a few times over.
fn ordered(items: &[(u32, Slot)], paths: &[Vec<usize>], depth: usize) -> Vec<Vec<usize>> {
    let mut order = vec![Vec::new(); depth];
    for (it, &(l, _)) in items.iter().enumerate() {
        order[l as usize].push(it);
    }
    let (mut before, mut after) = (vec![Vec::new(); items.len()], vec![Vec::new(); items.len()]);
    for path in paths {
        for pair in path.windows(2) {
            after[pair[0]].push(pair[1]);
            before[pair[1]].push(pair[0]);
        }
    }
    let mut place = vec![0f64; items.len()];
    let settle = |row: &mut Vec<usize>, place: &mut Vec<f64>, by: &[Vec<usize>]| {
        let mut keyed: Vec<(f64, usize)> = row
            .iter()
            .map(|&it| {
                let near = &by[it];
                let at = if near.is_empty() { place[it] } else { near.iter().map(|&n| place[n]).sum::<f64>() / near.len() as f64 };
                (at, it)
            })
            .collect();
        keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
        *row = keyed.into_iter().map(|k| k.1).collect();
        for (k, &it) in row.iter().enumerate() {
            place[it] = k as f64;
        }
    };
    for row in &order {
        for (k, &it) in row.iter().enumerate() {
            place[it] = k as f64;
        }
    }
    for _ in 0..4 {
        for row in order.iter_mut().skip(1) {
            settle(row, &mut place, &before);
        }
        for row in order.iter_mut().rev().skip(1) {
            settle(row, &mut place, &after);
        }
    }
    order
}
