use std::sync::mpsc::Sender;

use crate::servers::Command;

/// Context structure for the redap browser.
pub struct Context<'a> {
    /// Sender to queue new commands.
    pub command_sender: &'a Sender<Command>,
}
