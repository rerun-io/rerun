use std::sync::mpsc::Sender;

use re_protos::common::v1alpha1::ext::EntryId;

use crate::servers::{Command, Selection};

/// Context structure for the redap browser.
pub struct Context<'a> {
    /// Sender to queue new commands.
    pub command_sender: &'a Sender<Command>,

    /// Currently selected collection.
    pub selection: &'a Option<Selection>,
}

impl Context<'_> {
    pub fn is_entry_selected(&self, entry_id: EntryId) -> bool {
        matches!(
            self.selection,
            Some(Selection::Dataset(selected_entry_id))
            if selected_entry_id == &entry_id
        )
    }
}
