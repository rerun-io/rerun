use egui::KeyboardShortcut;
use egui::os::OperatingSystem;
use smallvec::SmallVec;

use super::{
    BoundCommand, CommandEnvironment, RecordingCommand, RecordingCommandKind, RedapServerCommand,
    RedapServerCommandKind, TableCommandKind, UICommand,
};

/// What a command acts on, which decides where its target comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandScope {
    /// Acts on the viewer as a whole. Needs no target.
    Global,

    /// Acts on a recording: the active one, unless the caller names another.
    Recording,

    /// Acts on a Redap server: the selected one, unless the caller names another.
    RedapServer,

    /// Acts on the table (or dataset) currently being viewed.
    Table,
}

impl CommandScope {
    /// The prefix of every command id in this scope, without the separating dot.
    pub fn id_prefix(self) -> &'static str {
        match self {
            Self::Global => "ui",
            Self::Recording => "recording",
            Self::RedapServer => "server",
            Self::Table => "table",
        }
    }
}

/// A command of any type, without the target it acts on.
///
/// This is what the command palette lists, and what a remote caller names by
/// [`Self::id`]: the target is filled in by [`Self::bind`] from the
/// [`CommandEnvironment`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommandKind {
    Ui(UICommand),
    Recording(RecordingCommandKind),
    RedapServer(RedapServerCommandKind),
    Table(TableCommandKind),
}

impl CommandKind {
    /// Every command that can be listed and run by id, in command palette order.
    ///
    /// Leaves out [`RecordingCommandKind::PlaybackSpeed`]: it takes an argument (the speed),
    /// so on its own it could only ever reset the speed to 1x.
    pub fn all() -> impl Iterator<Item = Self> {
        use strum::IntoEnumIterator as _;

        itertools::chain!(
            UICommand::iter().map(Self::Ui),
            RecordingCommandKind::iter()
                .filter(|kind| !matches!(kind, RecordingCommandKind::PlaybackSpeed(_)))
                .map(Self::Recording),
            RedapServerCommandKind::iter().map(Self::RedapServer),
            TableCommandKind::iter().map(Self::Table),
        )
    }

    /// Look up a command by its [`Self::id`].
    pub fn from_id(id: &str) -> Option<Self> {
        Self::all().find(|kind| kind.id() == id)
    }

