use std::collections::BTreeMap;

use itertools::Itertools as _;

use re_log_types::{ApplicationId, EntryId, StoreId, StoreKind};

use crate::EntityDb;
use crate::entity_db::EntityDbClass;

#[derive(thiserror::Error, Debug)]
pub enum StoreLoadError {
    #[error(transparent)]
    Decode(#[from] re_log_encoding::decoder::DecodeError),

    #[error(transparent)]
    ChunkStore(#[from] crate::Error),
}

// ---

pub type DatasetRecordings<'a> = BTreeMap<EntryId, Vec<&'a EntityDb>>;

pub type RemoteRecordings<'a> = BTreeMap<re_uri::Origin, DatasetRecordings<'a>>;

pub type LocalRecordings<'a> = BTreeMap<ApplicationId, Vec<&'a EntityDb>>;

pub struct SortDatasetsResults<'a> {
    pub remote_recordings: RemoteRecordings<'a>,
    pub example_recordings: LocalRecordings<'a>,
    pub local_recordings: LocalRecordings<'a>,
}

// ---

/// Stores many [`EntityDb`]s of recordings and blueprints.
#[derive(Default)]
pub struct StoreBundle {
    recording_store: ahash::HashMap<StoreId, EntityDb>,
}

impl StoreBundle {
    /// Decode an rrd stream.
    /// It can theoretically contain multiple recordings, and blueprints.
    pub fn from_rrd(read: impl std::io::Read) -> Result<Self, StoreLoadError> {
        re_tracing::profile_function!();

        let decoder = re_log_encoding::decoder::Decoder::new(read)?;

        let mut slf = Self::default();

        for msg in decoder {
            let msg = msg?;
            slf.entry(msg.store_id()).add(&msg)?;
        }
        Ok(slf)
    }

    /// All loaded [`EntityDb`], both recordings and blueprints, in arbitrary order.
    pub fn entity_dbs(&self) -> impl Iterator<Item = &EntityDb> {
        self.recording_store.values()
    }

    /// All loaded [`EntityDb`], both recordings and blueprints, in arbitrary order.
    pub fn entity_dbs_mut(&mut self) -> impl Iterator<Item = &mut EntityDb> {
        self.recording_store.values_mut()
    }

    pub fn append(&mut self, mut other: Self) {
        for (id, entity_db) in other.recording_store.drain() {
            self.recording_store.insert(id, entity_db);
        }
    }

    pub fn remove(&mut self, id: &StoreId) -> Option<EntityDb> {
        self.recording_store.remove(id)
    }

    pub fn sort_recordings_by_class(&self) -> SortDatasetsResults<'_> {
        let mut remote_recordings: RemoteRecordings<'_> = BTreeMap::new();
        let mut local_recordings: LocalRecordings<'_> = BTreeMap::new();
        let mut example_recordings: LocalRecordings<'_> = BTreeMap::new();

        for entity_db in self.entity_dbs() {
            // We want to show all open applications, even if they have no recordings
            let Some(app_id) = entity_db.app_id().cloned() else {
                continue; // this only happens if we haven't even started loading it, or if something is really wrong with it.
            };

            match entity_db.store_class() {
                EntityDbClass::LocalRecording => {
                    local_recordings.entry(app_id).or_default().push(entity_db);
                }

                EntityDbClass::ExampleRecording => {
                    example_recordings
                        .entry(app_id)
                        .or_default()
                        .push(entity_db);
                }

                EntityDbClass::DatasetPartition(uri) => {
                    remote_recordings
                        .entry(uri.origin.clone())
                        .or_default()
                        .entry(EntryId::from(uri.dataset_id))
                        .or_default()
                        .push(entity_db);
                }

                EntityDbClass::Blueprint => continue,
            }
        }

        SortDatasetsResults {
            remote_recordings,
            example_recordings,
            local_recordings,
        }
    }

    // --

    pub fn contains(&self, id: &StoreId) -> bool {
        self.recording_store.contains_key(id)
    }

    pub fn get(&self, id: &StoreId) -> Option<&EntityDb> {
        self.recording_store.get(id)
    }

    pub fn get_mut(&mut self, id: &StoreId) -> Option<&mut EntityDb> {
        self.recording_store.get_mut(id)
    }

    /// Returns either a recording or blueprint [`EntityDb`].
    /// One is created if it doesn't already exist.
    pub fn entry(&mut self, id: &StoreId) -> &mut EntityDb {
        self.recording_store.entry(id.clone()).or_insert_with(|| {
            re_log::trace!("Creating new store: '{id}'");
            EntityDb::new(id.clone())
        })
    }

    /// Creates one if it doesn't exist.
    ///
    /// Like [`Self::entry`] but also sets `StoreInfo` to a default value.
    pub fn blueprint_entry(&mut self, id: &StoreId) -> &mut EntityDb {
        debug_assert_eq!(id.kind, StoreKind::Blueprint);

        self.recording_store.entry(id.clone()).or_insert_with(|| {
            // TODO(jleibs): If the blueprint doesn't exist this probably means we are
            // initializing a new default-blueprint for the application in question.
            // Make sure it's marked as a blueprint.

            let mut blueprint_db = EntityDb::new(id.clone());

            re_log::trace!("Creating a new blueprint '{id}'");

            blueprint_db.set_store_info(re_log_types::SetStoreInfo {
                row_id: *re_chunk::RowId::new(),
                info: re_log_types::StoreInfo {
                    application_id: id.as_str().into(),
                    store_id: id.clone(),
                    cloned_from: None,
                    store_source: re_log_types::StoreSource::Other("viewer".to_owned()),
                    store_version: Some(re_build_info::CrateVersion::LOCAL),
                },
            });

            blueprint_db
        })
    }

    pub fn insert(&mut self, entity_db: EntityDb) {
        self.recording_store
            .insert(entity_db.store_id().clone(), entity_db);
    }

    /// In no particular order.
    pub fn recordings(&self) -> impl Iterator<Item = &EntityDb> {
        self.recording_store
            .values()
            .filter(|log| log.store_kind() == StoreKind::Recording)
    }

    /// In no particular order.
    pub fn blueprints(&self) -> impl Iterator<Item = &EntityDb> {
        self.recording_store
            .values()
            .filter(|log| log.store_kind() == StoreKind::Blueprint)
    }

    // --

    pub fn retain(&mut self, mut f: impl FnMut(&EntityDb) -> bool) {
        self.recording_store.retain(|_, db| f(db));
    }

    /// In no particular order.
    pub fn drain_entity_dbs(&mut self) -> impl Iterator<Item = EntityDb> + '_ {
        self.recording_store.drain().map(|(_, store)| store)
    }

    // --

    /// Returns the [`StoreId`] of the oldest modified recording, according to [`EntityDb::last_modified_at`].
    pub fn find_oldest_modified_recording(&self) -> Option<StoreId> {
        let mut entity_dbs = self
            .recording_store
            .values()
            .filter(|db| db.store_kind() == StoreKind::Recording)
            .collect_vec();

        entity_dbs.sort_by_key(|db| db.last_modified_at());

        entity_dbs.first().map(|db| db.store_id())
    }
}
