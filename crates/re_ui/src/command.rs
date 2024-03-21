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
    Open,
    SaveRecording,
    SaveRecordingSelection,
    SaveBlueprint,
    CloseCurrentRecording,
    #[cfg(not(target_arch = "wasm32"))]
    Quit,

    OpenWebHelp,
    OpenRerunDiscord,

    ResetViewer,

    #[cfg(not(target_arch = "wasm32"))]
    OpenProfiler,

    ToggleMemoryPanel,
    ToggleBlueprintPanel,
    ToggleSelectionPanel,
    ToggleTimePanel,

    #[cfg(debug_assertions)]
    ToggleBlueprintInspectionPanel,

    #[cfg(debug_assertions)]
    ToggleEguiDebugPanel,

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
    PrintDataStore,
    #[cfg(not(target_arch = "wasm32"))]
    PrintBlueprintStore,
    #[cfg(not(target_arch = "wasm32"))]
    ClearPrimaryCache,
    #[cfg(not(target_arch = "wasm32"))]
    PrintPrimaryCache,

    #[cfg(target_arch = "wasm32")]
    CopyDirectLink,

    // Graphics options:
    #[cfg(target_arch = "wasm32")]
    RestartWithWebGl,
    #[cfg(target_arch = "wasm32")]
    RestartWithWebGpu,
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
            Self::SaveRecording => ("Save recording…", "Save all data to a Rerun data file (.rrd)"),

            Self::SaveRecordingSelection => (
                "Save current time selection…",
                "Save data for the current loop selection to a Rerun data file (.rrd)",
            ),

            Self::SaveBlueprint => ("Save blueprint…", "Save the current viewer setup as a Rerun blueprint file (.rbl)"),

            Self::Open => ("Open…", "Open any supported files (.rrd, images, meshes, …)"),

            Self::CloseCurrentRecording => (
                "Close current Recording",
                "Close the current Recording (unsaved data will be lost)",
            ),

            #[cfg(not(target_arch = "wasm32"))]
            Self::Quit => ("Quit", "Close the Rerun Viewer"),

            Self::OpenWebHelp => ("Help", "Visit the help page on our website, with troubleshooting tips and more"),
            Self::OpenRerunDiscord => ("Rerun Discord", "Visit the Rerun Discord server, where you can ask questions and get help"),

            Self::ResetViewer => (
                "Reset Viewer",
                "Reset the Viewer to how it looked the first time you ran it, forgetting all stored blueprints and UI state",
            ),

            #[cfg(not(target_arch = "wasm32"))]
            Self::OpenProfiler => (
                "Open profiler",
                "Starts a profiler, showing what makes the viewer run slow",
            ),

            Self::ToggleMemoryPanel => (
                "Toggle memory panel",
                "View and track current RAM usage inside Rerun Viewer",
            ),
            Self::ToggleBlueprintPanel => ("Toggle blueprint panel", "Toggle the left panel"),
            Self::ToggleSelectionPanel => ("Toggle selection panel", "Toggle the right panel"),
            Self::ToggleTimePanel => ("Toggle time panel", "Toggle the bottom panel"),

            #[cfg(debug_assertions)]
            Self::ToggleBlueprintInspectionPanel => (
                "Toggle blueprint inspection panel",
                "Inspect the timeline of the internal blueprint data.",
            ),

            #[cfg(debug_assertions)]
            Self::ToggleEguiDebugPanel => (
                "Toggle egui debug panel",
                "View and change global egui style settings",
            ),


            #[cfg(not(target_arch = "wasm32"))]
            Self::ToggleFullscreen => (
                "Toggle fullscreen",
                "Toggle between windowed and fullscreen viewer",
            ),
            #[cfg(not(target_arch = "wasm32"))]
            Self::ZoomIn => ("Zoom in", "Increases the UI zoom level"),
            #[cfg(not(target_arch = "wasm32"))]
            Self::ZoomOut => ("Zoom out", "Decreases the UI zoom level"),
            #[cfg(not(target_arch = "wasm32"))]
            Self::ZoomReset => (
                "Reset zoom",
                "Resets the UI zoom level to the operating system's default value",
            ),

            Self::SelectionPrevious => ("Previous selection", "Go to previous selection"),
            Self::SelectionNext => ("Next selection", "Go to next selection"),
            Self::ToggleCommandPalette => ("Command palette…", "Toggle the Command Palette"),

            Self::PlaybackTogglePlayPause => {
                ("Toggle play/pause", "Either play or pause the time")
            }
            Self::PlaybackFollow => ("Follow", "Follow on from end of timeline"),
            Self::PlaybackStepBack => (
                "Step time back",
                "Move the time marker back to the previous point in time with any data",
            ),
            Self::PlaybackStepForward => (
                "Step time forward",
                "Move the time marker to the next point in time with any data",
            ),
            Self::PlaybackRestart => ("Restart", "Restart from beginning of timeline"),

            #[cfg(not(target_arch = "wasm32"))]
            Self::ScreenshotWholeApp => (
                "Screenshot",
                "Copy screenshot of the whole app to clipboard",
            ),
            #[cfg(not(target_arch = "wasm32"))]
            Self::PrintDataStore => (
                "Print datastore",
                "Prints the entire data store to the console and clipboard. WARNING: this may be A LOT of text.",
            ),
            #[cfg(not(target_arch = "wasm32"))]
            Self::PrintBlueprintStore => (
                "Print blueprint store",
                "Prints the entire blueprint store to the console and clipboard. WARNING: this may be A LOT of text.",
            ),
            #[cfg(not(target_arch = "wasm32"))]
            Self::ClearPrimaryCache => (
                "Clear primary cache",
                "Clears the primary cache in its entirety.",
            ),
            #[cfg(not(target_arch = "wasm32"))]
            Self::PrintPrimaryCache => (
                "Print primary cache",
                "Prints the state of the entire primary cache to the console and clipboard. WARNING: this may be A LOT of text.",
            ),


            #[cfg(target_arch = "wasm32")]
            Self::CopyDirectLink => (
                "Copy direct link",
                "Copy a link to the viewer with the URL parameter set to the current .rrd data source."
            ),

            #[cfg(target_arch = "wasm32")]
            Self::RestartWithWebGl => (
                "Restart with WebGL",
                "Reloads the webpage and force WebGL for rendering. All data will be lost."
            ),
            #[cfg(target_arch = "wasm32")]
            Self::RestartWithWebGpu => (
                "Restart with WebGPU",
                "Reloads the webpage and force WebGPU for rendering. All data will be lost."
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

        fn cmd_alt(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::ALT), key)
        }

        fn ctrl_shift(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::CTRL.plus(Modifiers::SHIFT), key)
        }

        match self {
            Self::SaveRecording => Some(cmd(Key::S)),
            Self::SaveRecordingSelection => Some(cmd_alt(Key::S)),
            Self::SaveBlueprint => None,
            Self::Open => Some(cmd(Key::O)),
            Self::CloseCurrentRecording => None,

            #[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
            Self::Quit => Some(KeyboardShortcut::new(Modifiers::ALT, Key::F4)),

            Self::OpenWebHelp => None,
            Self::OpenRerunDiscord => None,

            #[cfg(all(not(target_arch = "wasm32"), not(target_os = "windows")))]
            Self::Quit => Some(cmd(Key::Q)),

            Self::ResetViewer => Some(ctrl_shift(Key::R)),
            #[cfg(not(target_arch = "wasm32"))]
            Self::OpenProfiler => Some(ctrl_shift(Key::P)),
            Self::ToggleMemoryPanel => Some(ctrl_shift(Key::M)),
            Self::ToggleBlueprintPanel => Some(ctrl_shift(Key::B)),
            Self::ToggleSelectionPanel => Some(ctrl_shift(Key::S)),
            Self::ToggleTimePanel => Some(ctrl_shift(Key::T)),

            #[cfg(debug_assertions)]
            Self::ToggleBlueprintInspectionPanel => Some(ctrl_shift(Key::I)),

            #[cfg(debug_assertions)]
            Self::ToggleEguiDebugPanel => Some(ctrl_shift(Key::U)),

            #[cfg(not(target_arch = "wasm32"))]
            Self::ToggleFullscreen => Some(key(Key::F11)),
            #[cfg(not(target_arch = "wasm32"))]
            Self::ZoomIn => Some(egui::gui_zoom::kb_shortcuts::ZOOM_IN),
            #[cfg(not(target_arch = "wasm32"))]
            Self::ZoomOut => Some(egui::gui_zoom::kb_shortcuts::ZOOM_OUT),
            #[cfg(not(target_arch = "wasm32"))]
            Self::ZoomReset => Some(egui::gui_zoom::kb_shortcuts::ZOOM_RESET),

            Self::SelectionPrevious => Some(ctrl_shift(Key::ArrowLeft)),
            Self::SelectionNext => Some(ctrl_shift(Key::ArrowRight)),
            Self::ToggleCommandPalette => Some(cmd(Key::P)),

            Self::PlaybackTogglePlayPause => Some(key(Key::Space)),
            Self::PlaybackFollow => Some(cmd(Key::ArrowRight)),
            Self::PlaybackStepBack => Some(key(Key::ArrowLeft)),
            Self::PlaybackStepForward => Some(key(Key::ArrowRight)),
            Self::PlaybackRestart => Some(cmd(Key::ArrowLeft)),

            #[cfg(not(target_arch = "wasm32"))]
            Self::ScreenshotWholeApp => None,
            #[cfg(not(target_arch = "wasm32"))]
            Self::PrintDataStore => None,
            #[cfg(not(target_arch = "wasm32"))]
            Self::PrintBlueprintStore => None,
            #[cfg(not(target_arch = "wasm32"))]
            Self::ClearPrimaryCache => None,
            #[cfg(not(target_arch = "wasm32"))]
            Self::PrintPrimaryCache => None,

            #[cfg(target_arch = "wasm32")]
            Self::CopyDirectLink => None,

            #[cfg(target_arch = "wasm32")]
            Self::RestartWithWebGl => None,
            #[cfg(target_arch = "wasm32")]
            Self::RestartWithWebGpu => None,
        }
    }

    pub fn icon(self) -> Option<&'static crate::Icon> {
        match self {
            Self::OpenWebHelp => Some(&crate::icons::EXTERNAL_LINK),
            Self::OpenRerunDiscord => Some(&crate::icons::DISCORD),
            _ => None,
        }
    }

    pub fn is_link(self) -> bool {
        matches!(self, Self::OpenWebHelp | Self::OpenRerunDiscord)
    }

    #[must_use = "Returns the Command that was triggered by some keyboard shortcut"]
    pub fn listen_for_kb_shortcut(egui_ctx: &egui::Context) -> Option<UICommand> {
        use strum::IntoEnumIterator as _;

        let anything_has_focus = egui_ctx.memory(|mem| mem.focused().is_some());
        if anything_has_focus {
            return None; // e.g. we're typing in a TextField
        }

        let mut commands: Vec<(KeyboardShortcut, UICommand)> = Self::iter()
            .filter_map(|cmd| cmd.kb_shortcut().map(|kb_shortcut| (kb_shortcut, cmd)))
            .collect();

        // If the user pressed `Cmd-Shift-S` then egui will match that
        // with both `Cmd-Shift-S` and `Cmd-S`.
        // The reason is that `Shift` (and `Alt`) are sometimes required to produce certain keys,
        // such as `+` (`Shift =` on an american keyboard).
        // The result of this is that we bust check for `Cmd-Shift-S` before `Cmd-S`, etc.
        // So we order the commands here so that the commands with `Shift` and `Alt` in them
        // are checked first.
        commands.sort_by_key(|(kb_shortcut, _cmd)| {
            let num_shift_alts =
                kb_shortcut.modifiers.shift as i32 + kb_shortcut.modifiers.alt as i32;
            -num_shift_alts // most first
        });

        egui_ctx.input_mut(|input| {
            for (kb_shortcut, command) in commands {
                if input.consume_shortcut(&kb_shortcut) {
                    return Some(command);
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
        let mut response = ui.add(button).on_hover_text(self.tooltip());

        if self.is_link() {
            response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        }

        if response.clicked() {
            command_sender.send_ui(self);
            ui.close_menu();
        }

        response
    }

    pub fn menu_button(self, egui_ctx: &egui::Context) -> egui::Button<'static> {
        let mut button = if let Some(icon) = self.icon() {
            egui::Button::image_and_text(
                icon.as_image()
                    .fit_to_exact_size(crate::ReUi::small_icon_size()),
                self.text(),
            )
        } else {
            egui::Button::new(self.text())
        };

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

    pub fn tooltip_with_shortcut(self, egui_ctx: &egui::Context) -> String {
        format!(
            "{}{}",
            self.tooltip(),
            self.format_shortcut_tooltip_suffix(egui_ctx)
        )
    }
}

#[test]
fn check_for_clashing_command_shortcuts() {
    fn clashes(a: KeyboardShortcut, b: KeyboardShortcut) -> bool {
        if a.logical_key != b.logical_key {
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
