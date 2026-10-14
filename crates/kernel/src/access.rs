use accesskit::{Action, Node, NodeId, Rect as Bounds, Role, Toggled, TreeId, TreeInfo, TreeUpdate};

use crate::metrics::Metrics;
use crate::tree::{Id, Kind, NONE, Tree};

fn role(kind: Kind) -> Role {
    match kind {
        Kind::Root => Role::Window,
        Kind::Page => Role::Document,
        Kind::Row | Kind::Columns | Kind::Column | Kind::Empty => Role::Group,
        Kind::List { .. } => Role::List,
        Kind::ListRow { .. } => Role::ListItem,
        Kind::Field => Role::TextInput,
        Kind::Check { .. } => Role::CheckBox,
        Kind::Link => Role::Link,
        Kind::Button | Kind::Action => Role::Button,
        Kind::Text { .. } => Role::Label,
        Kind::Bar => Role::Toolbar,
        Kind::Status => Role::Status,
        Kind::Note => Role::Alert,
        Kind::Palette => Role::Dialog,
        Kind::Choice { .. } => Role::ListBoxOption,
        Kind::Swatch { .. } | Kind::Picture { .. } => Role::Image,
    }
}

/// The accessibility tree, made from the tree itself: every node with its role, its full label, its state and why it is unavailable.
pub fn update<C>(tree: &Tree<C>, m: Metrics, title: &str, focus: Option<Id>) -> TreeUpdate {
    let mut nodes = Vec::with_capacity(tree.nodes.len());
    for n in &tree.nodes {
        let mut node = Node::new(role(n.kind));
        let r = m.rect(n.area);
        node.set_bounds(Bounds { x0: r.x as f64, y0: r.y as f64, x1: r.right() as f64, y1: r.bottom() as f64 });
        let text = tree.str(n.text);
        let name = tree.str(n.name);
        match n.kind {
            Kind::Root => node.set_label(title),
            Kind::Field => {
                node.set_value(text);
                node.set_placeholder(name);
            }
            Kind::Check { on } => {
                node.set_label(text);
                node.set_toggled(if on { Toggled::True } else { Toggled::False });
                if !name.is_empty() {
                    node.set_description(name);
                }
            }
            Kind::ListRow { selected, .. } => node.set_selected(selected),
            Kind::Choice { chosen } => {
                node.set_label(text);
                node.set_selected(chosen);
            }
            _ if !name.is_empty() => node.set_label(name),
            _ if !text.is_empty() => node.set_label(text),
            _ => {}
        }
        if !n.reason.is_empty() {
            node.set_description(tree.str(n.reason));
        }
        if n.kind.acts() {
            if n.enabled {
                node.add_action(Action::Click);
                node.add_action(Action::Focus);
            } else {
                node.set_disabled();
            }
        }
        let mut children = Vec::new();
        let mut c = n.first;
        while c != NONE {
            children.push(NodeId(tree.nodes[c as usize].id.0));
            c = tree.nodes[c as usize].next;
        }
        node.set_children(children);
        nodes.push((NodeId(n.id.0), node));
    }
    let root = NodeId(Id::ROOT.0);
    let focus = focus.filter(|id| tree.find(*id).is_some()).map_or(root, |id| NodeId(id.0));
    TreeUpdate { nodes, tree: Some(TreeInfo::new(root)), tree_id: TreeId::ROOT, focus }
}
