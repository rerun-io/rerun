use egui::{Key, KeyboardShortcut, Modifiers};

/// All the commands we support.
///
/// Most are available in the GUI,
/// some have keyboard shortcuts,
/// and all are visible in the [`crate::CommandPalette`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, strum_macros::EnumIter)]
pub enum Command {
    /// In the order they show up in the command palette by default!
    Save,
    SaveSelection,
    Open,
    Quit,
    ResetViewer,
    ShowProfiler,

    ToggleMemoryPanel,
    ToggleBlueprintPanel,
    ToggleSelectionPanel,
    ToggleTimePanel,
    ToggleFullscreen,

    SelectionPrevious,
    SelectionNext,

    ToggleCommandPalette,
}

impl Command {
    pub fn text(&self) -> &'static str {
        match self {
            Command::Save => "Save",
            Command::SaveSelection => "Save selection",
            Command::Open => "Open",
            Command::Quit => "Quit",
            Command::ResetViewer => "Reset viewer",
            Command::ShowProfiler => "Show profiler",
            Command::ToggleMemoryPanel => "Toggle memory panel",
            Command::ToggleBlueprintPanel => "Toggle blueprint panel",
            Command::ToggleSelectionPanel => "Toggle selection panel",
            Command::ToggleTimePanel => "Toggle time panel",
            Command::ToggleFullscreen => "Toggle fullscreen",
            Command::SelectionPrevious => "Selection previous",
            Command::SelectionNext => "Selection next",
            Command::ToggleCommandPalette => "Toggle command palette",
        }
    }

    pub fn kb_shortcut(&self) -> Option<KeyboardShortcut> {
        fn cmd(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::COMMAND, key)
        }

        fn cmd_shift(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), key)
        }

        fn ctrl_shift(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::CTRL.plus(Modifiers::SHIFT), key)
        }

        match self {
            Command::Save => Some(cmd(Key::S)),
            Command::SaveSelection => Some(cmd_shift(Key::S)),
            Command::Open => Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::O)),

            #[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
            Command::Quit => Some(KeyboardShortcut::new(Modifiers::ALT, Key::F4)),

            #[cfg(all(not(target_arch = "wasm32"), not(target_os = "windows")))]
            Command::Quit => Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::Q)),

            Command::ResetViewer => Some(ctrl_shift(Key::R)),
            Command::ShowProfiler => Some(ctrl_shift(Key::P)),
            Command::ToggleMemoryPanel => Some(ctrl_shift(Key::M)),
            Command::ToggleBlueprintPanel => Some(ctrl_shift(Key::B)),
            Command::ToggleSelectionPanel => Some(ctrl_shift(Key::S)),
            Command::ToggleTimePanel => Some(ctrl_shift(Key::T)),
            Command::ToggleFullscreen => Some(ctrl_shift(Key::ArrowLeft)),
            Command::SelectionPrevious => Some(ctrl_shift(Key::ArrowRight)),
            Command::SelectionNext => Some(KeyboardShortcut::new(Modifiers::NONE, Key::F11)),
            Command::ToggleCommandPalette => Some(cmd(Key::P)),
        }
    }

    #[must_use = "Returns the Command that was triggered by some keyboard shortcut"]
    pub fn listen_for_kb_shortcut(egui_ctx: &egui::Context) -> Option<Command> {
        use strum::IntoEnumIterator as _;

        let mut input = egui_ctx.input_mut();
        for command in Command::iter() {
            if let Some(kb_shortcut) = command.kb_shortcut() {
                if input.consume_shortcut(&kb_shortcut) {
                    return Some(command);
                }
            }
        }
        None
    }
}
