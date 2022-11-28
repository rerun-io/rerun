//! All global keyboard shortcuts.
//!
//! We need to avoid shortcuts that the browser could/should use, e.g. `Cmd+R` on Mac (refresh).
//!
//! Similarly, ALT+SHIFT is a bad combination, as that is used for typing
//! special characters on Mac.

use egui::{Key, KeyboardShortcut, Modifiers};

const CTRL_SHIFT: Modifiers = Modifiers::CTRL.plus(Modifiers::SHIFT);

pub const RESET_VIEWER: KeyboardShortcut = KeyboardShortcut::new(CTRL_SHIFT, Key::R);

#[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
pub const SHOW_PROFILER: KeyboardShortcut = KeyboardShortcut::new(CTRL_SHIFT, Key::P);

pub const TOGGLE_MEMORY_PANEL: KeyboardShortcut = KeyboardShortcut::new(CTRL_SHIFT, Key::M);

pub const TOGGLE_BLUEPRINT_PANEL: KeyboardShortcut = KeyboardShortcut::new(CTRL_SHIFT, Key::B);
pub const TOGGLE_SELECTION_PANEL: KeyboardShortcut = KeyboardShortcut::new(CTRL_SHIFT, Key::S);
pub const TOGGLE_TIME_PANEL: KeyboardShortcut = KeyboardShortcut::new(CTRL_SHIFT, Key::T);

pub const TOGGLE_SELECTION_DETAILED: KeyboardShortcut = KeyboardShortcut::new(CTRL_SHIFT, Key::H);
// TODO(cmc): mouse button shortcut support for Andreas!
pub const SELECTION_PREVIOUS: KeyboardShortcut = KeyboardShortcut::new(CTRL_SHIFT, Key::O);
pub const SELECTION_NEXT: KeyboardShortcut = KeyboardShortcut::new(CTRL_SHIFT, Key::I);

#[cfg(not(target_arch = "wasm32"))]
pub const TOGGLE_FULLSCREEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F11);
