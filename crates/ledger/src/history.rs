use crate::op::Op;
use crate::path::Kind;

/// What became of a transaction once committed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kept {
    /// It changed the document or typed into a field, so it is a new step.
    Step,
    /// It continued the typing of the newest step, so it joined it.
    Merged,
    /// It only moved around: a selection changed. Not a step, but undo puts it back.
    Moved,
    /// It changed nothing that history keeps.
    Nothing,
}

/// One history per document, a tree rather than a line: undoing and then making a change starts a new branch, and the old one stays. `P` is where the user was (the kernel keeps its focus and caret there), so undo and redo can put them back.
pub struct History<P> {
    steps: Vec<Node<P>>,
    at: Option<usize>,
    moves: Vec<Op>,
}

struct Node<P> {
    parent: Option<usize>,
    newest: Option<usize>,
    label: String,
    ops: Vec<Op>,
    gesture: u64,
    typing: bool,
    moves_before: Vec<Op>,
    moves_after: Vec<Op>,
    before: P,
    after: Option<P>,
}

/// What undo or redo does: the operations to apply to the facts, in order, and where to put the user once they have landed.
pub struct Turn<P> {
    pub ops: Vec<Op>,
    pub place: P,
}

impl<P> Default for History<P> {
    fn default() -> Self {
        Self { steps: Vec::new(), at: None, moves: Vec::new() }
    }
}

impl<P: Clone> History<P> {
    pub fn new() -> Self {
        Self::default()
    }

    /// How many steps the tree holds, on every branch.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Takes a committed transaction's operations. `label` names the command that made them, `gesture` joins typing of one word into one step, and `place` is where the user was before it.
    pub fn commit(&mut self, ops: Vec<Op>, label: impl FnOnce() -> String, gesture: u64, place: P) -> Kept {
        if ops.is_empty() {
            return Kept::Nothing;
        }
        let typed = ops.iter().any(|op| matches!(op, Op::Splice { .. }));
        let step = typed || ops.iter().any(|op| op.path().kind() == Kind::Document);
        if !step {
            let gone: Vec<u128> = ops.iter().filter_map(Op::deleted).collect();
            if !gone.is_empty() {
                self.forget(&gone);
            }
            let moved = ops.iter().any(|op| matches!(op, Op::Select { .. }) && !names(op, &gone));
            self.moves.extend(ops.into_iter().filter(|op| matches!(op, Op::Select { .. }) && !names(op, &gone)));
            return if moved { Kept::Moved } else { Kept::Nothing };
        }
        let only_typing = ops.iter().all(|op| matches!(op, Op::Splice { .. } | Op::Select { .. }));
        if let Some(at) = self.at
            && only_typing
            && self.moves.is_empty()
            && self.steps[at].typing
            && self.steps[at].gesture == gesture
            && self.steps[at].newest.is_none()
        {
            self.steps[at].ops.extend(ops);
            return Kept::Merged;
        }
        let index = self.steps.len();
        if let Some(parent) = self.at {
            self.steps[parent].newest = Some(index);
        }
        self.steps.push(Node {
            parent: self.at,
            newest: None,
            label: label(),
            ops,
            gesture,
            typing: only_typing,
            moves_before: std::mem::take(&mut self.moves),
            moves_after: Vec::new(),
            before: place,
            after: None,
        });
        self.at = Some(index);
        Kept::Step
    }

    fn forget(&mut self, gone: &[u128]) {
        self.moves.retain(|op| !names(op, gone));
        for step in &mut self.steps {
            step.moves_before.retain(|op| !names(op, gone));
            step.moves_after.retain(|op| !names(op, gone));
        }
    }

    /// How many steps undo can take back from here, along the current branch.
    pub fn depth(&self) -> usize {
        std::iter::successors(self.at, |&at| self.steps[at].parent).count()
    }

    /// The label of the step undo would take back.
    pub fn undo_label(&self) -> Option<&str> {
        self.at.map(|at| self.steps[at].label.as_str())
    }

    /// The label of the step redo would put back: the newest branch from here.
    pub fn redo_label(&self) -> Option<&str> {
        self.next().map(|n| self.steps[n].label.as_str())
    }

    fn next(&self) -> Option<usize> {
        match self.at {
            Some(at) => self.steps[at].newest,
            None => self.steps.iter().rposition(|n| n.parent.is_none()),
        }
    }

    /// Takes back the newest step of the current branch: the selection moves since, the step's own changes, then the moves before it, so once its operations land the facts are as they were just before the step. `place` is where the user is now, kept for redo.
    pub fn undo(&mut self, place: P) -> Option<Turn<P>> {
        let at = self.at?;
        let trailing = std::mem::take(&mut self.moves);
        let mut ops = Vec::new();
        reverse(&trailing, &mut ops);
        let node = &mut self.steps[at];
        reverse(&node.ops, &mut ops);
        node.moves_after = trailing;
        node.after = Some(place);
        self.moves = node.moves_before.clone();
        self.at = node.parent;
        Some(Turn { ops, place: self.steps[at].before.clone() })
    }

    /// Puts back the newest step undone from here, and the selection as it was when it was undone.
    pub fn redo(&mut self) -> Option<Turn<P>> {
        let next = self.next()?;
        let trailing = std::mem::take(&mut self.moves);
        let mut ops = Vec::new();
        reverse(&trailing, &mut ops);
        let node = &self.steps[next];
        for list in [&node.moves_before, &node.ops, &node.moves_after] {
            ops.extend(list.iter().cloned());
        }
        self.moves = node.moves_after.clone();
        self.at = Some(next);
        let node = &self.steps[next];
        Some(Turn { ops, place: node.after.clone().unwrap_or_else(|| node.before.clone()) })
    }
}

fn reverse(ops: &[Op], out: &mut Vec<Op>) {
    out.extend(ops.iter().rev().map(Op::inverse));
}

fn names(op: &Op, gone: &[u128]) -> bool {
    fn mentions(value: &crate::Value, gone: &[u128]) -> bool {
        match value {
            crate::Value::Key(k) => gone.contains(k),
            crate::Value::List(items) => items.iter().any(|v| mentions(v, gone)),
            crate::Value::Some(v) => mentions(v, gone),
            _ => false,
        }
    }
    matches!(op, Op::Select { before, after, .. } if mentions(before, gone) || mentions(after, gone))
}
