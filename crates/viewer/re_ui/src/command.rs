use egui::os::OperatingSystem;
use egui::{Id, Key, KeyboardShortcut, Modifiers};
use smallvec::{SmallVec, smallvec};

use crate::context_ext::ContextExt as _;
use crate::egui_ext::context_ext::ContextExt as _;

/// Interface for sending [`UICommand`] messages.
pub trait UICommandSender {
    fn send_ui(&self, command: UICommand);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SetPlaybackSpeed(pub egui::emath::OrderedFloat<f32>);

impl Default for SetPlaybackSpeed {
    fn default() -> Self {
        Self(egui::emath::OrderedFloat(1.0))
    }
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
    OpenUrl,
    Import,

    /// Save the current recording, or all selected recordings
    SaveRecording,
    SaveRecordingSelection,
    SaveBlueprint,
    CloseCurrentRecording,
    CloseAllEntries,

    NextRecording,
    PreviousRecording,

    NavigateBack,
    NavigateForward,

    Undo,
    Redo,

    #[cfg(not(target_arch = "wasm32"))]
    Quit,

    OpenWebHelp,
    OpenRerunDiscord,

    ResetViewer,
    ClearActiveBlueprint,
    ClearActiveBlueprintAndEnableHeuristics,

    #[cfg(not(target_arch = "wasm32"))]
    OpenProfiler,

    TogglePanelStateOverrides,
    ToggleMemoryPanel,
    ToggleTopPanel,
    ToggleBlueprintPanel,
    ExpandBlueprintPanel,
    ToggleSelectionPanel,
    ExpandSelectionPanel,
    ToggleTimePanel,
    ToggleChunkStoreBrowser,
    Settings,

    #[cfg(debug_assertions)]
    ToggleBlueprintInspectionPanel,

    #[cfg(debug_assertions)]
    ToggleEguiDebugPanel,

    ToggleFullscreen,
    #[cfg(not(target_arch = "wasm32"))]
    ZoomIn,
    #[cfg(not(target_arch = "wasm32"))]
    ZoomOut,
    #[cfg(not(target_arch = "wasm32"))]
    ZoomReset,

    ToggleCommandPalette,

    // Playback:
    PlaybackTogglePlayPause,
    PlaybackFollow,
    PlaybackStepBack,
    PlaybackStepForward,
    PlaybackBack,
    PlaybackForward,
    PlaybackBackFast,
    PlaybackForwardFast,
    PlaybackBeginning,
    PlaybackEnd,
    PlaybackRestart,
    PlaybackSpeed(SetPlaybackSpeed),

    // Dev-tools:
    #[cfg(not(target_arch = "wasm32"))]
    ScreenshotWholeApp,
    #[cfg(not(target_arch = "wasm32"))]
    PrintChunkStore,
    #[cfg(not(target_arch = "wasm32"))]
    PrintBlueprintStore,
    #[cfg(not(target_arch = "wasm32"))]
    PrintPrimaryCache,

    #[cfg(debug_assertions)]
    ResetEguiMemory,

    Share,
    CopyDirectLink,

    CopyTimeSelectionLink,

    CopyEntityHierarchy,

    // Graphics options:
    #[cfg(target_arch = "wasm32")]
    RestartWithWebGl,
    #[cfg(target_arch = "wasm32")]
    RestartWithWebGpu,

    // Redap commands
    AddRedapServer,
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
            Self::SaveRecording => (
                "Save recording…",
                "Save all data to a Rerun data file (.rrd)",
            ),

            Self::SaveRecordingSelection => (
                "Save current time selection…",
                "Save data for the current loop selection to a Rerun data file (.rrd)",
            ),

            Self::SaveBlueprint => (
                "Save blueprint…",
                "Save the current viewer setup as a Rerun blueprint file (.rbl)",
            ),

