//! All global keyboard shortcuts.
//!
//! We need to avoid shortcuts that the browser could/should use, e.g. `Cmd+R` on Mac (refresh).
//!
//! Similarly, ALT+SHIFT is a bad combination, as that is used for typing
//! special characters on Mac.

use egui::{Key, KeyboardShortcut, Modifiers};

const CTRL_SHIFT: Modifiers = Modifiers::CTRL.plus(Modifiers::SHIFT);

#[cfg(not(target_arch = "wasm32"))]
pub const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);

#[cfg(not(target_arch = "wasm32"))]
pub const SAVE_SELECTION: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::S);

#[cfg(not(target_arch = "wasm32"))]
pub const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
pub const QUIT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::ALT, Key::F4);
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "windows")))]
pub const QUIT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Q);

pub const RESET_VIEWER: KeyboardShortcut = KeyboardShortcut::new(CTRL_SHIFT, Key::R);

#[cfg(not(target_arch = "wasm32"))]
pub const SHOW_PROFILER: KeyboardShortcut = KeyboardShortcut::new(CTRL_SHIFT, Key::P);

pub const TOGGLE_MEMORY_PANEL: KeyboardShortcut = KeyboardShortcut::new(CTRL_SHIFT, Key::M);

pub const TOGGLE_BLUEPRINT_PANEL: KeyboardShortcut = KeyboardShortcut::new(CTRL_SHIFT, Key::B);
pub const TOGGLE_SELECTION_PANEL: KeyboardShortcut = KeyboardShortcut::new(CTRL_SHIFT, Key::S);
pub const TOGGLE_TIME_PANEL: KeyboardShortcut = KeyboardShortcut::new(CTRL_SHIFT, Key::T);

// TODO(cmc): mouse button shortcut support for Andreas!
pub const SELECTION_PREVIOUS: KeyboardShortcut = KeyboardShortcut::new(CTRL_SHIFT, Key::ArrowLeft);
pub const SELECTION_NEXT: KeyboardShortcut = KeyboardShortcut::new(CTRL_SHIFT, Key::ArrowRight);

#[cfg(not(target_arch = "wasm32"))]
pub const TOGGLE_FULLSCREEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F11);
