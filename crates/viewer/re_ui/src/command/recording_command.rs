use egui::os::OperatingSystem;
use egui::{Id, Key, KeyboardShortcut, Modifiers};
use re_log_types::StoreId;
use smallvec::{SmallVec, smallvec};

use super::CommandEnvironment;
use crate::context_ext::ContextExt as _;

/// Interface for sending [`RecordingCommand`] messages.
pub trait RecordingCommandSender {
    fn send_recording_command(&self, command: RecordingCommand);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SetPlaybackSpeed(pub egui::emath::OrderedFloat<f32>);

impl Default for SetPlaybackSpeed {
    fn default() -> Self {
        Self(egui::emath::OrderedFloat(1.0))
    }
}

/// A command that acts on a specific recording.
///
/// Unlike [`super::UICommand`], these carry the [`StoreId`] of the recording they act on,
/// so they can be used both from the command palette (acting on the active recording)
/// and from menus and buttons (acting on a specific recording).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RecordingCommand {
    /// The recording this command acts on.
    pub recording_id: StoreId,

    /// What to do with the recording.
    pub kind: RecordingCommandKind,
}

/// What a [`RecordingCommand`] does to its recording.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, strum_macros::EnumIter)]
pub enum RecordingCommandKind {
    // Listed in the order they show up in the command palette by default!
    /// Save the recording, or all selected recordings.
    Save,

    /// Save the current time selection of the recording.
    SaveTimeSelection,

    /// Save the active blueprint of the recording.
    SaveBlueprint,

    /// Close the recording.
    Close,

    /// Undo the latest blueprint edit.
    Undo,

    /// Redo the latest undone blueprint edit.
    Redo,

    /// Add a view or container to the viewport.
    AddViewOrContainer,

    /// Reset the active blueprint to the default one.
    ClearActiveBlueprint,

    /// Reset the active blueprint to a heuristic one.
    ClearActiveBlueprintAndEnableHeuristics,

    ToggleTimePanel,
    ToggleChunkStoreBrowser,

    #[cfg(debug_assertions)]
    ToggleBlueprintInspectionPanel,

    // Playback:
    PlaybackTogglePlayPause,
    PlaybackStepBack,
    PlaybackStepForward,
    PlaybackBack,
    PlaybackForward,
    PlaybackBackFast,
    PlaybackForwardFast,
    PlaybackBeginning,
    PlaybackEndAndFollow,
    PlaybackSpeed(SetPlaybackSpeed),

    // Dev-tools:
    #[cfg(not(target_arch = "wasm32"))]
    PrintChunkStore,
    #[cfg(not(target_arch = "wasm32"))]
    PrintBlueprintStore,
    #[cfg(not(target_arch = "wasm32"))]
    PrintPrimaryCache,
}

impl RecordingCommand {
    /// All commands that act on the given recording.
    pub fn all_for_recording(recording_id: &StoreId) -> impl Iterator<Item = Self> {
        use strum::IntoEnumIterator as _;
        let recording_id = recording_id.clone();
        RecordingCommandKind::iter().map(move |kind| Self {
            recording_id: recording_id.clone(),
            kind,
        })
    }

    /// Show this command as a menu-button.
    ///
    /// If clicked, enqueue the command.
    pub fn menu_button_ui(
        self,
        ui: &mut egui::Ui,
        command_sender: &impl RecordingCommandSender,
    ) -> egui::Response {
        let Self { recording_id, kind } = self;
        kind.menu_button_ui(ui, Some(&recording_id), command_sender)
    }
}

impl RecordingCommandKind {
    /// Is this a "timeline" command, i.e. playback bound to a space/arrow/home/end key
    /// (or the playback-speed chord)?
    ///
    /// These keys must be consumed early (in `on_begin_pass`) so egui doesn't first use
    /// them to move keyboard focus or scroll — see [`super::consume_timeline_shortcut`].
    pub fn is_timeline(self) -> bool {
        matches!(
            self,
            Self::PlaybackTogglePlayPause
                | Self::PlaybackStepBack
                | Self::PlaybackStepForward
                | Self::PlaybackBack
                | Self::PlaybackForward
                | Self::PlaybackBackFast
                | Self::PlaybackForwardFast
                | Self::PlaybackBeginning
                | Self::PlaybackEndAndFollow
                | Self::PlaybackSpeed(_)
        )
    }