            Self::Open => (
                "Open file…",
                "Open any supported files (.rrd, images, meshes, …) in a new recording",
            ),
            Self::OpenUrl => (
                "Open from URL…",
                "Open or navigate to data from any supported URL",
            ),
            Self::Import => (
                "Import into current recording…",
                "Import any supported files (.rrd, images, meshes, …) in the current recording",
            ),

            Self::CloseCurrentRecording => (
                "Close current recording",
                "Close the current recording (unsaved data will be lost)",
            ),

            Self::CloseAllEntries => (
                "Close all recordings",
                "Close all open current recording (unsaved data will be lost)",
            ),

            Self::NextRecording => ("Next recording", "Switch to the next open recording"),
            Self::PreviousRecording => (
                "Previous recording",
                "Switch to the previous open recording",
            ),

            Self::NavigateBack => ("Back in history", "Go back in history"),
            Self::NavigateForward => ("Forward in history", "Go forward in history"),

            Self::Undo => (
                "Undo",
                "Undo the last blueprint edit for the open recording",
            ),
            Self::Redo => ("Redo", "Redo the last undone thing"),

            #[cfg(not(target_arch = "wasm32"))]
            Self::Quit => ("Quit", "Close the Rerun Viewer"),

            Self::OpenWebHelp => (
                "Help",
                "Visit the help page on our website, with troubleshooting tips and more",
            ),
            Self::OpenRerunDiscord => (
                "Rerun Discord",
                "Visit the Rerun Discord server, where you can ask questions and get help",
            ),

            Self::ResetViewer => (
                "Reset Viewer",
                "Reset the Viewer to how it looked the first time you ran it, forgetting all stored blueprints and UI state",
            ),

            Self::ClearActiveBlueprint => (
                "Reset to default blueprint",
                "Clear active blueprint and use the default blueprint instead. If no default blueprint is set, this will use a heuristic blueprint.",
            ),

