use std::panic::Location;

use re_chunk::EntityPath;
use re_chunk_store::external::re_chunk::Chunk;
use re_data_source::LogDataSource;
use re_log_channel::LogReceiver;
use re_log_types::StoreId;
use re_ui::{UICommand, UICommandSender};

use crate::time_control::TimeControlCommand;
use crate::{AuthContext, RecordingOrTable};

// ----------------------------------------------------------------------------

/// Commands used by internal system components
// TODO(jleibs): Is there a better crate for this?
#[derive(strum_macros::IntoStaticStr)]
pub enum SystemCommand {
    /// Make this the active application.
    ActivateApp(re_log_types::ApplicationId),

    /// Close this app and all its recordings.
    CloseApp(re_log_types::ApplicationId),

    /// Load data from a given data source.
    ///
    /// Will not load any new data if the source is already one of the active data sources.
    LoadDataSource(LogDataSource),

    /// Add a new receiver for log messages.
    AddReceiver(LogReceiver),

    /// Add a new server to the redap browser.
    AddRedapServer(re_uri::Origin),

    /// Open a modal to edit this redap server.
    EditRedapServerModal(EditRedapServerModalCommand),

    ChangeDisplayMode(crate::DisplayMode),

    /// Activates the setting display mode.
    OpenSettings,

    /// Activates the chunk store display mode.
    OpenChunkStoreBrowser,

    /// Sets the display mode to what it is at startup.
    ResetDisplayMode,

    /// Reset the `Viewer` to the default state
    ResetViewer,

    /// Clear the active blueprint.
    ///
    /// This may have two outcomes:
    /// - If a default blueprint is set, it will be used.
    /// - Otherwise, the heuristics will be enabled.
    ///
    /// To force using the heuristics, use [`Self::ClearActiveBlueprintAndEnableHeuristics`].
    ///
    /// UI note: because of the above ambiguity, controls for this command should only be enabled if
    /// a default blueprint is set or the behavior is explicitly explained.
    ClearActiveBlueprint,

    /// Clear the active blueprint and enable heuristics.
    ///
    /// The final outcome of this is to set the active blueprint to the heuristics. This command
    /// does not affect the default blueprint if any was set.
    ClearActiveBlueprintAndEnableHeuristics,

    /// Switch to this [`RecordingOrTable`].
    ActivateRecordingOrTable(RecordingOrTable),

    /// Close an [`RecordingOrTable`] and free its memory.
    CloseRecordingOrTable(RecordingOrTable),

    /// Close all stores and show the welcome screen again.
    CloseAllEntries,

    /// Add more data to a store (blueprint or recording).
    ///
    /// Edit recordings with case: we generally regard recordings as immutable.
    ///
    /// For blueprints,the [`StoreId`] should generally be the currently selected blueprint.
    ///
    /// Instead of using this directly, consider using `save_blueprint_archetype` or similar.
    AppendToStore(StoreId, Vec<Chunk>),

    UndoBlueprint {
        blueprint_id: StoreId,
    },
    RedoBlueprint {
        blueprint_id: StoreId,
    },

    /// Drop a specific entity from a store.
    ///
    /// Also drops all recursive children.
    ///
    /// The [`StoreId`] should generally be the currently selected blueprint
    /// but is tracked manually to ensure self-consistency if the blueprint
    /// is both modified and changed in the same frame.
    DropEntity(StoreId, EntityPath),

    /// Show a timeline of the blueprint data.
    #[cfg(debug_assertions)]
    EnableInspectBlueprintTimeline(bool),

    /// Navigate to time/entities/anchors/etc. that are set in a [`re_uri::Fragment`].
    SetUrlFragment {
        store_id: StoreId,
        fragment: re_uri::Fragment,
    },

    /// Copies the given url to the clipboard.
    ///
    /// On web this adds the viewer url as the base url.
    CopyViewerUrl(String),

    /// Set the item selection.
    SetSelection(SetSelection),

    TimeControlCommands {
        store_id: StoreId,
        time_commands: Vec<TimeControlCommand>,
    },

    /// Sets the focus to the given item.
    ///
    /// The focused item is cleared out every frame.
    /// Focusing is triggered either explicitly by ui-elements saying so
    /// or by double-clicking on a button representing an item.
    ///
    /// Unlike item selection, item focusing is not global state.
    /// It may however have stateful effects in certain views,
    /// e.g. the 3D view may follow the last focused item as it moves,
    /// or a frame may be highlighted for a few frames.
    ///
    /// Just like selection highlighting, the exact behavior of focusing is up to the receiving views.
    SetFocus(crate::Item),

    /// Show a notification to the user
    ShowNotification(re_ui::notifications::Notification),