    /// Pair this command with the active recording (from `env`) to make it dispatchable.
    ///
    /// Returns `None` when there is no active recording.
    pub fn for_environment(self, env: &CommandEnvironment) -> Option<RecordingCommand> {
        env.recording.clone().map(|recording_id| RecordingCommand {
            recording_id,
            kind: self,
        })
    }

    pub fn text(self) -> &'static str {
        self.text_and_tooltip().0
    }

    pub fn tooltip(self) -> &'static str {
        self.text_and_tooltip().1
    }

    pub fn text_and_tooltip(self) -> (&'static str, &'static str) {
        match self {
            Self::Save => (
                "Save recording…",
                "Save all data to a Rerun data file (.rrd)",
            ),

            Self::SaveTimeSelection => (
                "Save current time selection…",
                "Save data for the current loop selection to a Rerun data file (.rrd)",
            ),

            Self::SaveBlueprint => (
                "Save blueprint…",
                "Save the current viewer setup as a Rerun blueprint file (.rbl)",
            ),

            Self::Close => (
                "Close current recording",
                "Close the current recording (unsaved data will be lost)",
            ),

            Self::Undo => (
                "Undo",
                "Undo the last blueprint edit for the open recording",
            ),
            Self::Redo => ("Redo", "Redo the last undone thing"),

            Self::AddViewOrContainer => (
                "Add view or container…",
                "Add a new view or container to the viewport",
            ),

            Self::ClearActiveBlueprint => (
                "Reset to default blueprint",
                "Clear active blueprint and use the default blueprint instead. If no default blueprint is set, this will use a heuristic blueprint.",
            ),

            Self::ClearActiveBlueprintAndEnableHeuristics => (
                "Reset to heuristic blueprint",
                "Re-populate viewport with automatically chosen views using default visualizers",
            ),

            Self::ToggleTimePanel => ("Toggle time panel", "Toggle the bottom panel"),
            Self::ToggleChunkStoreBrowser => (
                "Toggle chunk store browser",
                "Toggle the chunk store browser",
            ),

            #[cfg(debug_assertions)]
            Self::ToggleBlueprintInspectionPanel => (
                "Toggle blueprint inspection panel",
                "Inspect the timeline of the internal blueprint data.",
            ),

            Self::PlaybackTogglePlayPause => ("Toggle play/pause", "Either play or pause the time"),
            Self::PlaybackStepBack => (
                "Step backwards",
                "Move the time marker back to the previous point in time with any data",
            ),
            Self::PlaybackStepForward => (
                "Step forwards",
                "Move the time marker to the next point in time with any data",
            ),
            Self::PlaybackBack => ("Backward 1", "Move the time marker backward by 1 second"),
            Self::PlaybackForward => ("Forward 1", "Move the time marker forward by 0.1 seconds"),
            Self::PlaybackBackFast => ("Backward 10", "Move the time marker backwards by 1 second"),
            Self::PlaybackForwardFast => {
                ("Forward 10", "Move the time marker forwards by 0.1 seconds")
            }
            Self::PlaybackBeginning => ("Start of timeline", "Go to beginning of timeline"),
            Self::PlaybackEndAndFollow => (
                "End of timeline",
                "Go to end of timeline and follow the latest data as it streams in",
            ),

            Self::PlaybackSpeed(_) => (
                "Set playback speed",
                "This is a chord, so you can press 5+0 to set the speed to 50x",
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
        }
    }

    pub fn icon(self) -> Option<&'static crate::Icon> {
        match self {
            Self::AddViewOrContainer => Some(&crate::icons::ADD),
            Self::ClearActiveBlueprint | Self::ClearActiveBlueprintAndEnableHeuristics => {
                Some(&crate::icons::RESET)
            }
            _ => None,
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
            Self::Save => smallvec![cmd(Key::S)],
            Self::SaveTimeSelection => smallvec![cmd_alt(Key::S)],
            Self::SaveBlueprint => smallvec![],
            Self::Close => smallvec![],

            Self::Undo => smallvec![cmd(Key::Z)],
            Self::Redo => {
                if os == OperatingSystem::Mac {
                    smallvec![cmd_shift(Key::Z), cmd(Key::Y)]
                } else {
                    smallvec![ctrl(Key::Y), ctrl_shift(Key::Z)]
                }
            }

            Self::AddViewOrContainer => smallvec![],
            Self::ClearActiveBlueprint => smallvec![],
            Self::ClearActiveBlueprintAndEnableHeuristics => smallvec![],

            Self::ToggleTimePanel => smallvec![ctrl_shift(Key::T)],
            Self::ToggleChunkStoreBrowser => smallvec![ctrl_shift(Key::D)],

            #[cfg(debug_assertions)]
            Self::ToggleBlueprintInspectionPanel => smallvec![ctrl_shift(Key::I)],

            Self::PlaybackTogglePlayPause => smallvec![key(Key::Space)],
            Self::PlaybackStepBack => smallvec![cmd(Key::ArrowLeft)],
            Self::PlaybackStepForward => smallvec![cmd(Key::ArrowRight)],
            Self::PlaybackBack => smallvec![key(Key::ArrowLeft)],
            Self::PlaybackForward => smallvec![key(Key::ArrowRight)],
            Self::PlaybackBackFast => smallvec![shift(Key::ArrowLeft)],
            Self::PlaybackForwardFast => smallvec![shift(Key::ArrowRight)],
            Self::PlaybackBeginning => smallvec![key(Key::Home)],
            Self::PlaybackEndAndFollow => smallvec![key(Key::End), alt(Key::ArrowRight)],

            Self::PlaybackSpeed(_) => {
                // This is a chord, so no single shortcut.
                smallvec![]
            }

            #[cfg(not(target_arch = "wasm32"))]
            Self::PrintChunkStore | Self::PrintBlueprintStore | Self::PrintPrimaryCache => {
                smallvec![]
            }
        }
    }

    /// Primary keyboard shortcut
    pub fn primary_kb_shortcut(self, os: OperatingSystem) -> Option<KeyboardShortcut> {
        self.kb_shortcuts(os).first().copied()
    }

    /// Return the keyboard shortcut for this command, nicely formatted
    pub fn formatted_kb_shortcut(self, egui_ctx: &egui::Context) -> Option<String> {
        if matches!(self, Self::PlaybackSpeed(_)) {
            return Some("01-99".to_owned());
        }
        // Note: we only show the primary shortcut to the user.
        // The fallbacks are there for people who have muscle memory for the other shortcuts.
        self.primary_kb_shortcut(egui_ctx.os())
            .map(|shortcut| egui_ctx.format_shortcut(&shortcut))
    }

    /// Show this command as a menu-button.
    ///
    /// Disabled if `recording_id` is `None`;
    /// otherwise, if clicked, enqueue the command for that recording.
    pub fn menu_button_ui(
        self,
        ui: &mut egui::Ui,
        recording_id: Option<&StoreId>,
        command_sender: &impl RecordingCommandSender,
    ) -> egui::Response {
        let button = self.menu_button(ui.ctx());
        let response = ui
            .add_enabled(recording_id.is_some(), button)
            .on_hover_text(self.tooltip());

        if response.clicked()
            && let Some(recording_id) = recording_id
        {
            command_sender.send_recording_command(RecordingCommand {
                recording_id: recording_id.clone(),
                kind: self,
            });
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
        let os = ui.os();

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

    /// A chord for setting the playback speed: type e.g. `5` then `0` for 50x speed.
    pub(super) fn handle_playback_chord(ctx: &egui::Context) -> Option<Self> {
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
                // Cap the length so key-repeat (e.g. holding `0`) can't grow this
                // unboundedly and overflow the `10.pow(leading_zeros)` below.
                if chord_state.accumulated.len() < 8 {
                    chord_state.accumulated.push(digit);
                }
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
}