            Self::ClearActiveBlueprintAndEnableHeuristics => (
                "Reset to heuristic blueprint",
                "Re-populate viewport with automatically chosen views using default visualizers",
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

            Self::TogglePanelStateOverrides => (
                "Toggle panel state overrides",
                "Toggle panel state between app blueprint and overrides",
            ),
            Self::ToggleTopPanel => ("Toggle top panel", "Toggle the top panel"),
            Self::ToggleBlueprintPanel => ("Toggle blueprint panel", "Toggle the left panel"),
            Self::ExpandBlueprintPanel => ("Expand blueprint panel", "Expand the left panel"),
            Self::ToggleSelectionPanel => ("Toggle selection panel", "Toggle the right panel"),
            Self::ExpandSelectionPanel => ("Expand selection panel", "Expand the right panel"),
            Self::ToggleTimePanel => ("Toggle time panel", "Toggle the bottom panel"),
            Self::ToggleChunkStoreBrowser => (
                "Toggle chunk store browser",
                "Toggle the chunk store browser",
            ),
            Self::Settings => ("Settings…", "Show the settings screen"),

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

            #[cfg(target_arch = "wasm32")]
            Self::ToggleFullscreen => (
                "Toggle fullscreen",
                "Toggle between full viewport dimensions and initial dimensions",
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

            Self::ToggleCommandPalette => ("Command palette…", "Toggle the Command Palette"),

            Self::PlaybackTogglePlayPause => ("Toggle play/pause", "Either play or pause the time"),
            Self::PlaybackFollow => ("Follow", "Follow on from end of timeline"),
            Self::PlaybackStepBack => (
                "Step backwards",
                "Move the time marker back to the previous point in time with any data",
            ),
            Self::PlaybackStepForward => (
                "Step forwards",
                "Move the time marker to the next point in time with any data",
            ),
            Self::PlaybackBack => (
                "Move backwards",
                "Move the time marker backward by 1 second",
            ),
            Self::PlaybackForward => (
                "Move forwards",
                "Move the time marker forward by 0.1 seconds",
            ),
            Self::PlaybackBackFast => (
                "Move backwards fast",
                "Move the time marker backwards by 1 second",
            ),
            Self::PlaybackForwardFast => (
                "Move forwards fast",
                "Move the time marker forwards by 0.1 seconds",
            ),
            Self::PlaybackBeginning => ("Go to beginning", "Go to beginning of timeline"),
            Self::PlaybackEnd => ("Go to end", "Go to end of timeline"),
            Self::PlaybackRestart => ("Restart", "Restart from beginning of timeline"),

            Self::PlaybackSpeed(_) => (
                "Set playback speed",
                "This is a chord, so you can press 5+0 to set the speed to 50x",
            ),

            #[cfg(not(target_arch = "wasm32"))]
            Self::ScreenshotWholeApp => (
                "Screenshot",
                "Copy screenshot of the whole app to clipboard",
            ),
            #[cfg(not(target_arch = "wasm32"))]
            Self::PrintChunkStore => (
                "Print datastore",
                "Prints the entire chunk store to the console and clipboard. WARNING: this may be A LOT of text.",
            ),
            #[cfg(not(target_arch = "wasm32"))]
            Self::PrintBlueprintStore => (
                "Print blueprint store",
                "Prints the entire blueprint store to the console and clipboard. WARNING: this may be A LOT of text.",
            ),
            #[cfg(not(target_arch = "wasm32"))]
            Self::PrintPrimaryCache => (
                "Print primary cache",
                "Prints the state of the entire primary cache to the console and clipboard. WARNING: this may be A LOT of text.",
            ),

            #[cfg(debug_assertions)]
            Self::ResetEguiMemory => (
                "Reset egui memory",
                "Reset egui memory, useful for debugging UI code.",
            ),

            Self::Share => ("Share…", "Share the current screen as a link"),
            Self::CopyDirectLink => (
                "Copy direct link",
                "Try to copy a shareable link to the current screen. This is not supported for all data sources & viewer states.",
            ),

            Self::CopyTimeSelectionLink => (
                "Copy link to selected time range",
                "Copy a link to the part of the active recording within the loop selection bounds.",
            ),

            Self::CopyEntityHierarchy => (
                "Copy entity hierarchy",
                "Copy the complete entity hierarchy tree of the currently active recording to the clipboard.",
            ),

            #[cfg(target_arch = "wasm32")]
            Self::RestartWithWebGl => (
                "Restart with WebGL",
                "Reloads the webpage and force WebGL for rendering. All data will be lost.",
            ),
            #[cfg(target_arch = "wasm32")]
            Self::RestartWithWebGpu => (
                "Restart with WebGPU",
                "Reloads the webpage and force WebGPU for rendering. All data will be lost.",
            ),

            Self::AddRedapServer => (
                "Connect to a server…",
                "Connect to a Redap server (experimental)",
            ),
        }
    }

    /// All keyboard shortcuts, with the primary first.
    pub fn kb_shortcuts(self, os: OperatingSystem) -> SmallVec<[KeyboardShortcut; 2]> {
        fn key(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::NONE, key)
        }

        fn ctrl(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::CTRL, key)
        }

