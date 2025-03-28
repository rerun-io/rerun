use std::sync::mpsc::Sender;

use re_protos::common::v1alpha1::ext::EntryId;

use crate::servers::Command;

/// Context structure for the redap browser.
pub struct Context<'a> {
    /// Sender to queue new commands.
    pub command_sender: &'a Sender<Command>,

    /// Currently selected entry.
    pub selected_entry: &'a Option<EntryId>,
}
