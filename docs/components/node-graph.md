# Node graph

Boxes joined by wires, set out by Graticule: in layers from left to right so the wires run forward, ordered within each layer so few of them cross. Each box holds whatever the app puts in it.

## Use it for

Showing how things lead to one another: the states a post moves through, the steps of a build, which tasks wait on which. Patchbay uses it for nodes the user wires by hand.

## Code

```rust
#[derived(Posts::graph, from = [states])]
graph: Derived<Graph>,

fn graph(states: &List<State>) -> Graph {
    Graph::new(states.len(), states.iter().enumerate().flat_map(|(k, s)| s.next.iter().map(move |&n| (k, n))))
}

if let Some(graph) = self.graph.get() {
    ui.graph(t!("How a post moves"), graph, |ui, k| {
        ui.text(&self.states[k].name);
        ui.muted(t!("{count} posts", count = self.states[k].count));
    });
}
```

`Graph::new` takes how many nodes there are and the edges between them, each from one node's number to another's, and works out where everything goes as soon as it is made. That work grows with the graph, so it belongs in a derived value, made on another thread whenever the facts it comes from change.

Every node is a box as wide as what it holds, on whole cells. A wire leaves a node's right edge, runs to a track of its own in the gap between two layers, turns, and enters the next node's left edge, where a small square marks its arrival. An edge that would close a loop is drawn against the flow, so its square sits on the right edge of the node it reaches. An edge that skips layers passes through them in a lane of its own.

## Mouse and keys

The graph itself is not pressed; what the app puts in each box is. A link or a button in a node is reached by Tab like any other. Moving between nodes along the wires by the arrow keys, dragging nodes and drawing wires by hand are not done.

## Narrow windows

A graph too wide for its place lists its nodes one under another, in the order of their numbers, without wires.

## Screen reader

The graph reads its label, then each node in the order of its numbers, each a group holding what the app put in it. The wires are not said yet.
