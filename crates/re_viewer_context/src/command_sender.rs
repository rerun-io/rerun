use re_log_types::StoreId;
use re_ui::{UICommand, UICommandSender};

// ----------------------------------------------------------------------------

/// Commands used by internal system components
// TODO(jleibs): Is there a better crate for this?
pub enum SystemCommand {
    SetRecordingId(StoreId),
}

/// Interface for sending [`SystemCommand`] messages.
pub trait SystemCommandSender {
    fn send_system(&self, command: SystemCommand);
}

// ----------------------------------------------------------------------------

/// Generic Command
pub enum Command {
    SystemCommand(SystemCommand),
    UICommand(UICommand),
}

/// Sender that queues up the execution of a command.
pub struct CommandSender(std::sync::mpsc::Sender<Command>);

/// Receiver for the [`CommandSender`]
pub struct CommandReceiver(std::sync::mpsc::Receiver<Command>);

impl CommandReceiver {
    /// Receive a command to be executed if any is queued.
    pub fn recv(&self) -> Option<Command> {
        // The only way this can fail (other than being empty)
        // is if the sender has been dropped.
        self.0.try_recv().ok()
    }
}

/// Creates a new command channel.
pub fn command_channel() -> (CommandSender, CommandReceiver) {
    let (sender, receiver) = std::sync::mpsc::channel();
    (CommandSender(sender), CommandReceiver(receiver))
}

// ----------------------------------------------------------------------------

impl SystemCommandSender for CommandSender {
    /// Send a command to be executed.
    fn send_system(&self, command: SystemCommand) {
        // The only way this can fail is if the receiver has been dropped.
        self.0.send(Command::SystemCommand(command)).ok();
    }
}

impl UICommandSender for CommandSender {
    /// Send a command to be executed.
    fn send_ui(&self, command: UICommand) {
        // The only way this can fail is if the receiver has been dropped.
        self.0.send(Command::UICommand(command)).ok();
    }
}
