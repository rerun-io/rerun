//! Commands shared by the command palette, web menu, and native menu.

use egui::{KeyboardShortcut, os::OperatingSystem};
use re_ui::{
    BoundCommand, CommandEnvironment, CommandKind, RecordingCommandKind, RedapServerCommandKind,
    TableCommandKind, UICommand,
};
use re_viewer_context::open_url::ViewerOpenUrl;
use re_viewer_context::{ActiveStoreContext, CommandSender, StoreHub};

use crate::App;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuGroup {
    Application,
    File,
    Edit,
    View,
    Playback,
    Data,
    Help,
    Developer,
}

impl MenuGroup {
    pub const ALL: [Self; 8] = [
        Self::Application,
        Self::File,
        Self::Edit,
        Self::View,
        Self::Playback,
        Self::Data,
        Self::Help,
        Self::Developer,
    ];

    pub fn items(self) -> &'static [MenuItem] {
        use CommandKind::{Recording, RedapServer, Table, Ui};
        use MenuItem::{Command, Separator};
        use RecordingCommandKind as RecordingCommand;
        use RedapServerCommandKind as ServerCommand;

        match self {
            Self::Application => &[
                Command(Ui(UICommand::About)),
                Separator,
                Command(Ui(UICommand::OpenSettings)),
                #[cfg(not(target_arch = "wasm32"))]
                Separator,
                #[cfg(not(target_arch = "wasm32"))]
                Command(Ui(UICommand::Quit)),
            ],
            Self::File => &[
                Command(Ui(UICommand::OpenFile)),
                Command(Ui(UICommand::OpenUrlDialog)),
                Command(Ui(UICommand::ImportFileIntoCurrentRecording)),
                Separator,
                Command(Recording(RecordingCommand::Save)),
                Command(Recording(RecordingCommand::SaveTimeSelection)),
                Command(Recording(RecordingCommand::SaveBlueprint)),
                Separator,
                Command(Ui(UICommand::OpenShareDialog)),
                Separator,
                Command(Recording(RecordingCommand::Close)),
                Command(Ui(UICommand::CloseAllRecordings)),
            ],
            Self::Edit => &[
                Command(Recording(RecordingCommand::UndoBlueprintEdit)),
                Command(Recording(RecordingCommand::RedoBlueprintEdit)),
                Command(Table(TableCommandKind::UndoBlueprintEdit)),
                Command(Table(TableCommandKind::RedoBlueprintEdit)),
                Separator,
                Command(Ui(UICommand::CopyDirectLinkToClipboard)),
                Command(Ui(UICommand::CopyTimeSelectionLinkToClipboard)),
                Separator,
                Command(Ui(UICommand::CopyEntityHierarchyToClipboard)),
            ],
            Self::View => &[
                Command(Ui(UICommand::ToggleCommandPalette)),
                Separator,
                Command(Ui(UICommand::NavigateBackInHistory)),
                Command(Ui(UICommand::NavigateForwardInHistory)),
                Command(Ui(UICommand::SwitchToPreviousRecording)),
                Command(Ui(UICommand::SwitchToNextRecording)),
                Separator,
                Command(Ui(UICommand::ToggleTopPanel)),
                Command(Ui(UICommand::ToggleBlueprintPanel)),
                Command(Ui(UICommand::ExpandBlueprintPanel)),
                Command(Ui(UICommand::ToggleSelectionPanel)),
                Command(Ui(UICommand::ExpandSelectionPanel)),
                Command(Recording(RecordingCommand::ToggleTimePanel)),
                Command(Ui(UICommand::ToggleAgentPanel)),
                Separator,
                Command(Recording(RecordingCommand::OpenAddViewOrContainerDialog)),
                Command(Recording(RecordingCommand::ResetBlueprintToDefault)),
                Command(Recording(RecordingCommand::ResetBlueprintToHeuristic)),
                Command(Table(TableCommandKind::ResetBlueprintToDefault)),
                #[cfg(not(target_arch = "wasm32"))]
                Separator,
                #[cfg(not(target_arch = "wasm32"))]
                Command(Ui(UICommand::ZoomInUi)),
                #[cfg(not(target_arch = "wasm32"))]
                Command(Ui(UICommand::ZoomOutUi)),
                #[cfg(not(target_arch = "wasm32"))]
                Command(Ui(UICommand::ResetUiZoom)),
                Separator,
                Command(Ui(UICommand::ToggleFullscreen)),
                Separator,
                Command(Ui(UICommand::ResetViewer)),
            ],
            Self::Playback => &[
                Command(Recording(RecordingCommand::TogglePlayPause)),
                Separator,
                Command(Recording(RecordingCommand::SeekToPreviousEvent)),
                Command(Recording(RecordingCommand::SeekToNextEvent)),
                Separator,
                Command(Recording(RecordingCommand::SeekBackwardShort)),
                Command(Recording(RecordingCommand::SeekForwardShort)),
                Command(Recording(RecordingCommand::SeekBackwardLong)),
                Command(Recording(RecordingCommand::SeekForwardLong)),
                Separator,
                Command(Recording(RecordingCommand::SeekToStart)),
                Command(Recording(RecordingCommand::SeekToEndAndFollow)),
            ],
            Self::Data => &[
                Command(Ui(UICommand::OpenAddServerDialog)),
                Command(RedapServer(ServerCommand::OpenEditDialog)),
                Command(RedapServer(ServerCommand::CopyUrlToClipboard)),
                Command(RedapServer(ServerCommand::Remove)),
                Separator,
                Command(Table(TableCommandKind::Refresh)),
                Command(RedapServer(ServerCommand::Refresh)),
            ],
            Self::Help => &[
                Command(Ui(UICommand::OpenDocsWebsite)),
                Separator,
                Command(Ui(UICommand::OpenRerunWebsite)),
                Command(Ui(UICommand::OpenDiscordWebsite)),
            ],
            Self::Developer => &[
                Command(Ui(UICommand::TogglePanelStateOverrides)),
                Command(Ui(UICommand::ToggleDevPanel)),
                Command(Ui(UICommand::ToggleChunkStoreBrowser)),
                #[cfg(debug_assertions)]
                Command(Recording(RecordingCommand::ToggleBlueprintInspectionPanel)),
                #[cfg(debug_assertions)]
                Command(Ui(UICommand::ToggleEguiDebugPanel)),
                #[cfg(debug_assertions)]
                Command(Ui(UICommand::DebugTools)),
                #[cfg(debug_assertions)]
                Command(Ui(UICommand::PlayTestSound)),
                #[cfg(not(target_arch = "wasm32"))]
                Separator,
                #[cfg(not(target_arch = "wasm32"))]
                Command(Ui(UICommand::OpenProfiler)),
                #[cfg(not(target_arch = "wasm32"))]
                Command(Ui(UICommand::CaptureProfileTrace)),
                #[cfg(not(target_arch = "wasm32"))]
                Command(Ui(UICommand::CopyScreenshotToClipboard)),
                #[cfg(not(target_arch = "wasm32"))]
                Separator,
                #[cfg(not(target_arch = "wasm32"))]
                Command(Recording(RecordingCommand::PrintChunkStore)),
                #[cfg(not(target_arch = "wasm32"))]
                Command(Recording(RecordingCommand::PrintBlueprintStore)),
                #[cfg(not(target_arch = "wasm32"))]
                Command(Recording(RecordingCommand::PrintPrimaryCache)),
                #[cfg(debug_assertions)]
                Separator,
                #[cfg(debug_assertions)]
                Command(Ui(UICommand::ResetEguiMemory)),
                #[cfg(target_arch = "wasm32")]
                Separator,
                #[cfg(target_arch = "wasm32")]
                Command(Ui(UICommand::RestartWithWebGl)),
                #[cfg(target_arch = "wasm32")]
                Command(Ui(UICommand::RestartWithWebGpu)),
            ],
        }
    }

    pub fn text(self) -> &'static str {
        match self {
            Self::Application => "Rerun",
            Self::File => "File",
            Self::Edit => "Edit",
            Self::View => "View",
            Self::Playback => "Playback",
            Self::Data => "Data",
            Self::Help => "Help",
            Self::Developer => "Developer",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuItem {
    Command(CommandKind),
    Separator,
}

/// Every command in the catalog, in menu order.
pub fn catalog_commands() -> impl Iterator<Item = CommandKind> {
    MenuGroup::ALL
        .into_iter()
        .flat_map(MenuGroup::items)
        .filter_map(|item| match item {
            MenuItem::Command(kind) => Some(*kind),
            MenuItem::Separator => None,
        })
}

pub fn primary_shortcut(kind: CommandKind, os: OperatingSystem) -> Option<KeyboardShortcut> {
    kind.kb_shortcuts(os).first().copied()
}

pub struct CommandContext {
    pub environment: CommandEnvironment,
    pub has_recording_data: bool,
    pub has_time_selection: bool,
    pub file_save_in_progress: bool,
    links: LinkAvailability,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LinkAvailability {
    Unavailable,
    Direct,
    TimeSelection,
}

impl LinkAvailability {
    fn from_url(url: Option<ViewerOpenUrl>) -> Self {
        match url {
            Some(mut url) => {
                if url.fragment_mut().is_some() {
                    Self::TimeSelection
                } else {
                    Self::Direct
                }
            }
            None => Self::Unavailable,
        }
    }
}

impl CommandContext {
    pub fn is_visible(&self, kind: CommandKind) -> bool {
        match kind {
            CommandKind::Ui(UICommand::OpenShareDialog | UICommand::CopyDirectLinkToClipboard) => {
                self.links != LinkAvailability::Unavailable
            }
            CommandKind::Ui(UICommand::CopyTimeSelectionLinkToClipboard) => {
                self.links == LinkAvailability::TimeSelection
            }
            CommandKind::Ui(
                UICommand::ImportFileIntoCurrentRecording
                | UICommand::CopyEntityHierarchyToClipboard,
            ) => self.environment.recording.is_some(),
            _ => kind.bind(&self.environment).is_some(),
        }
    }

    /// Visible items without leading, trailing, or consecutive separators, retaining catalog indices
    /// for persistent native items.
    pub fn visible_items(&self, group: MenuGroup) -> impl Iterator<Item = (usize, MenuItem)> {
        let mut items = Vec::new();
        let mut pending_separator = None;
        for (index, &item) in group.items().iter().enumerate() {
            match item {
                MenuItem::Separator => {
                    if !items.is_empty() {
                        pending_separator = Some((index, item));
                    }
                }
                MenuItem::Command(kind) => {
                    if self.is_visible(kind) {
                        items.extend(pending_separator.take());
                        items.push((index, item));
                    }
                }
            }
        }
        items.into_iter()
    }

    pub fn resolve(&self, kind: CommandKind) -> Option<BoundCommand> {
        if !self.is_visible(kind) {
            return None;
        }
        match kind {
            CommandKind::Recording(
                RecordingCommandKind::Save | RecordingCommandKind::SaveTimeSelection,
            ) if self.file_save_in_progress || !self.has_recording_data => return None,
            CommandKind::Recording(RecordingCommandKind::SaveTimeSelection)
            | CommandKind::Ui(UICommand::CopyTimeSelectionLinkToClipboard)
                if !self.has_time_selection =>
            {
                return None;
            }
            _ => {}
        }
        kind.bind(&self.environment)
    }
}

impl App {
    pub fn uses_native_menu(&self) -> bool {
        cfg!(target_os = "macos")
            && !matches!(
                self.app_env(),
                crate::AppEnvironment::Test | crate::AppEnvironment::Custom(_)
            )
    }

    pub fn command_context(
        &self,
        store_hub: &StoreHub,
        store_context: Option<&ActiveStoreContext<'_>>,
    ) -> CommandContext {
        CommandContext {
            environment: self.command_environment(),
            has_recording_data: store_context
                .is_some_and(|ctx| ctx.recording.num_physical_chunks() > 0),
            has_time_selection: store_context.is_some_and(|ctx| ctx.loop_selection().is_some()),
            file_save_in_progress: self.background_tasks.is_file_save_in_progress(),
            links: LinkAvailability::from_url(
                ViewerOpenUrl::from_route(store_hub, self.state.navigation.current()).ok(),
            ),
        }
    }
}

pub fn dispatch(command: BoundCommand, sender: &CommandSender) {
    sender.send_command(command);
}

#[cfg(test)]
mod tests {
    use re_viewer_context::Route;

    use super::*;

    fn context() -> CommandContext {
        CommandContext {
            environment: CommandEnvironment {
                recording: None,
                redap_server: None,
                has_editable_redap_server: false,
                table: None,
                has_table_blueprint: false,
            },
            has_recording_data: false,
            has_time_selection: false,
            file_save_in_progress: false,
            links: LinkAvailability::Unavailable,
        }
    }

    #[test]
    fn save_commands_follow_recording_and_selection_availability() {
        let mut context = context();
        let save = CommandKind::Recording(RecordingCommandKind::Save);
        let save_selection = CommandKind::Recording(RecordingCommandKind::SaveTimeSelection);
        assert!(context.resolve(save).is_none());

        context.environment.recording = Some(re_log_types::StoreId::random(
            re_log_types::StoreKind::Recording,
            "test",
        ));
        assert!(context.resolve(save).is_none());
        context.has_recording_data = true;
        assert!(context.resolve(save).is_some());
        assert!(context.resolve(save_selection).is_none());
        context.has_time_selection = true;
        assert!(context.resolve(save_selection).is_some());
        context.file_save_in_progress = true;
        assert!(context.is_visible(save));
        assert!(context.resolve(save).is_none());
        assert!(context.resolve(save_selection).is_none());
    }

    #[test]
    fn link_commands_follow_route_capabilities() {
        let store_hub = StoreHub::test_hub();
        let mut context = context();
        context.has_time_selection = true;
        let share = CommandKind::Ui(UICommand::OpenShareDialog);
        let direct = CommandKind::Ui(UICommand::CopyDirectLinkToClipboard);
        let selection = CommandKind::Ui(UICommand::CopyTimeSelectionLinkToClipboard);

        for route in [
            Route::LocalTable(re_log_types::TableId::new("local".to_owned())),
            Route::LocalRecording {
                recording_id: re_log_types::StoreId::empty_recording(),
            },
            Route::Loading(Box::new(re_log_channel::LogSource::Sdk)),
        ] {
            context.links =
                LinkAvailability::from_url(ViewerOpenUrl::from_route(&store_hub, &route).ok());
            for kind in [share, direct, selection] {
                assert!(!context.is_visible(kind));
                assert!(context.resolve(kind).is_none());
            }
        }

        let origin = "rerun://localhost:51234".parse().expect("valid server URL");
        context.links = LinkAvailability::from_url(
            ViewerOpenUrl::from_route(&store_hub, &Route::RedapServer(origin)).ok(),
        );
        assert!(context.resolve(share).is_some());
        assert!(context.resolve(direct).is_some());
        assert!(!context.is_visible(selection));
        assert!(context.resolve(selection).is_none());

        let entry_id = re_log_types::EntryId::new();
        let url = format!("rerun://localhost:51234/dataset/{entry_id}?segment_id=test");
        context.links = LinkAvailability::from_url(Some(ViewerOpenUrl::RedapDataset(
            url.parse().expect("valid segment URL"),
        )));
        assert!(context.resolve(selection).is_some());
        context.has_time_selection = false;
        assert!(context.is_visible(selection));
        assert!(context.resolve(selection).is_none());
    }

    #[test]
    fn menu_items_hide_missing_targets_and_unused_separators() {
        let mut context = context();
        let connect = CommandKind::Ui(UICommand::OpenAddServerDialog);
        assert_eq!(
            context.visible_items(MenuGroup::Data).collect::<Vec<_>>(),
            vec![(0, MenuItem::Command(connect))]
        );
        assert!(context.visible_items(MenuGroup::Playback).next().is_none());

        context.environment.redap_server =
            Some("rerun://localhost:51234".parse().expect("valid server URL"));
        assert!(!context.is_visible(CommandKind::RedapServer(
            RedapServerCommandKind::OpenEditDialog
        )));
        assert!(!context.is_visible(CommandKind::RedapServer(RedapServerCommandKind::Remove)));
        let items = context.visible_items(MenuGroup::Data).collect::<Vec<_>>();
        assert_eq!(
            items,
            vec![
                (0, MenuItem::Command(connect)),
                (
                    2,
                    MenuItem::Command(CommandKind::RedapServer(
                        RedapServerCommandKind::CopyUrlToClipboard
                    ))
                ),
                (4, MenuItem::Separator),
                (
                    6,
                    MenuItem::Command(CommandKind::RedapServer(RedapServerCommandKind::Refresh))
                ),
            ]
        );

        context.environment.has_editable_redap_server = true;
        assert!(context.is_visible(CommandKind::RedapServer(
            RedapServerCommandKind::OpenEditDialog
        )));
        assert!(context.is_visible(CommandKind::RedapServer(RedapServerCommandKind::Remove)));
    }

    /// Undo and redo for a table only show up once the table has an active blueprint.
    /// A table without one, like the welcome page, has nothing to undo.
    #[test]
    fn table_undo_follows_active_blueprint() {
        let mut context = context();
        let undo = CommandKind::Table(TableCommandKind::UndoBlueprintEdit);
        let redo = CommandKind::Table(TableCommandKind::RedoBlueprintEdit);

        context.environment.table = Some(re_uri::TableReference::RedapServerEntries {
            origin: "rerun://localhost:51234".parse().expect("valid server URL"),
        });
        for kind in [undo, redo] {
            assert!(!context.is_visible(kind));
            assert!(context.resolve(kind).is_none());
        }

        context.environment.has_table_blueprint = true;
        for kind in [undo, redo] {
            assert!(context.resolve(kind).is_some());
        }
    }

    #[cfg(feature = "testing")]
    #[tokio::test]
    async fn removed_server_selection_is_not_a_command_target() {
        let mut harness = crate::viewer_test_utils::viewer_harness(&Default::default());
        let app = harness.state_mut();
        let origin = "rerun://localhost:51234".parse().expect("valid server URL");
        app.state
            .selection_state
            .set_selection(re_viewer_context::Item::RedapServer(origin));
        let context = app.command_context(app.store_hub.as_ref().expect("store hub"), None);
        assert!(context.environment.redap_server.is_none());
        assert!(!context.environment.has_editable_redap_server);
    }

    #[test]
    fn catalog_covers_each_command_once() {
        use std::collections::HashSet;
        use strum::IntoEnumIterator as _;

        let commands: Vec<_> = catalog_commands().collect();
        let unique: HashSet<_> = commands.iter().copied().collect();
        assert_eq!(commands.len(), unique.len());
        for command in UICommand::iter() {
            assert!(unique.contains(&CommandKind::Ui(command)));
        }
        for command in RecordingCommandKind::iter() {
            assert_eq!(
                unique.contains(&CommandKind::Recording(command)),
                !matches!(command, RecordingCommandKind::PlaybackSpeed(_))
            );
        }
        for command in RedapServerCommandKind::iter() {
            assert!(unique.contains(&CommandKind::RedapServer(command)));
        }
        for command in TableCommandKind::iter() {
            assert!(unique.contains(&CommandKind::Table(command)));
        }
    }
}
