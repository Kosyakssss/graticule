/// What a command did to the facts. A command that can no longer apply returns `Nothing`, and Ledger doesn't record it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[must_use]
pub enum Outcome {
    Changed,
    Nothing,
}

impl Outcome {
    pub fn changed(self) -> bool {
        self == Outcome::Changed
    }

    pub fn or(self, other: Outcome) -> Outcome {
        if self.changed() || other.changed() { Outcome::Changed } else { Outcome::Nothing }
    }

    pub fn when(changed: bool) -> Outcome {
        if changed { Outcome::Changed } else { Outcome::Nothing }
    }
}
