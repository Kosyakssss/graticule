use std::fmt;

use winit::keyboard::{Key as Logical, NamedKey};
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;

use crate::shell::Mods;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Up,
    Down,
    Left,
    Right,
    Enter,
    Escape,
    Backspace,
    Delete,
    PageUp,
    PageDown,
    Home,
    End,
    Tab,
    F(u8),
}

/// One press: a key with the modifiers held. `primary` is Command on a Mac and Control elsewhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    pub key: Key,
    pub mods: Mods,
}

impl Chord {
    pub fn of(event: &winit::event::KeyEvent, mods: Mods) -> Option<Chord> {
        let unshifted;
        let logical = if mods.alt || mods.primary {
            unshifted = event.key_without_modifiers();
            &unshifted
        } else {
            &event.logical_key
        };
        let key = match logical {
            Logical::Named(NamedKey::ArrowDown) => Key::Down,
            Logical::Named(NamedKey::ArrowUp) => Key::Up,
            Logical::Named(NamedKey::ArrowLeft) => Key::Left,
            Logical::Named(NamedKey::ArrowRight) => Key::Right,
            Logical::Named(NamedKey::Enter) => Key::Enter,
            Logical::Named(NamedKey::Escape) => Key::Escape,
            Logical::Named(NamedKey::Backspace) => Key::Backspace,
            Logical::Named(NamedKey::Delete) => Key::Delete,
            Logical::Named(NamedKey::PageDown) => Key::PageDown,
            Logical::Named(NamedKey::PageUp) => Key::PageUp,
            Logical::Named(NamedKey::Home) => Key::Home,
            Logical::Named(NamedKey::End) => Key::End,
            Logical::Named(NamedKey::Space) => Key::Char(' '),
            Logical::Named(NamedKey::Tab) => Key::Tab,
            Logical::Named(NamedKey::F1) => Key::F(1),
            Logical::Named(NamedKey::F2) => Key::F(2),
            Logical::Named(NamedKey::F3) => Key::F(3),
            Logical::Named(NamedKey::F4) => Key::F(4),
            Logical::Named(NamedKey::F5) => Key::F(5),
            Logical::Named(NamedKey::F6) => Key::F(6),
            Logical::Named(NamedKey::F7) => Key::F(7),
            Logical::Named(NamedKey::F8) => Key::F(8),
            Logical::Named(NamedKey::F9) => Key::F(9),
            Logical::Named(NamedKey::F10) => Key::F(10),
            Logical::Named(NamedKey::F11) => Key::F(11),
            Logical::Named(NamedKey::F12) => Key::F(12),
            Logical::Character(s) => Key::Char(s.chars().next()?.to_lowercase().next()?),
            _ => return None,
        };
        Some(Chord { key, mods })
    }

    /// Reads a chord as a keymap file writes it: `d`, `delete`, `space`, `primary+shift+p`.
    pub fn parse(s: &str) -> Result<Chord, String> {
        let mut mods = Mods::default();
        let mut parts: Vec<&str> = s.split('+').collect();
        let name = parts.pop().filter(|n| !n.is_empty()).ok_or_else(|| format!("“{s}” has no key"))?;
        for m in parts {
            match m {
                "primary" => mods.primary = true,
                "shift" => mods.shift = true,
                "alt" | "option" => mods.alt = true,
                "control" => mods.control = true,
                other => return Err(format!("“{other}” is not a modifier; the modifiers are primary, shift, alt and control")),
            }
        }
        let key = match name {
            "up" => Key::Up,
            "down" => Key::Down,
            "left" => Key::Left,
            "right" => Key::Right,
            "enter" => Key::Enter,
            "escape" => Key::Escape,
            "backspace" => Key::Backspace,
            "delete" => Key::Delete,
            "pageup" => Key::PageUp,
            "pagedown" => Key::PageDown,
            "home" => Key::Home,
            "end" => Key::End,
            "tab" => Key::Tab,
            "space" => Key::Char(' '),
            f if f.len() > 1 && f.starts_with('f') && f[1..].parse::<u8>().is_ok_and(|n| (1..=12).contains(&n)) => Key::F(f[1..].parse().unwrap_or(1)),
            c if c.chars().count() == 1 => Key::Char(c.chars().next().unwrap_or(' ').to_lowercase().next().unwrap_or(' ')),
            other => return Err(format!("“{other}” is not a key")),
        };
        Ok(Chord { key, mods })
    }

