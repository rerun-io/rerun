use re_data_source::DataSource;
use re_log_types::{DataRow, StoreId};
use re_ui::{UICommand, UICommandSender};

// ----------------------------------------------------------------------------

/// Commands used by internal system components
// TODO(jleibs): Is there a better crate for this?
pub enum SystemCommand {
    /// Load some data.
    LoadDataSource(DataSource),

    /// Reset the `Viewer` to the default state
    ResetViewer,

    /// Change the active recording-id in the `StoreHub`
    SetRecordingId(StoreId),

    /// Close a recording
    CloseRecordingId(StoreId),

    /// Update the blueprint with additional data
    ///
    /// The [`StoreId`] should generally be the currently selected blueprint
    /// but is tracked manually to ensure self-consistency if the blueprint
    /// is both modified and changed in the same frame.
    UpdateBlueprint(StoreId, Vec<DataRow>),
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
