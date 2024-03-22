use re_data_source::DataSource;
use re_entity_db::EntityDb;
use re_log_types::{DataRow, StoreId};
use re_ui::{UICommand, UICommandSender};

// ----------------------------------------------------------------------------

/// Commands used by internal system components
// TODO(jleibs): Is there a better crate for this?
pub enum SystemCommand {
    /// Load some data.
    LoadDataSource(DataSource),

    /// Load some log messages.
    LoadStoreDb(EntityDb),

    /// Reset the `Viewer` to the default state
    ResetViewer,

    /// Reset the `Blueprint` to the default state
    ResetBlueprint,

    /// If this is a recording, switch to it.
    /// If this is a blueprint, switch to the `AppId` of the blueprint,
    /// and make this blueprint the active blueprint for that `AppId`.
    ActivateStore(StoreId),

    /// Toggle the preference for showing the app blueprint or recording blueprint
    TogglePreferRecordingBlueprint,

    /// Close a recording or blueprint (free its memory).
    CloseStore(StoreId),

    /// Update the blueprint with additional data
    ///
    /// The [`StoreId`] should generally be the currently selected blueprint
    /// but is tracked manually to ensure self-consistency if the blueprint
    /// is both modified and changed in the same frame.
    UpdateBlueprint(StoreId, Vec<DataRow>),

    /// Show a timeline of the blueprint data.
    #[cfg(debug_assertions)]
    EnableInspectBlueprintTimeline(bool),

    /// Enable or disable the experimental dataframe space views.
    EnableExperimentalDataframeSpaceView(bool),

    /// Set the item selection.
    SetSelection {
        /// If set, use the recording config of this recording.
        /// Else, use the currently active recording.
        recording_id: Option<StoreId>,

        item: crate::Item,
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
}

/// Interface for sending [`SystemCommand`] messages.
pub trait SystemCommandSender {
    fn send_system(&self, command: SystemCommand);
}

// ----------------------------------------------------------------------------

/// Sender that queues up the execution of commands.
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
