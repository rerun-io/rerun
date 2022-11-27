//! All global keyboard shortcuts.
//!
//! We need to avoid shortcuts that the browser could/should use, e.g. `Cmd+R` on Mac (refresh).
//!
//! Similarly, ALT+SHIFT is a bad combination, as that is used for typing
//! special characters on Mac.

use egui::{Key, KeyboardShortcut, Modifiers};

const CTRL_SHIFT: Modifiers = Modifiers::CTRL.plus(Modifiers::SHIFT);
#[cfg(not(target_arch = "wasm32"))]
const CMD_SHIFT: Modifiers = Modifiers::COMMAND.plus(Modifiers::SHIFT);

pub const RESET_VIEWER: KeyboardShortcut = KeyboardShortcut::new(CTRL_SHIFT, Key::R);

#[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
pub const SHOW_PROFILER: KeyboardShortcut = KeyboardShortcut::new(CTRL_SHIFT, Key::P);

pub const TOGGLE_MEMORY_PANEL: KeyboardShortcut = KeyboardShortcut::new(CTRL_SHIFT, Key::M);

pub const TOGGLE_BLUEPRINT_PANEL: KeyboardShortcut = KeyboardShortcut::new(CTRL_SHIFT, Key::B);
pub const TOGGLE_SELECTION_PANEL: KeyboardShortcut = KeyboardShortcut::new(CTRL_SHIFT, Key::S);
pub const TOGGLE_TIME_PANEL: KeyboardShortcut = KeyboardShortcut::new(CTRL_SHIFT, Key::T);

#[cfg(not(target_arch = "wasm32"))]
pub const TOGGLE_FULLSCREEN: KeyboardShortcut = KeyboardShortcut::new(CMD_SHIFT, Key::F);