        fn cmd(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::COMMAND, key)
        }

        fn alt(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::ALT, key)
        }

        fn shift(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::SHIFT, key)
        }

        fn cmd_shift(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, key)
        }

        fn cmd_alt(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, key)
        }

        fn ctrl_shift(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::CTRL | Modifiers::SHIFT, key)
        }

        match self {
            Self::SaveRecording => smallvec![cmd(Key::S)],
            Self::SaveRecordingSelection => smallvec![cmd_alt(Key::S)],
            Self::SaveBlueprint => smallvec![],
            Self::Open => smallvec![cmd(Key::O)],
            // Some browsers have a "paste and go" action.
            // But unfortunately there's no standard shortcut for this.
            // Claude however thinks it's this one (it's not). Let's go with that anyways!
            Self::OpenUrl => smallvec![cmd_shift(Key::L)],
            Self::Import => smallvec![cmd_shift(Key::O)],
            Self::CloseCurrentRecording => smallvec![],
            Self::CloseAllEntries => smallvec![],

            Self::NextRecording => smallvec![cmd_alt(Key::ArrowDown)],
            Self::PreviousRecording => smallvec![cmd_alt(Key::ArrowUp)],

            Self::NavigateBack => smallvec![cmd(Key::OpenBracket)],
            Self::NavigateForward => smallvec![cmd(Key::CloseBracket)],

            Self::Undo => smallvec![cmd(Key::Z)],
            Self::Redo => {
                if os == OperatingSystem::Mac {
                    smallvec![cmd_shift(Key::Z), cmd(Key::Y)]
                } else {
                    smallvec![ctrl(Key::Y), ctrl_shift(Key::Z)]
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            Self::Quit => {
                if os == OperatingSystem::Windows {
                    smallvec![KeyboardShortcut::new(Modifiers::ALT, Key::F4)]
                } else {
                    smallvec![cmd(Key::Q)]
                }
            }

            Self::OpenWebHelp => smallvec![],
            Self::OpenRerunDiscord => smallvec![],

            Self::ResetViewer => smallvec![ctrl_shift(Key::R)],
            Self::ClearActiveBlueprint => smallvec![],
            Self::ClearActiveBlueprintAndEnableHeuristics => smallvec![],

            #[cfg(not(target_arch = "wasm32"))]
            Self::OpenProfiler => smallvec![ctrl_shift(Key::P)],
            Self::ToggleMemoryPanel => smallvec![ctrl_shift(Key::M)],
            Self::TogglePanelStateOverrides => smallvec![],
            Self::ToggleTopPanel => smallvec![],
            Self::ToggleBlueprintPanel => smallvec![ctrl_shift(Key::B)],
            Self::ExpandBlueprintPanel => smallvec![],
            Self::ToggleSelectionPanel => smallvec![ctrl_shift(Key::S)],
            Self::ExpandSelectionPanel => smallvec![],
            Self::ToggleTimePanel => smallvec![ctrl_shift(Key::T)],
            Self::ToggleChunkStoreBrowser => smallvec![ctrl_shift(Key::D)],
            Self::Settings => smallvec![cmd(Key::Comma)],

            #[cfg(debug_assertions)]
            Self::ToggleBlueprintInspectionPanel => smallvec![ctrl_shift(Key::I)],

            #[cfg(debug_assertions)]
            Self::ToggleEguiDebugPanel => smallvec![ctrl_shift(Key::U)],

            Self::ToggleFullscreen => {
                if cfg!(target_arch = "wasm32") {
                    smallvec![]
                } else {
                    smallvec![key(Key::F11)]
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            Self::ZoomIn => smallvec![egui::gui_zoom::kb_shortcuts::ZOOM_IN],
            #[cfg(not(target_arch = "wasm32"))]
            Self::ZoomOut => smallvec![egui::gui_zoom::kb_shortcuts::ZOOM_OUT],
            #[cfg(not(target_arch = "wasm32"))]
            Self::ZoomReset => smallvec![egui::gui_zoom::kb_shortcuts::ZOOM_RESET],

            Self::ToggleCommandPalette => smallvec![cmd(Key::P)],

            Self::PlaybackTogglePlayPause => smallvec![key(Key::Space)],
            Self::PlaybackFollow => smallvec![alt(Key::ArrowRight)],
            Self::PlaybackStepBack => smallvec![cmd(Key::ArrowLeft)],
            Self::PlaybackStepForward => smallvec![cmd(Key::ArrowRight)],
            Self::PlaybackBack => smallvec![key(Key::ArrowLeft)],
            Self::PlaybackForward => smallvec![key(Key::ArrowRight)],
            Self::PlaybackBackFast => smallvec![shift(Key::ArrowLeft)],
            Self::PlaybackForwardFast => smallvec![shift(Key::ArrowRight)],
            Self::PlaybackBeginning => smallvec![key(Key::Home)],
            Self::PlaybackEnd => smallvec![key(Key::End)],
            Self::PlaybackRestart => smallvec![alt(Key::ArrowLeft)],

            Self::PlaybackSpeed(_) => {
                // This is a chord, so no single shortcut.
                smallvec![]
            }

            #[cfg(not(target_arch = "wasm32"))]
            Self::ScreenshotWholeApp => smallvec![],
            #[cfg(not(target_arch = "wasm32"))]
            Self::PrintChunkStore => smallvec![],
            #[cfg(not(target_arch = "wasm32"))]
            Self::PrintBlueprintStore => smallvec![],
            #[cfg(not(target_arch = "wasm32"))]
            Self::PrintPrimaryCache => smallvec![],

            #[cfg(debug_assertions)]
            Self::ResetEguiMemory => smallvec![],

            Self::Share => smallvec![cmd(Key::L)],
            Self::CopyDirectLink => smallvec![],

            Self::CopyTimeSelectionLink => smallvec![],

            Self::CopyEntityHierarchy => smallvec![ctrl_shift(Key::E)],

            #[cfg(target_arch = "wasm32")]
            Self::RestartWithWebGl => smallvec![],
            #[cfg(target_arch = "wasm32")]
            Self::RestartWithWebGpu => smallvec![],

            Self::AddRedapServer => smallvec![],
        }
    }

    /// Primary keyboard shortcut
    pub fn primary_kb_shortcut(self, os: OperatingSystem) -> Option<KeyboardShortcut> {
        self.kb_shortcuts(os).first().copied()
    }

    /// Return the keyboard shortcut for this command, nicely formatted
    // TODO(emilk): use Help/IconText instead
    pub fn formatted_kb_shortcut(self, egui_ctx: &egui::Context) -> Option<String> {
        if matches!(self, Self::PlaybackSpeed(_)) {
            return Some("01-99".to_owned());
        }
        // Note: we only show the primary shortcut to the user.
        // The fallbacks are there for people who have muscle memory for the other shortcuts.
        self.primary_kb_shortcut(egui_ctx.os())
            .map(|shortcut| egui_ctx.format_shortcut(&shortcut))
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

    fn handle_playback_chord(ctx: &egui::Context) -> Option<Self> {
        const CHORD_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);
        const NUMBER_KEYS: [Key; 10] = [
            Key::Num0,
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
        ];

        fn key_to_digit(key: Key) -> Option<char> {
            let i = NUMBER_KEYS.iter().position(|&k| k == key)?;
            char::from_digit(i as u32, 10)
        }

        #[derive(Default, Clone)]
        struct PlaybackChordState {
            last_key_time: Option<web_time::Instant>,
            accumulated: String,
        }

        if ctx.text_edit_focused() {
            return None;
        }

        let mut chord_state = ctx.data_mut(|data| {
            data.get_temp_mut_or_default::<PlaybackChordState>(Id::NULL)
                .clone()
        });

        let now = web_time::Instant::now();

        let pressed_number = ctx.input(|i| {
            let mut pressed_number = NUMBER_KEYS.iter().find(|&&k| i.key_pressed(k)).copied();
            let has_other = i.keys_down.iter().any(|k| !NUMBER_KEYS.contains(k));

            if has_other || i.modifiers.any() {
                chord_state = PlaybackChordState::default();
                pressed_number = None;
            }

            pressed_number
        });

        // Check if timeout expired - clear old state
        if let Some(last_time) = chord_state.last_key_time
            && now.duration_since(last_time) >= CHORD_TIMEOUT
        {
            chord_state = PlaybackChordState::default();
        }

        let mut command = None;

        // Handle number key press
        if let Some(key) = pressed_number {
            if let Some(digit) = key_to_digit(key) {
                chord_state.accumulated.push(digit);
            }

            chord_state.last_key_time = Some(now);

            // Leading zeros should divide the speed by 10 for each zero.
            // So e.g. 05 = 0.5x speed, 005 = 0.05x speed, etc.
            let leading_zeros = chord_state
                .accumulated
                .chars()
                .take_while(|&c| c == '0')
                .count();

            let factor = 10usize.pow(leading_zeros as u32);

            if let Ok(speed) = chord_state.accumulated.parse::<f32>()
                && speed > 0.0
            {
                command = Some(Self::PlaybackSpeed(SetPlaybackSpeed(
                    egui::emath::OrderedFloat(speed / factor as f32),
                )));
            }
        }

        ctx.data_mut(|data| data.insert_temp(Id::NULL, chord_state.clone()));

        command
    }

    #[must_use = "Returns the Command that was triggered by some keyboard shortcut"]
    pub fn listen_for_kb_shortcut(egui_ctx: &egui::Context) -> Option<Self> {
        fn conflicts_with_text_editing(kb_shortcut: &KeyboardShortcut) -> bool {
            // TODO(emilk): move this into egui
            kb_shortcut.modifiers.is_none()
                || matches!(
                    kb_shortcut.logical_key,
                    Key::Space
                        | Key::ArrowLeft
                        | Key::ArrowRight
                        | Key::ArrowUp
                        | Key::ArrowDown
                        | Key::Home
                        | Key::End
                )
        }

        use strum::IntoEnumIterator as _;

        let text_edit_has_focus = egui_ctx.text_edit_focused();

        let mut commands: Vec<(KeyboardShortcut, Self)> = Self::iter()
            .flat_map(|cmd| {
                cmd.kb_shortcuts(egui_ctx.os())
                    .into_iter()
                    .map(move |kb_shortcut| (kb_shortcut, cmd))
            })
            .collect();

        // If the user pressed `Cmd-Shift-S` then egui will match that
        // with both `Cmd-Shift-S` and `Cmd-S`.
        // The reason is that `Shift` (and `Alt`) are sometimes required to produce certain keys,
        // such as `+` (`Shift =` on an american keyboard).
        // The result of this is that we must check for `Cmd-Shift-S` before `Cmd-S`, etc.
        // So we order the commands here so that the commands with `Shift` and `Alt` in them
        // are checked first.
        commands.sort_by_key(|(kb_shortcut, _cmd)| {
            let num_shift_alts =
                kb_shortcut.modifiers.shift as i32 + kb_shortcut.modifiers.alt as i32;
            -num_shift_alts // most first
        });

        let command = egui_ctx.input_mut(|input| {
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
        });

        if command.is_none() {
            Self::handle_playback_chord(egui_ctx)
        } else {
            command
        }
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
            ui.close();
        }

        response
    }

    pub fn menu_button(self, egui_ctx: &egui::Context) -> egui::Button<'static> {
        let tokens = egui_ctx.tokens();

        let mut button = if let Some(icon) = self.icon() {
            egui::Button::image_and_text(
                icon.as_image()
                    .tint(tokens.label_button_icon_color)
                    .fit_to_exact_size(tokens.small_icon_size),
                self.text(),
            )
        } else {
            egui::Button::new(self.text())
        };

        if let Some(shortcut_text) = self.formatted_kb_shortcut(egui_ctx) {
            button = button.shortcut_text(shortcut_text);
        }

        button
    }

    /// Show name of command and how to activate it
    pub fn tooltip_ui(self, ui: &mut egui::Ui) {
        let os = ui.ctx().os();

        let (label, details) = self.text_and_tooltip();

        if let Some(shortcut) = self.primary_kb_shortcut(os) {
            crate::Help::new_without_title()
                .control(label, crate::IconText::from_keyboard_shortcut(os, shortcut))
                .ui(ui);
        } else {
            ui.label(label);
        }

        ui.set_max_width(220.0);
        ui.label(details);
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

    for os in [
        OperatingSystem::Mac,
        OperatingSystem::Windows,
        OperatingSystem::Nix,
    ] {
        for a_cmd in UICommand::iter() {
            for a_shortcut in a_cmd.kb_shortcuts(os) {
                for b_cmd in UICommand::iter() {
                    if a_cmd == b_cmd {
                        continue;
                    }
                    for b_shortcut in b_cmd.kb_shortcuts(os) {
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
}
