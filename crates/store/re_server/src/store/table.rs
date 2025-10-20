use std::sync::Arc;

use datafusion::catalog::TableProvider;

use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::{
    EntryKind,
    ext::{EntryDetails, ProviderDetails as _, SystemTable, TableEntry},
};

#[derive(Clone)]
pub struct Table {
    id: EntryId,
    name: String,
    provider: Arc<dyn TableProvider>,

    created_at: jiff::Timestamp,
    updated_at: jiff::Timestamp,

    system_table: Option<SystemTable>,
}

impl Table {
    pub fn new(
        id: EntryId,
        name: String,
        provider: Arc<dyn TableProvider>,
        created_at: Option<jiff::Timestamp>,
        system_table: Option<SystemTable>,
    ) -> Self {
        Self {
            id,
            name,
            provider,
            created_at: created_at.unwrap_or_else(jiff::Timestamp::now),
            updated_at: jiff::Timestamp::now(),
            system_table,
        }
    }

    pub fn id(&self) -> EntryId {
        self.id
    }

    pub fn created_at(&self) -> jiff::Timestamp {
        self.created_at
    }

    pub fn as_entry_details(&self) -> EntryDetails {
        EntryDetails {
            id: self.id,
            name: self.name.clone(),
            kind: EntryKind::Table,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    pub fn provider(&self) -> &Arc<dyn TableProvider> {
        &self.provider
    }

    pub fn as_table_entry(&self) -> TableEntry {
        let provider_details = match &self.system_table {
            Some(s) => s.try_as_any().expect("system_table should always be valid"),
            None => Default::default(),
        };

        TableEntry {
            details: EntryDetails {
                id: self.id,
                name: self.name.clone(),
                kind: EntryKind::Table,
                created_at: self.created_at,
                updated_at: self.updated_at,
            },

            provider_details,
        }
    }
}
