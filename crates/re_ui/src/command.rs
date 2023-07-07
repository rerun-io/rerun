use egui::{Key, KeyboardShortcut, Modifiers};

/// Interface for sending [`UICommand`] messages.
pub trait UICommandSender {
    fn send_ui(&self, command: UICommand);
}

/// All the commands we support.
///
/// Most are available in the GUI,
/// some have keyboard shortcuts,
/// and all are visible in the [`crate::CommandPalette`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, strum_macros::EnumIter)]
pub enum UICommand {
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
    PlaybackFollow,
    PlaybackStepBack,
    PlaybackStepForward,
    PlaybackRestart,

    // Dev-tools:
    #[cfg(not(target_arch = "wasm32"))]
    ScreenshotWholeApp,
    #[cfg(not(target_arch = "wasm32"))]
    PrintDatastore,
}

impl UICommand {
    pub fn text(self) -> &'static str {
        self.text_and_tooltip().0
    }

    pub fn tooltip(self) -> &'static str {
        self.text_and_tooltip().1
    }

    pub fn text_and_tooltip(self) -> (&'static str, &'static str) {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::Save => ("Save…", "Save all data to a Rerun data file (.rrd)"),

            #[cfg(not(target_arch = "wasm32"))]
            UICommand::SaveSelection => (
                "Save loop selection…",
                "Save data for the current loop selection to a Rerun data file (.rrd)",
            ),

            #[cfg(not(target_arch = "wasm32"))]
            UICommand::Open => ("Open…", "Open a Rerun Data File (.rrd)"),

            #[cfg(not(target_arch = "wasm32"))]
            UICommand::Quit => ("Quit", "Close the Rerun Viewer"),

            UICommand::ResetViewer => (
                "Reset viewer",
                "Reset the viewer to how it looked the first time you ran it",
            ),

            #[cfg(not(target_arch = "wasm32"))]
            UICommand::OpenProfiler => (
                "Open profiler",
                "Starts a profiler, showing what makes the viewer run slow",
            ),

            UICommand::ToggleMemoryPanel => (
                "Toggle memory panel",
                "Investigate what is using up RAM in Rerun Viewer",
            ),
            UICommand::ToggleBlueprintPanel => ("Toggle blueprint panel", "Toggle the left panel"),
            UICommand::ToggleSelectionPanel => ("Toggle selection panel", "Toggle the right panel"),
            UICommand::ToggleTimePanel => ("Toggle time panel", "Toggle the bottom time panel"),

            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ToggleFullscreen => (
                "Toggle fullscreen",
                "Toggle between windowed and fullscreen viewer",
            ),
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ZoomIn => ("Zoom In", "Increases the ui scaling factor"),
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ZoomOut => ("Zoom Out", "Decreases the ui scaling factor"),
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ZoomReset => (
                "Reset Zoom",
                "Resets ui scaling factor to the OS provided default",
            ),

            UICommand::SelectionPrevious => ("Previous selection", "Go to previous selection"),
            UICommand::SelectionNext => ("Next selection", "Go to next selection"),
            UICommand::ToggleCommandPalette => {
                ("Command palette…", "Toggle the command palette window")
            }

            UICommand::PlaybackTogglePlayPause => {
                ("Toggle play/pause", "Either play or pause the time")
            }
            UICommand::PlaybackFollow => ("Follow", "Follow on from end of timeline"),
            UICommand::PlaybackStepBack => (
                "Step time back",
                "Move the time marker back to the previous point in time with any data",
            ),
            UICommand::PlaybackStepForward => (
                "Step time forward",
                "Move the time marker to the next point in time with any data",
            ),
            UICommand::PlaybackRestart => ("Restart", "Restart from beginning of timeline"),

            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ScreenshotWholeApp => (
                "Screenshot",
                "Copy screenshot of the whole app to clipboard",
            ),
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::PrintDatastore => (
                "Print datastore",
                "Prints the entire data store to the console. WARNING: this may be A LOT of text.",
            ),
        }
    }

    #[allow(clippy::unnecessary_wraps)] // Only on some platforms
    pub fn kb_shortcut(self) -> Option<KeyboardShortcut> {
        fn key(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::NONE, key)
        }

        fn cmd(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::COMMAND, key)
        }

        #[cfg(not(target_arch = "wasm32"))]
        fn cmd_alt(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::ALT), key)
        }

        fn ctrl_shift(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::CTRL.plus(Modifiers::SHIFT), key)
        }

        match self {
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::Save => Some(cmd(Key::S)),
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::SaveSelection => Some(cmd_alt(Key::S)),
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::Open => Some(cmd(Key::O)),

            #[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
            UICommand::Quit => Some(KeyboardShortcut::new(Modifiers::ALT, Key::F4)),

            #[cfg(all(not(target_arch = "wasm32"), not(target_os = "windows")))]
            UICommand::Quit => Some(cmd(Key::Q)),

            UICommand::ResetViewer => Some(ctrl_shift(Key::R)),
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::OpenProfiler => Some(ctrl_shift(Key::P)),
            UICommand::ToggleMemoryPanel => Some(ctrl_shift(Key::M)),
            UICommand::ToggleBlueprintPanel => Some(ctrl_shift(Key::B)),
            UICommand::ToggleSelectionPanel => Some(ctrl_shift(Key::S)),
            UICommand::ToggleTimePanel => Some(ctrl_shift(Key::T)),

            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ToggleFullscreen => Some(key(Key::F11)),
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ZoomIn => Some(egui::gui_zoom::kb_shortcuts::ZOOM_IN),
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ZoomOut => Some(egui::gui_zoom::kb_shortcuts::ZOOM_OUT),
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ZoomReset => Some(egui::gui_zoom::kb_shortcuts::ZOOM_RESET),

            UICommand::SelectionPrevious => Some(ctrl_shift(Key::ArrowLeft)),
            UICommand::SelectionNext => Some(ctrl_shift(Key::ArrowRight)),
            UICommand::ToggleCommandPalette => Some(cmd(Key::P)),

            UICommand::PlaybackTogglePlayPause => Some(key(Key::Space)),
            UICommand::PlaybackFollow => Some(cmd(Key::ArrowRight)),
            UICommand::PlaybackStepBack => Some(key(Key::ArrowLeft)),
            UICommand::PlaybackStepForward => Some(key(Key::ArrowRight)),
            UICommand::PlaybackRestart => Some(cmd(Key::ArrowLeft)),

            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ScreenshotWholeApp => None,
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::PrintDatastore => None,
        }
    }

    #[must_use = "Returns the Command that was triggered by some keyboard shortcut"]
    pub fn listen_for_kb_shortcut(egui_ctx: &egui::Context) -> Option<UICommand> {
        use strum::IntoEnumIterator as _;

        let anything_has_focus = egui_ctx.memory(|mem| mem.focus().is_some());
        if anything_has_focus {
            return None; // e.g. we're typing in a TextField
        }

        egui_ctx.input_mut(|input| {
            for command in UICommand::iter() {
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
        command_sender: &impl UICommandSender,
    ) -> egui::Response {
        let button = self.menu_button(ui.ctx());
        let response = ui.add(button).on_hover_text(self.tooltip());
        if response.clicked() {
            command_sender.send_ui(self);
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

#[test]
fn check_for_clashing_command_shortcuts() {
    fn clashes(a: KeyboardShortcut, b: KeyboardShortcut) -> bool {
        if a.key != b.key {
            return false;
        }

        if a.modifiers.alt != b.modifiers.alt {
            return false;
        }

        if a.modifiers.shift != b.modifiers.shift {
            return false;
        }

        // On Non-Mac, command is interpreted as ctrl!
        (a.modifiers.command || a.modifiers.ctrl) == (b.modifiers.command || b.modifiers.ctrl)
    }

    use strum::IntoEnumIterator as _;

    for a_cmd in UICommand::iter() {
        if let Some(a_shortcut) = a_cmd.kb_shortcut() {
            for b_cmd in UICommand::iter() {
                if a_cmd == b_cmd {
                    continue;
                }
                if let Some(b_shortcut) = b_cmd.kb_shortcut() {
                    assert!(
                        !clashes(a_shortcut, b_shortcut),
                        "Command '{a_cmd:?}' and '{b_cmd:?}' have overlapping keyboard shortcuts: {:?} vs {:?}",
                        a_shortcut.format(&egui::ModifierNames::NAMES, true),
                        b_shortcut.format(&egui::ModifierNames::NAMES, true),
                    );
                }
            }
        }
    }
}
