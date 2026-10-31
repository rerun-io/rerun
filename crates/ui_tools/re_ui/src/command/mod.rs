//! [`UICommand`]: the commands that use the viewer's current context,
//! and helpers for matching keyboard shortcuts.

mod ui_command;

pub use self::ui_command::{UICommand, UICommandSender};

use egui::KeyboardShortcut;

/// Consume the best-matching pressed keyboard shortcut, if any.
pub fn consume_best_shortcut<Cmd: Copy>(
    egui_ctx: &egui::Context,
    mut commands: Vec<(KeyboardShortcut, Cmd)>,
) -> Option<Cmd> {
    let text_edit_has_focus = egui_ctx.text_edit_focused();

    // If the user pressed `Cmd-Shift-S` then egui will match that
    // with both `Cmd-Shift-S` and `Cmd-S`.
    // The reason is that `Shift` (and `Alt`) are sometimes required to produce certain keys,
    // such as `+` (`Shift =` on an american keyboard).
    // The result of this is that we must check for `Cmd-Shift-S` before `Cmd-S`, etc.
    // So we order the commands here so that the commands with `Shift` and `Alt` in them
    // are checked first.
    commands.sort_by_key(|(kb_shortcut, _cmd)| {
        let num_shift_alts = kb_shortcut.modifiers.shift as i32 + kb_shortcut.modifiers.alt as i32;
        -num_shift_alts // most first
    });

    egui_ctx.input_mut(|input| {
        for (kb_shortcut, command) in commands {
            if text_edit_has_focus && conflicts_with_text_editing(&kb_shortcut) {
                continue; // Make sure we can move text cursor with alt-arrow keys, etc
            }

            if input.consume_shortcut(&kb_shortcut) {
                // Clear the shortcut key from input to prevent it from propagating to other UI component.
                input.keys_down.remove(&kb_shortcut.logical_key);
                return Some(command);
            }
        }
        None
    })
}

/// Like [`KeyboardShortcut::conflicts_with_text_editing`], but also reserves undo/redo
/// (cmd/ctrl + Z/Y) for the focused text edit.
pub fn conflicts_with_text_editing(kb_shortcut: &KeyboardShortcut) -> bool {
    let modifiers = kb_shortcut.modifiers;
    kb_shortcut.conflicts_with_text_editing()
        || ((modifiers.command || modifiers.ctrl || modifiers.mac_cmd)
            && matches!(kb_shortcut.logical_key, egui::Key::Z | egui::Key::Y))
}

/// Like [`egui::Context::format_shortcut`], but spells arrow keys with glyphs our bundled fonts have.
///
/// egui's macOS key symbols for the arrow keys (`⏴⏵⏶⏷`) are missing from our bundled fonts,
/// and egui panics on missing glyphs.
pub fn format_shortcut(egui_ctx: &egui::Context, shortcut: &KeyboardShortcut) -> String {
    replace_arrow_symbols(&egui_ctx.format_shortcut(shortcut))
}

/// Like [`egui::Key::symbol_or_name`], but spells arrow keys with glyphs our bundled fonts have.
pub fn key_symbol_or_name(key: egui::Key) -> &'static str {
    match key {
        egui::Key::ArrowDown => "↓",
        egui::Key::ArrowLeft => "←",
        egui::Key::ArrowRight => "→",
        egui::Key::ArrowUp => "↑",
        key => key.symbol_or_name(),
    }
}

fn replace_arrow_symbols(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '⏷' => '↓',
            '⏴' => '←',
            '⏵' => '→',
            '⏶' => '↑',
            c => c,
        })
        .collect()
}
