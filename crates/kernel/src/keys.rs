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

/// A mode of a modal keymap: which of its bindings apply, and whether typing reaches a field. Only insert lets typing through.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
    Select,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::Normal => "normal",
            Mode::Insert => "insert",
            Mode::Select => "select",
        }
    }

    fn parse(name: &str) -> Option<Mode> {
        [Mode::Normal, Mode::Insert, Mode::Select].into_iter().find(|m| m.name() == name)
    }
}

/// What a press means, given the keys pressed before it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lookup<'a> {
    Run(&'a str),
    More,
    /// The keys before it were an operator, so this press is the movement it acts over.
    Operator(&'a str),
    Nothing,
}

/// One place in a sequence: a chord, or the movement an operator waits for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Chord(Chord),
    Motion,
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Step::Chord(c) => c.fmt(f),
            Step::Motion => f.write_str("motion"),
        }
    }
}

/// One line of a keymap file: keys, the command they run, the mode they belong to (every mode when `None`), and whether they end waiting for a movement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub keys: Vec<Chord>,
    pub command: String,
    pub mode: Option<Mode>,
    pub motion: bool,
}

/// The keys of an app: a data file binding chords and sequences of chords to commands by name, in every mode or in one.
#[derive(Clone, Debug, Default)]
pub struct Keymap {
    pub bindings: Vec<Binding>,
    pub modal: bool,
}

impl Keymap {
    /// Reads a keymap file: one binding a line, `keys: command`, keys separated by spaces for a sequence, `motion` last for an operator; `[normal]`, `[insert]` or `[select]` starts the bindings of that mode; `#` starts a comment.
    pub fn parse(source: &str) -> Result<Keymap, String> {
        let mut keymap = Keymap::default();
        let mut mode = None;
        for (n, line) in source.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                mode = Some(Mode::parse(name.trim()).ok_or_else(|| format!("line {}: the modes are normal, insert and select", n + 1))?);
                keymap.modal = true;
                continue;
            }
            let Some((keys, command)) = line.rsplit_once(':') else { return Err(format!("line {}: write a binding as `keys: command`", n + 1)) };
            let command = command.trim();
            if command.is_empty() {
                return Err(format!("line {}: no command after the colon", n + 1));
            }
            let mut words: Vec<&str> = keys.split_whitespace().collect();
            let motion = words.last() == Some(&"motion");
            if motion {
                words.pop();
            }
            if words.contains(&"motion") {
                return Err(format!("line {}: “motion” can only come last", n + 1));
            }
            let chords = words.into_iter().map(Chord::parse).collect::<Result<Vec<_>, _>>().map_err(|e| format!("line {}: {e}", n + 1))?;
            if chords.is_empty() {
                return Err(format!("line {}: no keys before {}", n + 1, if motion { "the motion" } else { "the colon" }));
            }
            keymap.bindings.push(Binding { keys: chords, command: command.to_string(), mode, motion });
        }
        Ok(keymap)
    }

    fn active(&self, mode: Mode) -> impl Iterator<Item = &Binding> {
        self.bindings.iter().filter(move |b| b.mode.is_none_or(|m| m == mode))
    }

    /// The command bound to `pending` followed by `chord` in `mode`, whether they begin a longer sequence, or whether `pending` was an operator waiting for a movement.
    pub fn lookup(&self, mode: Mode, pending: &[Chord], chord: Chord) -> Lookup<'_> {
        let n = pending.len();
        let mut more = false;
        let mut operator = None;
        for b in self.active(mode) {
            if b.keys.len() > n && b.keys[..n] == *pending && b.keys[n] == chord {
                if b.keys.len() == n + 1 && !b.motion {
                    return Lookup::Run(&b.command);
                }
                more = true;
            } else if b.motion && b.keys == pending {
                operator = Some(b.command.as_str());
            }
        }
        match operator {
            _ if more => Lookup::More,
            Some(command) => Lookup::Operator(command),
            None => Lookup::Nothing,
        }
    }

    /// What can come after `pending` in `mode`: each next step with the command it runs, or `None` when it leads further.
    pub fn next<'a>(&'a self, mode: Mode, pending: &'a [Chord]) -> impl Iterator<Item = (Step, Option<&'a str>)> + 'a {
        let n = pending.len();
        self.active(mode).filter_map(move |b| match b.keys.len() {
            len if len > n && b.keys[..n] == *pending => Some((Step::Chord(b.keys[n]), (len == n + 1 && !b.motion).then_some(b.command.as_str()))),
            len if len == n && b.motion && b.keys == pending => Some((Step::Motion, Some(b.command.as_str()))),
            _ => None,
        })
    }

    /// Whether `mode`'s own bindings, not those of every mode, start with `chord`, so it reaches the keymap before a focused field's own keys: `[insert]` then `escape: normal-mode` leaves insert mode from inside an editor.
    pub fn claims(&self, mode: Mode, chord: Chord) -> bool {
        self.bindings.iter().any(|b| b.mode == Some(mode) && b.keys[0] == chord)
    }

    /// The first keys bound to a command in `mode`, for showing beside it.
    pub fn keys_for(&self, mode: Mode, command: &str) -> Option<String> {
        self.active(mode).find(|b| b.command == command).map(|b| if b.motion { format!("{} motion", label(&b.keys)) } else { label(&b.keys) })
    }

    /// The one chord that runs a command in `mode`, when a single chord does.
    pub fn chord_for(&self, mode: Mode, command: &str) -> Option<Chord> {
        self.active(mode).find(|b| b.command == command && !b.motion && b.keys.len() == 1).map(|b| b.keys[0])
    }
}

pub fn label(keys: &[Chord]) -> String {
    keys.iter().map(Chord::to_string).collect::<Vec<_>>().join(" ")
}
