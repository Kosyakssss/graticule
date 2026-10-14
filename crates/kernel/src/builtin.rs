use crate::keys::{Chord, Key};
use crate::look::{Choice, Switch};
use crate::shell::Mods;

/// The kernel's own commands. They sit in the registry beside the app's, so they show in the palette and which-key with labels and reasons; the platform's conventions bind them, and a keymap file can bind them too, by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Builtin {
    Palette,
    Undo,
    Redo,
    Next,
    Previous,
    Up,
    Down,
    ExtendUp,
    ExtendDown,
    First,
    Last,
    SelectAll,
    Cancel,
    LightAppearance,
    DarkAppearance,
    SystemAppearance,
    NextTheme,
    NextStyle,
}

impl Builtin {
    pub const ALL: [Builtin; 18] = [
        Builtin::Palette,
        Builtin::Undo,
        Builtin::Redo,
        Builtin::Next,
        Builtin::Previous,
        Builtin::Up,
        Builtin::Down,
        Builtin::ExtendUp,
        Builtin::ExtendDown,
        Builtin::First,
        Builtin::Last,
        Builtin::SelectAll,
        Builtin::Cancel,
        Builtin::LightAppearance,
        Builtin::DarkAppearance,
        Builtin::SystemAppearance,
        Builtin::NextTheme,
        Builtin::NextStyle,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Builtin::Palette => "palette",
            Builtin::Undo => "undo",
            Builtin::Redo => "redo",
            Builtin::Next => "next-control",
            Builtin::Previous => "previous-control",
            Builtin::Up => "up",
            Builtin::Down => "down",
            Builtin::ExtendUp => "extend-up",
            Builtin::ExtendDown => "extend-down",
            Builtin::First => "first",
            Builtin::Last => "last",
            Builtin::SelectAll => "select-all",
            Builtin::Cancel => "cancel",
            Builtin::LightAppearance => "light-appearance",
            Builtin::DarkAppearance => "dark-appearance",
            Builtin::SystemAppearance => "system-appearance",
            Builtin::NextTheme => "next-theme",
            Builtin::NextStyle => "next-style",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Builtin::Palette => "Open the palette",
            Builtin::Undo => "Undo",
            Builtin::Redo => "Redo",
            Builtin::Next => "Go to the next control",
            Builtin::Previous => "Go to the previous control",
            Builtin::Up => "Go up the list",
            Builtin::Down => "Go down the list",
            Builtin::ExtendUp => "Choose up the list too",
            Builtin::ExtendDown => "Choose down the list too",
            Builtin::First => "Go to the top of the list",
            Builtin::Last => "Go to the bottom of the list",
            Builtin::SelectAll => "Choose everything in the list",
            Builtin::Cancel => "Cancel",
            Builtin::LightAppearance => "Use the light appearance",
            Builtin::DarkAppearance => "Use the dark appearance",
            Builtin::SystemAppearance => "Follow the system's appearance",
            Builtin::NextTheme => "Use the next theme",
            Builtin::NextStyle => "Use the next style",
        }
    }

    pub fn find(name: &str) -> Option<Builtin> {
        Builtin::ALL.into_iter().find(|b| b.name() == name)
    }

    /// The platform's own keys for it, which every app gets.
    pub fn convention(self) -> &'static [Chord] {
        match self {
            Builtin::Undo => {
                const K: &[Chord] = &[c(Key::Char('z'), false, true)];
                K
            }
            Builtin::Redo if cfg!(target_os = "macos") => {
                const K: &[Chord] = &[c(Key::Char('z'), true, true)];
                K
            }
            Builtin::Redo => {
                const K: &[Chord] = &[c(Key::Char('z'), true, true), c(Key::Char('y'), false, true)];
                K
            }
            Builtin::Next => {
                const K: &[Chord] = &[c(Key::Tab, false, false)];
                K
            }
            Builtin::Previous => {
                const K: &[Chord] = &[c(Key::Tab, true, false)];
                K
            }
            Builtin::Up => {
                const K: &[Chord] = &[c(Key::Up, false, false)];
                K
            }
            Builtin::Down => {
                const K: &[Chord] = &[c(Key::Down, false, false)];
                K
            }
            Builtin::ExtendUp => {
                const K: &[Chord] = &[c(Key::Up, true, false)];
                K
            }
            Builtin::ExtendDown => {
                const K: &[Chord] = &[c(Key::Down, true, false)];
                K
            }
            Builtin::First => {
                const K: &[Chord] = &[c(Key::Home, false, false)];
                K
            }
            Builtin::Last => {
                const K: &[Chord] = &[c(Key::End, false, false)];
                K
            }
            Builtin::SelectAll => {
                const K: &[Chord] = &[c(Key::Char('a'), false, true)];
                K
            }
            Builtin::Cancel => {
                const K: &[Chord] = &[c(Key::Escape, false, false)];
                K
            }
            Builtin::Palette | Builtin::LightAppearance | Builtin::DarkAppearance | Builtin::SystemAppearance | Builtin::NextTheme | Builtin::NextStyle => &[],
        }
    }

    /// Commands that move around or change only how everything looks, rather than the facts; they never become undo steps and aren't offered in the action bar.
    pub fn moves(self) -> bool {
        !matches!(self, Builtin::Palette | Builtin::Undo | Builtin::Redo)
    }

    /// The change to the look it asks for, if it is one of the commands that switch it.
    pub fn switch(self) -> Option<Switch> {
        match self {
            Builtin::LightAppearance => Some(Switch::Appearance(Choice::Light)),
            Builtin::DarkAppearance => Some(Switch::Appearance(Choice::Dark)),
            Builtin::SystemAppearance => Some(Switch::Appearance(Choice::System)),
            Builtin::NextTheme => Some(Switch::NextTheme),
            Builtin::NextStyle => Some(Switch::NextStyle),
            _ => None,
        }
    }
}

const fn c(key: Key, shift: bool, primary: bool) -> Chord {
    Chord { key, mods: Mods { shift, alt: false, primary, control: false } }
}
