use egui::{Key, KeyboardShortcut, Modifiers};

/// All the commands we support.
///
/// Most are available in the GUI,
/// some have keyboard shortcuts,
/// and all are visible in the [`crate::CommandPalette`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, strum_macros::EnumIter)]
pub enum Command {
    // Listed in the order they show up in the command palette by default!
    #[cfg(not(target_arch = "wasm32"))]
    Open,
    #[cfg(not(target_arch = "wasm32"))]
    Save,
    #[cfg(not(target_arch = "wasm32"))]
    SaveSelection,
    #[cfg(not(target_arch = "wasm32"))]
    Quit,

    ResetViewer,

    #[cfg(not(target_arch = "wasm32"))]
    OpenProfiler,

    ToggleMemoryPanel,
    ToggleBlueprintPanel,
    ToggleSelectionPanel,
    ToggleTimePanel,

    #[cfg(not(target_arch = "wasm32"))]
    ToggleFullscreen,
    #[cfg(not(target_arch = "wasm32"))]
    ZoomIn,
    #[cfg(not(target_arch = "wasm32"))]
    ZoomOut,
    #[cfg(not(target_arch = "wasm32"))]
    ZoomReset,

    SelectionPrevious,
    SelectionNext,

    ToggleCommandPalette,

    // Playback:
    PlaybackTogglePlayPause,
    PlaybackStepBack,
    PlaybackStepForward,
}