    /// Identifier, e.g. `recording.toggle_play_pause`: the scope prefix, then the variant name.
    ///
    /// This is what remote callers name the command by, and often all an agent reads about it
    /// before deciding to run it, so the variant name must say what the command does on its own.
    pub fn id(self) -> String {
        let name: &'static str = match self {
            Self::Ui(cmd) => cmd.into(),
            Self::Recording(kind) => kind.into(),
            Self::RedapServer(kind) => kind.into(),
            Self::Table(kind) => kind.into(),
        };
        format!("{}.{name}", self.scope().id_prefix())
    }

    /// What the command acts on.
    pub fn scope(self) -> CommandScope {
        match self {
            Self::Ui(_) => CommandScope::Global,
            Self::Recording(_) => CommandScope::Recording,
            Self::RedapServer(_) => CommandScope::RedapServer,
            Self::Table(_) => CommandScope::Table,
        }
    }

    /// The name shown in the command palette.
    pub fn text(self) -> &'static str {
        self.text_and_tooltip().0
    }

    /// One-line description of what the command does.
    pub fn tooltip(self) -> &'static str {
        self.text_and_tooltip().1
    }

    /// [`Self::text`] and [`Self::tooltip`] together.
    pub fn text_and_tooltip(self) -> (&'static str, &'static str) {
        match self {
            Self::Ui(cmd) => cmd.text_and_tooltip(),
            Self::Recording(kind) => kind.text_and_tooltip(),
            Self::RedapServer(kind) => kind.text_and_tooltip(),
            Self::Table(kind) => kind.text_and_tooltip(),
        }
    }

    /// All keyboard shortcuts, with the primary first.
    pub fn kb_shortcuts(self, os: OperatingSystem) -> SmallVec<[KeyboardShortcut; 2]> {
        match self {
            Self::Ui(cmd) => cmd.kb_shortcuts(os),
            Self::Recording(kind) => kind.kb_shortcuts(os),
            Self::RedapServer(kind) => kind.kb_shortcuts(os),
            Self::Table(kind) => kind.kb_shortcuts(os),
        }
    }

    /// The primary keyboard shortcut, formatted for the platform of `egui_ctx`.
    pub fn formatted_kb_shortcut(self, egui_ctx: &egui::Context) -> Option<String> {
        match self {
            Self::Ui(cmd) => cmd.formatted_kb_shortcut(egui_ctx),
            Self::Recording(kind) => kind.formatted_kb_shortcut(egui_ctx),
            Self::RedapServer(kind) => kind.formatted_kb_shortcut(egui_ctx),
            Self::Table(kind) => kind.formatted_kb_shortcut(egui_ctx),
        }
    }

    /// Does this command only exist in debug builds?
    ///
    /// Such commands are marked with an orange "debug only" badge in the UI.
    #[cfg(debug_assertions)]
    pub fn is_debug_only(self) -> bool {
        match self {
            Self::Ui(cmd) => cmd.is_debug_only(),
            Self::Recording(kind) => kind.is_debug_only(),
            Self::RedapServer(_) | Self::Table(_) => false,
        }
    }

    /// Does running this command open a blocking native modal, such as a file dialog?
    ///
    /// While it is open the viewer stops painting and answering until a person dismisses it,
    /// so a remote caller must not run these unattended.
    pub fn opens_blocking_native_modal(self) -> bool {
        match self {
            Self::Ui(cmd) => match cmd {
                UICommand::OpenFile | UICommand::ImportFileIntoCurrentRecording => true,

                #[cfg(not(target_arch = "wasm32"))]
                UICommand::CaptureProfileTrace => true,

                UICommand::OpenUrlDialog
                | UICommand::CloseAllRecordings
                | UICommand::SwitchToNextRecording
                | UICommand::SwitchToPreviousRecording
                | UICommand::NavigateBackInHistory
                | UICommand::NavigateForwardInHistory
                | UICommand::OpenRerunWebsite
                | UICommand::OpenDocsWebsite
                | UICommand::OpenDiscordWebsite
                | UICommand::ResetViewer
                | UICommand::TogglePanelStateOverrides
                | UICommand::ToggleDevPanel
                | UICommand::ToggleChunkStoreBrowser
                | UICommand::ToggleTopPanel
                | UICommand::ToggleBlueprintPanel
                | UICommand::ExpandBlueprintPanel
                | UICommand::ToggleSelectionPanel
                | UICommand::ExpandSelectionPanel
                | UICommand::ToggleAgentPanel
                | UICommand::OpenSettings
                | UICommand::ToggleFullscreen
                | UICommand::ToggleCommandPalette
                | UICommand::OpenShareDialog
                | UICommand::CopyDirectLinkToClipboard
                | UICommand::CopyTimeSelectionLinkToClipboard
                | UICommand::CopyEntityHierarchyToClipboard
                | UICommand::OpenAddServerDialog => false,

                #[cfg(not(target_arch = "wasm32"))]
                UICommand::Quit
                | UICommand::OpenProfiler
                | UICommand::ZoomInUi
                | UICommand::ZoomOutUi
                | UICommand::ResetUiZoom
                | UICommand::CopyScreenshotToClipboard => false,

                #[cfg(debug_assertions)]
                UICommand::ToggleEguiDebugPanel | UICommand::ResetEguiMemory => false,

                #[cfg(target_arch = "wasm32")]
                UICommand::RestartWithWebGl | UICommand::RestartWithWebGpu => false,
            },

            Self::Recording(kind) => match kind {
                RecordingCommandKind::Save
                | RecordingCommandKind::SaveTimeSelection
                | RecordingCommandKind::SaveBlueprint => true,

                RecordingCommandKind::Close
                | RecordingCommandKind::UndoBlueprintEdit
                | RecordingCommandKind::RedoBlueprintEdit
                | RecordingCommandKind::OpenAddViewOrContainerDialog
                | RecordingCommandKind::ResetBlueprintToDefault
                | RecordingCommandKind::ResetBlueprintToHeuristic
                | RecordingCommandKind::ToggleTimePanel
                | RecordingCommandKind::TogglePlayPause
                | RecordingCommandKind::SeekToPreviousEvent
                | RecordingCommandKind::SeekToNextEvent
                | RecordingCommandKind::SeekBackwardShort
                | RecordingCommandKind::SeekForwardShort
                | RecordingCommandKind::SeekBackwardLong
                | RecordingCommandKind::SeekForwardLong
                | RecordingCommandKind::SeekToStart
                | RecordingCommandKind::SeekToEndAndFollow
                | RecordingCommandKind::PlaybackSpeed(_) => false,

                #[cfg(debug_assertions)]
                RecordingCommandKind::ToggleBlueprintInspectionPanel => false,

                #[cfg(not(target_arch = "wasm32"))]
                RecordingCommandKind::PrintChunkStore
                | RecordingCommandKind::PrintBlueprintStore
                | RecordingCommandKind::PrintPrimaryCache => false,
            },

            Self::RedapServer(kind) => match kind {
                RedapServerCommandKind::Refresh
                | RedapServerCommandKind::OpenEditDialog
                | RedapServerCommandKind::CopyUrlToClipboard
                | RedapServerCommandKind::Remove => false,
            },

            Self::Table(kind) => match kind {
                TableCommandKind::Refresh | TableCommandKind::ResetBlueprintToDefault => false,
            },
        }
    }

    /// Does running this command throw away state that cannot be brought back from within the
    /// viewer: open data, blueprint edits, a configured server, or the viewer itself?
    pub fn is_destructive(self) -> bool {
        match self {
            Self::Ui(cmd) => match cmd {
                UICommand::CloseAllRecordings | UICommand::ResetViewer => true,

                #[cfg(not(target_arch = "wasm32"))]
                UICommand::Quit => true,

                #[cfg(target_arch = "wasm32")]
                UICommand::RestartWithWebGl | UICommand::RestartWithWebGpu => true,

                UICommand::OpenFile
                | UICommand::OpenUrlDialog
                | UICommand::ImportFileIntoCurrentRecording
                | UICommand::SwitchToNextRecording
                | UICommand::SwitchToPreviousRecording
                | UICommand::NavigateBackInHistory
                | UICommand::NavigateForwardInHistory
                | UICommand::OpenRerunWebsite
                | UICommand::OpenDocsWebsite
                | UICommand::OpenDiscordWebsite
                | UICommand::TogglePanelStateOverrides
                | UICommand::ToggleDevPanel
                | UICommand::ToggleChunkStoreBrowser
                | UICommand::ToggleTopPanel
                | UICommand::ToggleBlueprintPanel
                | UICommand::ExpandBlueprintPanel
                | UICommand::ToggleSelectionPanel
                | UICommand::ExpandSelectionPanel
                | UICommand::ToggleAgentPanel
                | UICommand::OpenSettings
                | UICommand::ToggleFullscreen
                | UICommand::ToggleCommandPalette
                | UICommand::OpenShareDialog
                | UICommand::CopyDirectLinkToClipboard
                | UICommand::CopyTimeSelectionLinkToClipboard
                | UICommand::CopyEntityHierarchyToClipboard
                | UICommand::OpenAddServerDialog => false,

                #[cfg(not(target_arch = "wasm32"))]
                UICommand::OpenProfiler
                | UICommand::CaptureProfileTrace
                | UICommand::ZoomInUi
                | UICommand::ZoomOutUi
                | UICommand::ResetUiZoom
                | UICommand::CopyScreenshotToClipboard => false,

                #[cfg(debug_assertions)]
                UICommand::ToggleEguiDebugPanel | UICommand::ResetEguiMemory => false,
            },

            Self::Recording(kind) => match kind {
                RecordingCommandKind::Close
                | RecordingCommandKind::ResetBlueprintToDefault
                | RecordingCommandKind::ResetBlueprintToHeuristic => true,

                RecordingCommandKind::Save
                | RecordingCommandKind::SaveTimeSelection
                | RecordingCommandKind::SaveBlueprint
                | RecordingCommandKind::UndoBlueprintEdit
                | RecordingCommandKind::RedoBlueprintEdit
                | RecordingCommandKind::OpenAddViewOrContainerDialog
                | RecordingCommandKind::ToggleTimePanel
                | RecordingCommandKind::TogglePlayPause
                | RecordingCommandKind::SeekToPreviousEvent
                | RecordingCommandKind::SeekToNextEvent
                | RecordingCommandKind::SeekBackwardShort
                | RecordingCommandKind::SeekForwardShort
                | RecordingCommandKind::SeekBackwardLong
                | RecordingCommandKind::SeekForwardLong
                | RecordingCommandKind::SeekToStart
                | RecordingCommandKind::SeekToEndAndFollow
                | RecordingCommandKind::PlaybackSpeed(_) => false,

                #[cfg(debug_assertions)]
                RecordingCommandKind::ToggleBlueprintInspectionPanel => false,

                #[cfg(not(target_arch = "wasm32"))]
                RecordingCommandKind::PrintChunkStore
                | RecordingCommandKind::PrintBlueprintStore
                | RecordingCommandKind::PrintPrimaryCache => false,
            },

            Self::RedapServer(kind) => match kind {
                RedapServerCommandKind::Remove => true,
                RedapServerCommandKind::Refresh
                | RedapServerCommandKind::OpenEditDialog
                | RedapServerCommandKind::CopyUrlToClipboard => false,
            },

            Self::Table(kind) => match kind {
                TableCommandKind::ResetBlueprintToDefault => true,
                TableCommandKind::Refresh => false,
            },
        }
    }

    /// Pair this command with its target from `env`, making it dispatchable.
    ///
    /// Returns `None` when the command cannot run in `env`: it is not supported on this
    /// platform, or there is no target for it (no active recording, no selected server, no
    /// viewed table), or the target does not allow it (e.g. editing the built-in catalog).
    pub fn bind(self, env: &CommandEnvironment) -> Option<BoundCommand> {
        match self {
            Self::Ui(cmd) => cmd.is_supported().then_some(BoundCommand::Ui(cmd)),
            Self::Recording(kind) => kind.for_environment(env).map(BoundCommand::Recording),
            Self::RedapServer(kind) => kind.for_environment(env).map(BoundCommand::RedapServer),
            Self::Table(kind) => kind.for_environment(env).map(BoundCommand::Table),
        }
    }
}

