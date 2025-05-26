use crate::StoreHubEntry;
use re_chunk::{EntityPath, Timeline};
use re_chunk_store::external::re_chunk::Chunk;
use re_data_source::DataSource;
use re_log_types::{ResolvedTimeRangeF, StoreId};
use re_ui::{UICommand, UICommandSender};
// ----------------------------------------------------------------------------

/// Commands used by internal system components
// TODO(jleibs): Is there a better crate for this?
#[derive(strum_macros::IntoStaticStr)]
pub enum SystemCommand {
    /// Make this the active application.
    ActivateApp(re_log_types::ApplicationId),

    /// Close this app and all its recordings.
    CloseApp(re_log_types::ApplicationId),

    /// Load some data.
    LoadDataSource(DataSource),

    /// Clear everything that came from this source, and close the source.
    ClearSourceAndItsStores(re_smart_channel::SmartChannelSource),

    /// Add a new receiver for log messages.
    AddReceiver(re_smart_channel::Receiver<re_log_types::LogMsg>),

    /// Add a new server to the redap browser.
    AddRedapServer(re_uri::Origin),

    ChangeDisplayMode(crate::DisplayMode),

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

    /// Switch to this [`StoreHubEntry`].
    ActivateEntry(StoreHubEntry),

    /// Close an [`StoreHubEntry`] and free its memory.
    CloseEntry(StoreHubEntry),

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

    /// Set the item selection.
    SetSelection(crate::Item),

    /// Set the active timeline and time for the given recording.
    SetActiveTime {
        rec_id: StoreId,
        timeline: re_chunk::Timeline,
        time: Option<re_log_types::TimeReal>,
    },

    /// Set the loop selection for the given timeline.
    ///
    /// This also sets the active timeline and activates the loop selection.
    SetLoopSelection {
        rec_id: StoreId,
        timeline: Timeline,
        time_range: ResolvedTimeRangeF,
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

    /// Add a task, run on a background thread, that saves something to disk.
    #[cfg(not(target_arch = "wasm32"))]
    FileSaver(Box<dyn FnOnce() -> anyhow::Result<std::path::PathBuf> + Send + 'static>),
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

/// Sender that queues up the execution of commands.
#[derive(Clone)]
pub struct CommandSender {
    system_sender: std::sync::mpsc::Sender<SystemCommand>,
    ui_sender: std::sync::mpsc::Sender<UICommand>,
}

/// Receiver for the [`CommandSender`]
pub struct CommandReceiver {
    system_receiver: std::sync::mpsc::Receiver<SystemCommand>,
    ui_receiver: std::sync::mpsc::Receiver<UICommand>,
}

impl CommandReceiver {
    /// Receive a [`SystemCommand`] to be executed if any is queued.
    pub fn recv_system(&self) -> Option<SystemCommand> {
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
    fn send_system(&self, command: SystemCommand) {
        // The only way this can fail is if the receiver has been dropped.
        self.system_sender.send(command).ok();
    }
}

impl UICommandSender for CommandSender {
    /// Send a command to be executed.
    fn send_ui(&self, command: UICommand) {
        // The only way this can fail is if the receiver has been dropped.
        self.ui_sender.send(command).ok();
    }
}