    /// Add a task, run on a background thread, that saves something to disk.
    #[cfg(not(target_arch = "wasm32"))]
    FileSaver(Box<dyn FnOnce() -> anyhow::Result<std::path::PathBuf> + Send + 'static>),

    /// Notify about authentication changes.
    OnAuthChanged(Option<AuthContext>),

    /// Set authentication credentials from an external source.
    SetAuthCredentials {
        access_token: String,
        email: String,
    },

    /// Logout from rerun cloud
    Logout,
}

impl SystemCommand {
    pub fn clear_selection() -> Self {
        Self::set_selection(crate::ItemCollection::default())
    }

    pub fn set_selection(selection: impl Into<SetSelection>) -> Self {
        Self::SetSelection(selection.into())
    }
}

/// What triggered this item to be selected?
///
/// See [`crate::ViewerContext::handle_select_focus_sync`] why this is useful.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionSource {
    ListItemNavigation,
    Other,
}

pub struct SetSelection {
    pub selection: crate::ItemCollection,
    pub source: SelectionSource,
}

impl SetSelection {
    pub fn new(selection: impl Into<crate::ItemCollection>) -> Self {
        Self {
            selection: selection.into(),
            source: SelectionSource::Other,
        }
    }

    pub fn with_source(mut self, source: SelectionSource) -> Self {
        self.source = source;
        self
    }
}

impl<T: Into<crate::ItemCollection>> From<T> for SetSelection {
    fn from(selection: T) -> Self {
        Self {
            selection: selection.into(),
            source: SelectionSource::Other,
        }
    }
}

impl SystemCommand {
    /// A short debug name for this command.
    pub fn debug_name(&self) -> &'static str {
        self.into()
    }
}

impl std::fmt::Debug for SystemCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // not all variant contents can be made `Debug`, so we only output the variant name
        f.write_str(self.into())
    }
}

/// Interface for sending [`SystemCommand`] messages.
pub trait SystemCommandSender {
    fn send_system(&self, command: SystemCommand);
}

// ----------------------------------------------------------------------------

pub type StaticLocation = &'static Location<'static>;

/// Sender that queues up the execution of commands.
#[derive(Clone)]
pub struct CommandSender {
    system_sender: std::sync::mpsc::Sender<(StaticLocation, SystemCommand)>,
    ui_sender: std::sync::mpsc::Sender<UICommand>,
}

/// Receiver for the [`CommandSender`]
pub struct CommandReceiver {
    system_receiver: std::sync::mpsc::Receiver<(StaticLocation, SystemCommand)>,
    ui_receiver: std::sync::mpsc::Receiver<UICommand>,
}

impl CommandReceiver {
    /// Receive a [`SystemCommand`] to be executed if any is queued.
    ///
    /// Includes where it was sent from.
    pub fn recv_system(&self) -> Option<(StaticLocation, SystemCommand)> {
        // The only way this can fail (other than being empty)
        // is if the sender has been dropped.
        self.system_receiver.try_recv().ok()
    }

    /// Receive a [`UICommand`] to be executed if any is queued.
    pub fn recv_ui(&self) -> Option<UICommand> {
        // The only way this can fail (other than being empty)
        // is if the sender has been dropped.
        self.ui_receiver.try_recv().ok()
    }
}

/// Creates a new command channel.
pub fn command_channel() -> (CommandSender, CommandReceiver) {
    let (system_sender, system_receiver) = std::sync::mpsc::channel();
    let (ui_sender, ui_receiver) = std::sync::mpsc::channel();
    (
        CommandSender {
            system_sender,
            ui_sender,
        },
        CommandReceiver {
            system_receiver,
            ui_receiver,
        },
    )
}

// ----------------------------------------------------------------------------

impl SystemCommandSender for CommandSender {
    /// Send a command to be executed.
    #[track_caller]
    fn send_system(&self, command: SystemCommand) {
        // The only way this can fail is if the receiver has been dropped.
        self.system_sender.send((Location::caller(), command)).ok();
    }
}

impl UICommandSender for CommandSender {
    /// Send a command to be executed.
    fn send_ui(&self, command: UICommand) {
        // The only way this can fail is if the receiver has been dropped.
        self.ui_sender.send(command).ok();
    }
}

/// Command to open the edit redap server modal.
///
/// This exists as a separate struct to make it convenient to funnel it through the redap browser
/// command system.
pub struct EditRedapServerModalCommand {
    /// Which server should be edited?
    pub origin: re_uri::Origin,

    /// Provide a custom url to open when the server was successfully edited.
    ///
    /// By default, the server dataset table is opened.
    pub open_on_success: Option<String>,

    /// Optional custom title for the modal.
    pub title: Option<String>,
}

impl EditRedapServerModalCommand {
    pub fn new(origin: re_uri::Origin) -> Self {
        Self {
            origin,
            open_on_success: None,
            title: None,
        }
    }
}
