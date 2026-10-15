use accesskit::{Action, Node, NodeId, Rect as Bounds, Role, Toggled, TreeId, TreeInfo, TreeUpdate};

use crate::metrics::Metrics;
use crate::tree::{Holds, Id, Kind, NONE, Tree};

fn role(kind: Kind) -> Role {
    match kind {
        Kind::Root => Role::Window,
        Kind::Page => Role::Document,
        Kind::Row | Kind::Columns | Kind::Column | Kind::Empty => Role::Group,
        Kind::List { .. } => Role::List,
        Kind::ListRow { .. } => Role::ListItem,
        Kind::Field { holds: Holds::Lines { .. } } => Role::MultilineTextInput,
        Kind::Field { holds: Holds::Search } => Role::SearchInput,
        Kind::Field { holds: Holds::Number } => Role::SpinButton,
        Kind::Field { holds: Holds::Date } => Role::DateInput,
        Kind::Field { holds: Holds::Time } => Role::TimeInput,
        Kind::Calendar { .. } => Role::Grid,
        Kind::Day { .. } => Role::GridCell,
        Kind::Turn { .. } => Role::Button,
        Kind::Slider { .. } | Kind::Thumb { .. } => Role::Slider,
        Kind::Range { .. } => Role::Group,
        Kind::Cycle { .. } | Kind::Browse | Kind::Chip => Role::Button,
        Kind::Select { .. } => Role::ComboBox,
        Kind::Pick { .. } => Role::ListBoxOption,
        Kind::Field { .. } => Role::TextInput,
        Kind::Check { .. } => Role::CheckBox,
        Kind::Link => Role::Link,
        Kind::Button | Kind::Action => Role::Button,
        Kind::Text { strong: true, .. } => Role::Heading,
        Kind::Text { .. } | Kind::Value { .. } | Kind::Tag | Kind::Keys => Role::Label,
        Kind::Rule | Kind::Placeholder { .. } => Role::GenericContainer,
        Kind::Gauge { meter: false, .. } => Role::ProgressIndicator,
        Kind::Gauge { meter: true, .. } => Role::Meter,
        Kind::Avatar => Role::Image,
        Kind::Callout { ink: crate::ink::Ink::Danger | crate::ink::Ink::Warn } => Role::Alert,
        Kind::Callout { .. } => Role::Note,
        Kind::Card | Kind::Group | Kind::Fieldset | Kind::Labelled => Role::Group,
        Kind::Toggle { .. } | Kind::IconButton { .. } => Role::Button,
        Kind::Radio { .. } => Role::RadioButton,
        Kind::Tabs | Kind::Disclosure => Role::Group,
        Kind::Strip => Role::TabList,
        Kind::Tab { .. } => Role::Tab,
        Kind::Place { .. } | Kind::PageLink { .. } => Role::Link,
        Kind::Crumbs | Kind::Places => Role::Navigation,
        Kind::Opener { .. } | Kind::More => Role::Button,
        Kind::MenuButton | Kind::Popover { .. } | Kind::Confirm { open: false } => Role::Button,
        Kind::Confirm { open: true } => Role::AlertDialog,
        Kind::Overflow => Role::Toolbar,
        Kind::Switch { .. } => Role::Switch,
        Kind::Bar => Role::Toolbar,
        Kind::Status => Role::Status,
        Kind::Note => Role::Alert,
        Kind::Palette => Role::Dialog,
        Kind::Overlay { .. } => Role::Menu,
        Kind::Entry => Role::MenuItem,
        Kind::Choice { .. } => Role::ListBoxOption,
        Kind::Swatch { .. } | Kind::Picture { .. } => Role::Image,
    }
}

/// The accessibility tree, made from the tree itself: every node with its role, its full label, its state and why it is unavailable.
pub fn update<C>(tree: &Tree<C>, m: Metrics, title: &str, focus: Option<Id>) -> TreeUpdate {
    let mut nodes = Vec::with_capacity(tree.nodes.len());
    for (i, n) in tree.nodes.iter().enumerate() {
        let mut node = Node::new(role(n.kind));
        let r = m.rect(n.area);
        node.set_bounds(Bounds { x0: r.x as f64, y0: r.y as f64, x1: r.right() as f64, y1: r.bottom() as f64 });
        let text = tree.str(n.text);
        let name = tree.str(n.name);
        match n.kind {
            Kind::Root => node.set_label(title),
            Kind::Slider { .. } | Kind::Thumb { .. } | Kind::Field { holds: Holds::Number } => {
                node.set_value(text);
                if !name.is_empty() {
                    node.set_label(name);
                }
                if let Ok(value) = crate::field::evaluate(text) {
                    node.set_numeric_value(value);
                }
                let held = if matches!(n.kind, Kind::Thumb { .. }) { n.parent } else { i as u32 };
                if let Some((least, most)) = tree.bounds(held) {
                    node.set_min_numeric_value(least as f64);
                    node.set_max_numeric_value(most as f64);
                }
            }
            Kind::Field { .. } => {
                node.set_value(text);
                node.set_placeholder(name);
            }
            Kind::Tab { on } | Kind::Place { on } | Kind::PageLink { on } => {
                node.set_label(if name.is_empty() { text } else { name });
                node.set_selected(on);
            }
            Kind::Select { open, .. } => {
                node.set_label(name);
                node.set_value(text);
                node.set_expanded(open);
            }
            Kind::Day { on, .. } => {
                node.set_label(name);
                node.set_selected(on);
            }
            Kind::Pick { on } => {
                node.set_label(text);
                node.set_selected(on);
            }
            Kind::Popover { open } => {
                node.set_label(text);
                node.set_expanded(open);
                node.set_has_popup(accesskit::HasPopup::Dialog);
            }
            Kind::MenuButton => {
                node.set_label(text);
                node.set_has_popup(accesskit::HasPopup::Menu);
            }
            Kind::Opener { open } => {
                node.set_label(text);
                node.set_expanded(open);
            }
            Kind::Toggle { on } | Kind::Radio { on } | Kind::Switch { on } => {
                node.set_label(if name.is_empty() { text } else { name });
                node.set_toggled(if on { Toggled::True } else { Toggled::False });
            }
            Kind::Check { on } => {
                node.set_label(text);
                node.set_toggled(if on { Toggled::True } else { Toggled::False });
                if !name.is_empty() {
                    node.set_description(name);
                }
            }
            Kind::ListRow { selected, .. } => node.set_selected(selected),
            Kind::Gauge { filled, .. } => {
                node.set_label(text);
                node.set_numeric_value(f64::from(filled) / 10.0);
                node.set_min_numeric_value(0.0);
                node.set_max_numeric_value(100.0);
            }
            Kind::Placeholder { .. } => node.set_label("Loading"),
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
        if n.kind.acts() && n.parent != NONE && tree.nodes[n.parent as usize].kind == Kind::Labelled {
            let around = tree.nodes[n.parent as usize];
            node.set_label(tree.str(around.text));
            let mut said = Vec::new();
            let mut c = around.first;
            while c != NONE {
                let s = tree.nodes[c as usize];
                if matches!(s.kind, Kind::Text { tone: crate::tree::Tone::Muted, .. } | Kind::Value { value: crate::value::Value::Error }) {
                    said.push(tree.str(s.text));
                }
                c = s.next;
            }
            if !said.is_empty() {
                node.set_description(said.join(". "));
            }
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
