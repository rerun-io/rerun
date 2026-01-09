use itertools::Itertools as _;
use re_log_types::{StoreId, StoreKind};

use crate::EntityDb;

#[derive(thiserror::Error, Debug)]
pub enum StoreLoadError {
    #[error(transparent)]
    Decode(#[from] re_log_encoding::DecodeError),

    #[error(transparent)]
    ChunkStore(#[from] crate::Error),
}

// ---

/// Stores many [`EntityDb`]s of recordings and blueprints.
///
/// The stores are kept and iterated in insertion order to allow the UI to display them by default
/// in opening order.
#[derive(Default)]
pub struct StoreBundle {
    // `indexmap` is used to keep track of the insertion order.
    recording_store: indexmap::IndexMap<StoreId, EntityDb>,
}

impl StoreBundle {
    /// Decode an rrd stream.
    /// It can theoretically contain multiple recordings, and blueprints.
    pub fn from_rrd<R: std::io::Read>(
        reader: std::io::BufReader<R>,
    ) -> Result<Self, StoreLoadError> {
        re_tracing::profile_function!();

        let decoder = re_log_encoding::DecoderApp::decode_eager(reader)?;

        let mut slf = Self::default();

        for msg in decoder {
            let msg = msg?;
            slf.entry(msg.store_id()).add_log_msg(&msg)?;
        }
        Ok(slf)
    }

    /// All loaded [`EntityDb`], both recordings and blueprints, in insertion order.
    pub fn entity_dbs(&self) -> impl Iterator<Item = &EntityDb> {
        self.recording_store.values()
    }

    /// All loaded [`EntityDb`], both recordings and blueprints, in insertion order.
    pub fn entity_dbs_mut(&mut self) -> impl Iterator<Item = &mut EntityDb> {
        self.recording_store.values_mut()
    }

    pub fn remove(&mut self, id: &StoreId) -> Option<EntityDb> {
        self.recording_store.shift_remove(id)
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
            re_log::trace!("Creating new store: '{id:?}'");
            EntityDb::new(id.clone())
        })
    }

    /// Creates one if it doesn't exist.
    ///
    /// Like [`Self::entry`] but also sets `StoreInfo` to a default value.
    pub fn blueprint_entry(&mut self, id: &StoreId) -> &mut EntityDb {
        debug_assert!(id.is_blueprint());

        self.recording_store.entry(id.clone()).or_insert_with(|| {
            // TODO(jleibs): If the blueprint doesn't exist this probably means we are
            // initializing a new default-blueprint for the application in question.
            // Make sure it's marked as a blueprint.

            let mut blueprint_db = EntityDb::new(id.clone());

            re_log::trace!("Creating a new blueprint '{id:?}'");

            blueprint_db.set_store_info(re_log_types::SetStoreInfo {
                row_id: *re_chunk::RowId::new(),
                info: re_log_types::StoreInfo::new(
                    id.clone(),
                    re_log_types::StoreSource::Other("viewer".to_owned()),
                ),
            });

            blueprint_db
        })
    }

    pub fn insert(&mut self, entity_db: EntityDb) {
        self.recording_store
            .insert(entity_db.store_id().clone(), entity_db);
    }

    /// In insertion order.
    pub fn recordings(&self) -> impl Iterator<Item = &EntityDb> {
        self.recording_store
            .values()
            .filter(|log| log.store_kind() == StoreKind::Recording)
    }

    /// In insertion order.
    pub fn recordings_mut(&mut self) -> impl Iterator<Item = &mut EntityDb> {
        self.recording_store
            .values_mut()
            .filter(|log| log.store_kind() == StoreKind::Recording)
    }

    // --

    pub fn retain(&mut self, mut f: impl FnMut(&EntityDb) -> bool) {
        self.recording_store.retain(|_, db| f(db));
    }

    /// In insertion order.
    pub fn drain_entity_dbs(&mut self) -> impl Iterator<Item = EntityDb> + '_ {
        self.recording_store.drain(..).map(|(_, store)| store)
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

        entity_dbs.first().map(|db| db.store_id().clone())
    }
}