impl BoundCommand {
    /// The command, without its target.
    pub fn kind(&self) -> CommandKind {
        match self {
            Self::Ui(cmd) => CommandKind::Ui(*cmd),
            Self::Recording(cmd) => CommandKind::Recording(cmd.kind),
            Self::RedapServer(cmd) => CommandKind::RedapServer(cmd.kind),
            Self::Table(cmd) => CommandKind::Table(cmd.kind),
        }
    }
}

/// A command as the command palette shows it.
#[derive(Clone, Debug)]
pub struct ListedCommand {
    /// The command, paired with the target it acts on.
    pub command: BoundCommand,

    /// Can it run? Unavailable commands are shown grayed out.
    pub enabled: bool,
}

/// The commands the command palette offers in `env`, in palette order.
///
/// Global commands are always listed, grayed out where unsupported.
/// Recording, server, and table commands are only listed when `env` has a target for them;
/// server commands that need an editable server are grayed out for the built-in catalog.
///
/// Every enabled entry is exactly what [`CommandKind::bind`] returns for its kind.
pub fn palette_commands(env: &CommandEnvironment) -> Vec<ListedCommand> {
    use strum::IntoEnumIterator as _;

    let mut commands: Vec<ListedCommand> = UICommand::iter()
        .map(|cmd| ListedCommand {
            command: BoundCommand::Ui(cmd),
            enabled: cmd.is_supported(),
        })
        .collect();

    if let Some(recording_id) = &env.recording {
        commands.extend(
            RecordingCommand::all_for_recording(recording_id)
                .filter(|cmd| !matches!(cmd.kind, RecordingCommandKind::PlaybackSpeed(_)))
                .map(|cmd| ListedCommand {
                    command: BoundCommand::Recording(cmd),
                    enabled: true,
                }),
        );
    }

    if let Some(origin) = &env.redap_server {
        commands.extend(
            RedapServerCommand::all_for_server(origin).map(|cmd| ListedCommand {
                enabled: !cmd.requires_editable_server() || env.has_editable_redap_server,
                command: BoundCommand::RedapServer(cmd),
            }),
        );
    }

    commands.extend(
        TableCommandKind::iter()
            .filter_map(|kind| kind.for_environment(env))
            .map(|cmd| ListedCommand {
                command: BoundCommand::Table(cmd),
                enabled: true,
            }),
    );

    commands
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn test_environments() -> Vec<CommandEnvironment> {
        let recording =
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app");
        let origin: re_uri::Origin = "rerun+http://example.com:51234".parse().unwrap();
        let empty = CommandEnvironment {
            recording: None,
            redap_server: None,
            has_editable_redap_server: false,
            table: None,
        };
        vec![
            empty.clone(),
            CommandEnvironment {
                recording: Some(recording.clone()),
                ..empty.clone()
            },
            CommandEnvironment {
                recording: Some(recording),
                redap_server: Some(origin.clone()),
                has_editable_redap_server: true,
                ..empty.clone()
            },
            CommandEnvironment {
                redap_server: Some(origin),
                has_editable_redap_server: false,
                ..empty
            },
        ]
    }

    #[test]
    fn ids_are_unique_and_round_trip() {
        let mut seen = HashSet::new();
        for kind in CommandKind::all() {
            let id = kind.id();
            assert!(seen.insert(id.clone()), "duplicate command id {id:?}");
            assert_eq!(CommandKind::from_id(&id), Some(kind), "{id:?}");
        }
    }

    #[test]
    fn palette_agrees_with_bind() {
        for env in test_environments() {
            let listed = palette_commands(&env);

            for entry in &listed {
                let kind = entry.command.kind();
                assert_eq!(
                    entry.enabled,
                    kind.bind(&env).is_some(),
                    "{} in {env:?}",
                    kind.id()
                );
                assert!(CommandKind::all().any(|k| k == kind), "{}", kind.id());
            }

            // Every command that can run is in the palette.
            for kind in CommandKind::all() {
                if kind.bind(&env).is_some() {
                    assert!(
                        listed
                            .iter()
                            .any(|entry| entry.enabled && entry.command.kind() == kind),
                        "{} runs in {env:?} but is not in the palette",
                        kind.id()
                    );
                }
            }
        }
    }

    /// Debug-only commands are left out, so the snapshot is the same in every build profile.
    #[test]
    fn command_ids() {
        let mut lines: Vec<String> = CommandKind::all()
            .filter(|kind| {
                cfg_select! {
                    debug_assertions => !kind.is_debug_only(),
                    _ => true,
                }
            })
            .map(|kind| format!("{}: {}", kind.id(), kind.tooltip()))
            .collect();
        lines.sort();
        let listing = lines.join("\n");
        insta::assert_snapshot!(listing);
    }
}
