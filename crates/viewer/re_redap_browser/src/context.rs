use std::sync::mpsc::Sender;

use re_protos::common::v1alpha1::ext::EntryId;

use crate::servers::{Command, ServerSelection};

/// Context structure for the redap browser.
pub struct Context<'a> {
    /// Sender to queue new commands.
    pub command_sender: &'a Sender<Command>,

    /// Currently selected collection.
    pub selected_collection: &'a Option<ServerSelection>,
}

impl Context<'_> {
    pub fn is_selected(&self, collection_id: CollectionId) -> bool {
        matches!(
            self.selected_collection,
            Some(ServerSelection::Dataset(selected_collection_id))
            if *selected_collection_id == collection_id
        )
    }
}