    /// A key that types text in a field when pressed without primary or control.
    pub fn types(&self) -> bool {
        matches!(self.key, Key::Char(_)) && !self.mods.primary && !self.mods.control
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mac = cfg!(target_os = "macos");
        let m = self.mods;
        if m.control {
            f.write_str(if mac { "⌃" } else { "Win+" })?;
        }
        if m.alt {
            f.write_str(if mac { "⌥" } else { "Alt+" })?;
        }
        if m.shift {
            f.write_str(if mac { "⇧" } else { "Shift+" })?;
        }
        if m.primary {
            f.write_str(if mac { "⌘" } else { "Ctrl+" })?;
        }
        match self.key {
            Key::Char(' ') => f.write_str("Space"),
            Key::Char(c) if m.primary || m.control || m.alt => write!(f, "{}", c.to_uppercase()),
            Key::Char(c) => write!(f, "{c}"),
            Key::Up => f.write_str("↑"),
            Key::Down => f.write_str("↓"),
            Key::Left => f.write_str("←"),
            Key::Right => f.write_str("→"),
            Key::Enter => f.write_str("Enter"),
            Key::Escape => f.write_str("Esc"),
            Key::Backspace => f.write_str("Backspace"),
            Key::Delete => f.write_str("Delete"),
            Key::PageUp => f.write_str("Page Up"),
            Key::PageDown => f.write_str("Page Down"),
            Key::Home => f.write_str("Home"),
            Key::End => f.write_str("End"),
            Key::Tab => f.write_str("Tab"),
            Key::F(n) => write!(f, "F{n}"),
        }
    }
}

/// What a press means, given the keys pressed before it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lookup<'a> {
    Run(&'a str),
    More,
    Nothing,
}

/// The keys of an app: a data file binding chords and sequences of chords to commands by name.
#[derive(Clone, Debug, Default)]
pub struct Keymap {
    pub bindings: Vec<(Vec<Chord>, String)>,
}

impl Keymap {
    /// Reads a keymap file: one binding a line, `keys: command`, keys separated by spaces for a sequence; `#` starts a comment.
    pub fn parse(source: &str) -> Result<Keymap, String> {
        let mut bindings = Vec::new();
        for (n, line) in source.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let Some((keys, command)) = line.rsplit_once(':') else { return Err(format!("line {}: write a binding as `keys: command`", n + 1)) };
            let command = command.trim();
            if command.is_empty() {
                return Err(format!("line {}: no command after the colon", n + 1));
            }
            let chords = keys.split_whitespace().map(Chord::parse).collect::<Result<Vec<_>, _>>().map_err(|e| format!("line {}: {e}", n + 1))?;
            if chords.is_empty() {
                return Err(format!("line {}: no keys before the colon", n + 1));
            }
            bindings.push((chords, command.to_string()));
        }
        Ok(Keymap { bindings })
    }

    /// The command bound to `pending` followed by `chord`, or whether they begin a longer sequence.
    pub fn lookup(&self, pending: &[Chord], chord: Chord) -> Lookup<'_> {
        let n = pending.len();
        let mut more = false;
        for (keys, command) in &self.bindings {
            if keys.len() > n && keys[..n] == *pending && keys[n] == chord {
                if keys.len() == n + 1 {
                    return Lookup::Run(command);
                }
                more = true;
            }
        }
        if more { Lookup::More } else { Lookup::Nothing }
    }

    /// What can come after `pending`: each next chord with the command it runs, or `None` when it leads further.
    pub fn next<'a>(&'a self, pending: &'a [Chord]) -> impl Iterator<Item = (Chord, Option<&'a str>)> + 'a {
        let n = pending.len();
        self.bindings.iter().filter(move |(keys, _)| keys.len() > n && keys[..n] == *pending).map(move |(keys, command)| (keys[n], (keys.len() == n + 1).then_some(command.as_str())))
    }

    /// The first keys bound to a command, for showing beside it.
    pub fn keys_for(&self, command: &str) -> Option<&[Chord]> {
        self.bindings.iter().find(|(_, c)| c == command).map(|(k, _)| k.as_slice())
    }
}

pub fn label(keys: &[Chord]) -> String {
    keys.iter().map(Chord::to_string).collect::<Vec<_>>().join(" ")
}
