//! Modifier keys with a viewer-wide meaning.
//!
//! Test them with [`egui::Modifiers::matches_logically`].

use egui::Modifiers;

/// Hold while clicking to select every item between the last clicked item and this one.
pub const SELECTION_RANGE_MODIFIER: Modifiers = Modifiers::SHIFT;

/// Hold while clicking to add an item to, or remove it from, the current selection.
pub const SELECTION_TOGGLE_MODIFIER: Modifiers = Modifiers::COMMAND;

/// Hold to show extra information in the UI, e.g. in tooltips.
pub const SHOW_EXTRAS_MODIFIER: Modifiers = Modifiers::ALT;

/// Hold while clicking a visibility toggle or legend entry to show only that item.
pub const SOLO_MODIFIER: Modifiers = Modifiers::ALT;
