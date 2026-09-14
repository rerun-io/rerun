use egui::{Key, KeyboardShortcut};

pub trait KeyboardShortcutExt {
    /// Would consuming this shortcut interfere with editing text
    /// (e.g. moving the text cursor, or typing a space)?
    ///
    /// Such shortcuts should only be consumed when no text field has focus.
    fn conflicts_with_text_editing(&self) -> bool;
}

impl KeyboardShortcutExt for KeyboardShortcut {
    fn conflicts_with_text_editing(&self) -> bool {
        self.modifiers.is_none()
            || matches!(
                self.logical_key,
                Key::Space
                    | Key::ArrowLeft
                    | Key::ArrowRight
                    | Key::ArrowUp
                    | Key::ArrowDown
                    | Key::Home
                    | Key::End
            )
    }
}
