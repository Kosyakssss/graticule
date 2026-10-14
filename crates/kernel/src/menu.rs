//! Menus the system draws: the macOS menu bar's app and Window menus, and context menus generated from the registry.

use crate::keys::Chord;

/// One entry of a context menu: a command from the registry, by the id of the control that runs it, with its label, its key when it has a single one, and whether it can run now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuItem {
    pub id: u64,
    pub label: String,
    pub keys: Option<Chord>,
    pub enabled: bool,
}

/// The menu bar, kept alive for as long as the app runs.
#[cfg(target_os = "macos")]
pub type MenuBar = muda::Menu;

#[cfg(not(target_os = "macos"))]
pub type MenuBar = ();

/// Puts the app and Window menus in the macOS menu bar, so the system's own shortcuts and window tiling work, and sends every menu choice, from the bar or a context menu, to `chosen` by its id.
#[cfg(target_os = "macos")]
pub fn install(event_loop: &winit::event_loop::ActiveEventLoop, title: &str, chosen: impl Fn(String) + Send + Sync + 'static) -> Option<MenuBar> {
    use muda::{PredefinedMenuItem as Item, Submenu};
    use winit::platform::macos::ActiveEventLoopExtMacOS;
    muda::MenuEvent::set_event_handler(Some(move |e: muda::MenuEvent| chosen(e.id.0)));
    event_loop.set_allows_automatic_window_tabbing(false);
    let app = Submenu::with_items(
        title,
        true,
        &[
            &Item::about(None, None),
            &Item::separator(),
            &Item::services(None),
            &Item::separator(),
            &Item::hide(None),
            &Item::hide_others(None),
            &Item::show_all(None),
            &Item::separator(),
            &Item::quit(None),
        ],
    )
    .ok()?;
    let window = Submenu::with_items("Window", true, &[&Item::minimize(None), &Item::maximize(None), &Item::fullscreen(None), &Item::separator(), &Item::bring_all_to_front(None)]).ok()?;
    let menu = muda::Menu::with_items(&[&app, &window]).ok()?;
    menu.init_for_nsapp();
    window.set_as_windows_menu_for_nsapp();
    Some(menu)
}

#[cfg(not(target_os = "macos"))]
pub fn install(_: &winit::event_loop::ActiveEventLoop, _: &str, _: impl Fn(String) + Send + Sync + 'static) -> Option<MenuBar> {
    None
}

#[cfg(target_os = "macos")]
fn accelerator(chord: Chord) -> Option<muda::accelerator::Accelerator> {
    use crate::keys::Key;
    use muda::accelerator::{Accelerator, Code, Modifiers};
    const LETTERS: [Code; 26] = [
        Code::KeyA,
        Code::KeyB,
        Code::KeyC,
        Code::KeyD,
        Code::KeyE,
        Code::KeyF,
        Code::KeyG,
        Code::KeyH,
        Code::KeyI,
        Code::KeyJ,
        Code::KeyK,
        Code::KeyL,
        Code::KeyM,
        Code::KeyN,
        Code::KeyO,
        Code::KeyP,
        Code::KeyQ,
        Code::KeyR,
        Code::KeyS,
        Code::KeyT,
        Code::KeyU,
        Code::KeyV,
        Code::KeyW,
        Code::KeyX,
        Code::KeyY,
        Code::KeyZ,
    ];
    const DIGITS: [Code; 10] = [Code::Digit0, Code::Digit1, Code::Digit2, Code::Digit3, Code::Digit4, Code::Digit5, Code::Digit6, Code::Digit7, Code::Digit8, Code::Digit9];
    let m = chord.mods;
    let mut mods = Modifiers::empty();
    for (held, modifier) in [(m.primary, Modifiers::META), (m.shift, Modifiers::SHIFT), (m.alt, Modifiers::ALT), (m.control, Modifiers::CONTROL)] {
        if held {
            mods |= modifier;
        }
    }
    let code = match chord.key {
        Key::Char(c) if c.is_ascii_lowercase() => LETTERS[(c as u8 - b'a') as usize],
        Key::Char(c) if c.is_ascii_digit() => DIGITS[(c as u8 - b'0') as usize],
        Key::Char(' ') => Code::Space,
        Key::Enter => Code::Enter,
        Key::Escape => Code::Escape,
        Key::Up => Code::ArrowUp,
        Key::Down => Code::ArrowDown,
        Key::Left => Code::ArrowLeft,
        Key::Right => Code::ArrowRight,
        Key::PageUp => Code::PageUp,
        Key::PageDown => Code::PageDown,
        Key::Home => Code::Home,
        Key::End => Code::End,
        Key::Backspace => Code::Backspace,
        Key::Delete => Code::Delete,
        Key::Tab => Code::Tab,
        _ => return None,
    };
    Some(Accelerator::new(mods, code))
}

/// Opens a context menu of `items` at the pointer. The choice arrives later, through the handler `install` set, as the item's id. This is the one approved use of `unsafe` (`docs/internals/unsafe.md`): AppKit needs the window's view as a raw pointer.
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
pub fn popup(window: &winit::window::Window, items: &[MenuItem]) {
    use muda::ContextMenu;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if items.is_empty() {
        return;
    }
    let menu = muda::Menu::new();
    for item in items {
        if menu.append(&muda::MenuItem::with_id(item.id.to_string(), &item.label, item.enabled, item.keys.and_then(accelerator))).is_err() {
            return;
        }
    }
    let Some(RawWindowHandle::AppKit(view)) = window.window_handle().ok().map(|h| h.as_raw()) else { return };
    unsafe { menu.show_context_menu_for_nsview(view.ns_view.as_ptr(), None) };
}

#[cfg(not(target_os = "macos"))]
pub fn popup(_: &winit::window::Window, _: &[MenuItem]) {}
