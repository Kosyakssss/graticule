# Unsafe code

The workspace denies `unsafe` (`unsafe_code = "deny"` in the root `Cargo.toml`). Platform APIs are reached through crates that wrap them safely. Two exceptions are approved by the owner; any other needs their sign-off first, with proof that no safe route exists.

## The native context menu on macOS

`crates/kernel/src/menu.rs`, `popup`. muda opens a context menu with `ContextMenu::show_context_menu_for_nsview`, which takes the window's `NSView` as a raw pointer and is therefore `unsafe`; no safe wrapper exists. The pointer comes from winit's window handle for the window borrowed by the call, so the view outlives the menu: AppKit runs the menu's tracking loop inside the call and returns when the menu closes. muda reads the view only during that call. The function carries `#[allow(unsafe_code)]` and nothing else in it is unsafe.

Windows has the same call for a window handle (`show_context_menu_for_hwnd`), also `unsafe`; it isn't approved, so Windows and Linux get Graticule's own drawn menu, an anchored overlay (`docs/internals/shell.md`), instead.

## The macOS spellchecker

Approved if no safe binding reaches `NSSpellChecker` when Galley's spelling is built (item 23). Not used yet.
