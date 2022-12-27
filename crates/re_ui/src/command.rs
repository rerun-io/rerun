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
    OpenProfiler,

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
    pub fn text_and_tooltip(&self) -> (&'static str, &'static str) {
        match self {
            Command::Save => ("Save…", "Save all data to a Rerun data file (.rrd)"),
            Command::SaveSelection => (
                "Save loop selection…",
                "Save data for the current loop selection to a Rerun data file (.rrd)",
            ),
            Command::Open => ("Open", "Open a Rerun Data File (.rrd)"),
            Command::Quit => ("Quit", "Close the Rerun Viewer"),
            Command::ResetViewer => (
                "Reset viewer",
                "Reset the viewer to how it looked the first time you ran it",
            ),
            Command::OpenProfiler => (
                "Open profiler",
                "Starts a profiler, showing what makes the viewer run slow",
            ),

            Command::ToggleMemoryPanel => (
                "Toggle memory panel",
                "Investigate what is using up RAM in Rerun Viewer",
            ),
            Command::ToggleBlueprintPanel => ("Toggle blueprint panel", "Toggle the left panel"),
            Command::ToggleSelectionPanel => ("Toggle selection panel", "Toggle the right panel"),
            Command::ToggleTimePanel => ("Toggle time panel", "Toggle the bottom time panel"),
            Command::ToggleFullscreen => (
                "Toggle fullscreen",
                "Toggle between windowed and fullscreen viewer",
            ),

            Command::SelectionPrevious => ("Previous selection", "Go to previous selection"),
            Command::SelectionNext => ("Next selection", "Go to next selection"),
            Command::ToggleCommandPalette => (
                "Toggle command palette",
                "Toggle the command palette window",
            ),
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
            Command::OpenProfiler => Some(ctrl_shift(Key::P)),
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
