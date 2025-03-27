use std::sync::mpsc::Sender;

use re_protos::common::v1alpha1::ext::EntryId;

use crate::servers::Command;

/// An handle for a [`DatasetOld`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatasetHandle {
    pub origin: re_uri::Origin,
    pub entry_id: EntryId,
}

/// Context structure for the redap browser.
pub struct Context<'a> {
    /// Sender to queue new commands.
    pub command_sender: &'a Sender<Command>,

    /// Currently selected collection.
    pub selected_collection: &'a Option<DatasetHandle>,
}