impl Command {
    pub fn text(self) -> &'static str {
        self.text_and_tooltip().0
    }

    pub fn tooltip(self) -> &'static str {
        self.text_and_tooltip().1
    }

    pub fn text_and_tooltip(self) -> (&'static str, &'static str) {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Command::Save => ("Save…", "Save all data to a Rerun data file (.rrd)"),

            #[cfg(not(target_arch = "wasm32"))]
            Command::SaveSelection => (
                "Save loop selection…",
                "Save data for the current loop selection to a Rerun data file (.rrd)",
            ),

            #[cfg(not(target_arch = "wasm32"))]
            Command::Open => ("Open…", "Open a Rerun Data File (.rrd)"),

            #[cfg(not(target_arch = "wasm32"))]
            Command::Quit => ("Quit", "Close the Rerun Viewer"),

            Command::ResetViewer => (
                "Reset viewer",
                "Reset the viewer to how it looked the first time you ran it",
            ),

            #[cfg(not(target_arch = "wasm32"))]
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

            #[cfg(not(target_arch = "wasm32"))]
            Command::ToggleFullscreen => (
                "Toggle fullscreen",
                "Toggle between windowed and fullscreen viewer",
            ),
            #[cfg(not(target_arch = "wasm32"))]
            Command::ZoomIn => ("Zoom In", "Increases the ui scaling factor"),
            #[cfg(not(target_arch = "wasm32"))]
            Command::ZoomOut => ("Zoom Out", "Decreases the ui scaling factor"),
            #[cfg(not(target_arch = "wasm32"))]
            Command::ZoomReset => (
                "Reset Zoom",
                "Resets ui scaling factor to the OS provided default",
            ),

            Command::SelectionPrevious => ("Previous selection", "Go to previous selection"),
            Command::SelectionNext => ("Next selection", "Go to next selection"),
            Command::ToggleCommandPalette => {
                ("Command palette…", "Toggle the command palette window")
            }

            Command::PlaybackTogglePlayPause => {
                ("Toggle play/pause", "Either play or pause the time")
            }
            Command::PlaybackStepBack => (
                "Step time back",
                "Move the time marker back to the previous point in time with any data",
            ),
            Command::PlaybackStepForward => (
                "Step time forward",
                "Move the time marker to the next point in time with any data",
            ),
        }
    }

    pub fn kb_shortcut(self) -> Option<KeyboardShortcut> {
        fn key(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::NONE, key)
        }

        fn cmd(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::COMMAND, key)
        }

        #[cfg(not(target_arch = "wasm32"))]
        fn cmd_shift(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), key)
        }

        fn ctrl_shift(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::CTRL.plus(Modifiers::SHIFT), key)
        }

        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Command::Save => Some(cmd(Key::S)),
            #[cfg(not(target_arch = "wasm32"))]
            Command::SaveSelection => Some(cmd_shift(Key::S)),
            #[cfg(not(target_arch = "wasm32"))]
            Command::Open => Some(cmd(Key::O)),

            #[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
            Command::Quit => Some(KeyboardShortcut::new(Modifiers::ALT, Key::F4)),

            #[cfg(all(not(target_arch = "wasm32"), not(target_os = "windows")))]
            Command::Quit => Some(cmd(Key::Q)),

            Command::ResetViewer => Some(ctrl_shift(Key::R)),
            #[cfg(not(target_arch = "wasm32"))]
            Command::OpenProfiler => Some(ctrl_shift(Key::P)),
            Command::ToggleMemoryPanel => Some(ctrl_shift(Key::M)),
            Command::ToggleBlueprintPanel => Some(ctrl_shift(Key::B)),
            Command::ToggleSelectionPanel => Some(ctrl_shift(Key::S)),
            Command::ToggleTimePanel => Some(ctrl_shift(Key::T)),

            #[cfg(not(target_arch = "wasm32"))]
            Command::ToggleFullscreen => Some(key(Key::F11)),
            #[cfg(not(target_arch = "wasm32"))]
            Command::ZoomIn => Some(egui::gui_zoom::kb_shortcuts::ZOOM_IN),
            #[cfg(not(target_arch = "wasm32"))]
            Command::ZoomOut => Some(egui::gui_zoom::kb_shortcuts::ZOOM_OUT),
            #[cfg(not(target_arch = "wasm32"))]
            Command::ZoomReset => Some(egui::gui_zoom::kb_shortcuts::ZOOM_RESET),

            Command::SelectionPrevious => Some(ctrl_shift(Key::ArrowLeft)),
            Command::SelectionNext => Some(ctrl_shift(Key::ArrowRight)),
            Command::ToggleCommandPalette => Some(cmd(Key::P)),

            Command::PlaybackTogglePlayPause => Some(key(Key::Space)),
            Command::PlaybackStepBack => Some(key(Key::ArrowLeft)),
            Command::PlaybackStepForward => Some(key(Key::ArrowRight)),
        }
    }

    #[must_use = "Returns the Command that was triggered by some keyboard shortcut"]
    pub fn listen_for_kb_shortcut(egui_ctx: &egui::Context) -> Option<Command> {
        use strum::IntoEnumIterator as _;

        let anything_has_focus = egui_ctx.memory(|mem| mem.focus().is_some());
        if anything_has_focus {
            return None; // e.g. we're typing in a TextField
        }

        egui_ctx.input_mut(|input| {
            for command in Command::iter() {
                if let Some(kb_shortcut) = command.kb_shortcut() {
                    if input.consume_shortcut(&kb_shortcut) {
                        return Some(command);
                    }
                }
            }
            None
        })
    }

    /// Show this command as a menu-button.
    ///
    /// If clicked, enqueue the command.
    pub fn menu_button_ui(
        self,
        ui: &mut egui::Ui,
        pending_commands: &mut Vec<Command>,
    ) -> egui::Response {
        let button = self.menu_button(ui.ctx());
        let response = ui.add(button).on_hover_text(self.tooltip());
        if response.clicked() {
            pending_commands.push(self);
            ui.close_menu();
        }
        response
    }

    pub fn menu_button(self, egui_ctx: &egui::Context) -> egui::Button {
        let mut button = egui::Button::new(self.text());
        if let Some(shortcut) = self.kb_shortcut() {
            button = button.shortcut_text(egui_ctx.format_shortcut(&shortcut));
        }
        button
    }

    /// Add e.g. " (Ctrl+F11)" as a suffix
    pub fn format_shortcut_tooltip_suffix(self, egui_ctx: &egui::Context) -> String {
        if let Some(kb_shortcut) = self.kb_shortcut() {
            format!(" ({})", egui_ctx.format_shortcut(&kb_shortcut))
        } else {
            Default::default()
        }
    }
}
