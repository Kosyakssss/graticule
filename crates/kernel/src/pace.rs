/// Whether a frame is owed. A frame is asked for only when something changed, and many changes between two frames make one frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pace {
    #[default]
    Idle,
    Wanted,
}

impl Pace {
    pub fn want(&mut self) -> bool {
        let ask = *self == Pace::Idle;
        *self = Pace::Wanted;
        ask
    }

    pub fn drawn(&mut self) {
        *self = Pace::Idle;
    }
}
