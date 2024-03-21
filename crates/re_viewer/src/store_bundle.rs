use itertools::Itertools as _;

use re_entity_db::EntityDb;
use re_log_encoding::decoder::VersionPolicy;
use re_log_types::{StoreId, StoreKind};

#[derive(thiserror::Error, Debug)]
pub enum StoreLoadError {
    #[error(transparent)]
    Decode(#[from] re_log_encoding::decoder::DecodeError),

    #[error(transparent)]
    DataStore(#[from] re_entity_db::Error),
}

/// Stores many [`EntityDb`]s of recordings and blueprints.
#[derive(Default)]
pub struct StoreBundle {
    // TODO(emilk): two separate maps per [`StoreKind`].
    pub(crate) entity_dbs: ahash::HashMap<StoreId, EntityDb>, // TODO: private
}

impl StoreBundle {
    /// Decode an rrd stream.
    /// It can theoretically contain multiple recordings, and blueprints.
    pub fn from_rrd(
        version_policy: VersionPolicy,
        read: impl std::io::Read,
    ) -> Result<Self, StoreLoadError> {
        re_tracing::profile_function!();

        let decoder = re_log_encoding::decoder::Decoder::new(version_policy, read)?;

        let mut slf = Self::default();

        for msg in decoder {
            let msg = msg?;
            slf.entity_db_entry(msg.store_id()).add(&msg)?;
        }
        Ok(slf)
    }

    /// Returns either a recording or blueprint [`EntityDb`].
    /// One is created if it doesn't already exist.
    pub fn entity_db_entry(&mut self, id: &StoreId) -> &mut EntityDb {
        self.entity_dbs.entry(id.clone()).or_insert_with(|| {
            re_log::debug!("Creating new store: {id}");
            EntityDb::new(id.clone())
        })
    }

    /// All loaded [`EntityDb`], both recordings and blueprints, in arbitrary order.
    pub fn entity_dbs(&self) -> impl Iterator<Item = &EntityDb> {
        self.entity_dbs.values()
    }

    /// All loaded [`EntityDb`], both recordings and blueprints, in arbitrary order.
    pub fn entity_dbs_mut(&mut self) -> impl Iterator<Item = &mut EntityDb> {
        self.entity_dbs.values_mut()
    }

    pub fn append(&mut self, mut other: Self) {
        for (id, entity_db) in other.entity_dbs.drain() {
            self.entity_dbs.insert(id, entity_db);
        }
    }

    pub fn remove(&mut self, id: &StoreId) {
        self.entity_dbs.remove(id);
    }

    /// Returns the closest "neighbor" recording to the given id.
    ///
    /// The closest neighbor is the next recording when sorted by (app ID, time), if any, or the
    /// previous one otherwise. This is used to update the selected recording when the current one
    /// is deleted.
    pub fn find_closest_recording(&self, id: &StoreId) -> Option<&StoreId> {
        let mut recs = self.recordings().collect_vec();
        recs.sort_by_key(|entity_db| entity_db.sort_key());

        let cur_pos = recs.iter().position(|rec| rec.store_id() == id);

        if let Some(cur_pos) = cur_pos {
            if recs.len() > cur_pos + 1 {
                Some(recs[cur_pos + 1].store_id())
            } else if cur_pos > 0 {
                Some(recs[cur_pos - 1].store_id())
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns the [`StoreId`] of the oldest modified recording, according to [`EntityDb::last_modified_at`].
    pub fn find_oldest_modified_recording(&self) -> Option<&StoreId> {
        let mut entity_dbs = self
            .entity_dbs
            .values()
            .filter(|db| db.store_kind() == StoreKind::Recording)
            .collect_vec();

        entity_dbs.sort_by_key(|db| db.last_modified_at());

        entity_dbs.first().map(|db| db.store_id())
    }

    // --

    pub fn contains_store(&self, id: &StoreId) -> bool {
        self.entity_dbs.contains_key(id)
    }

    pub fn recording(&self, id: &StoreId) -> Option<&EntityDb> {
        debug_assert_eq!(id.kind, StoreKind::Recording);
        self.entity_dbs.get(id)
    }

    pub fn recording_mut(&mut self, id: &StoreId) -> Option<&mut EntityDb> {
        debug_assert_eq!(id.kind, StoreKind::Recording);
        self.entity_dbs.get_mut(id)
    }

    /// Creates one if it doesn't exist.
    pub fn recording_entry(&mut self, id: &StoreId) -> &mut EntityDb {
        debug_assert_eq!(id.kind, StoreKind::Recording);
        self.entity_dbs
            .entry(id.clone())
            .or_insert_with(|| EntityDb::new(id.clone()))
    }

    pub fn insert_entity_db(&mut self, entity_db: EntityDb) {
        self.entity_dbs
            .insert(entity_db.store_id().clone(), entity_db);
    }

    pub fn insert_blueprint(&mut self, entity_db: EntityDb) {
        debug_assert_eq!(entity_db.store_kind(), StoreKind::Blueprint);
        self.entity_dbs
            .insert(entity_db.store_id().clone(), entity_db);
    }

    pub fn recordings(&self) -> impl Iterator<Item = &EntityDb> {
        self.entity_dbs
            .values()
            .filter(|log| log.store_kind() == StoreKind::Recording)
    }

    pub fn blueprints(&self) -> impl Iterator<Item = &EntityDb> {
        self.entity_dbs
            .values()
            .filter(|log| log.store_kind() == StoreKind::Blueprint)
    }

    // --

    pub fn contains_blueprint(&self, id: &StoreId) -> bool {
        debug_assert_eq!(id.kind, StoreKind::Blueprint);
        self.entity_dbs.contains_key(id)
    }

    pub fn blueprint(&self, id: &StoreId) -> Option<&EntityDb> {
        debug_assert_eq!(id.kind, StoreKind::Blueprint);
        self.entity_dbs.get(id)
    }

    pub fn blueprint_mut(&mut self, id: &StoreId) -> Option<&mut EntityDb> {
        debug_assert_eq!(id.kind, StoreKind::Blueprint);
        self.entity_dbs.get_mut(id)
    }

    /// Creates one if it doesn't exist.
    pub fn blueprint_entry(&mut self, id: &StoreId) -> &mut EntityDb {
        debug_assert_eq!(id.kind, StoreKind::Blueprint);

        self.entity_dbs.entry(id.clone()).or_insert_with(|| {
            // TODO(jleibs): If the blueprint doesn't exist this probably means we are
            // initializing a new default-blueprint for the application in question.
            // Make sure it's marked as a blueprint.

            let mut blueprint_db = EntityDb::new(id.clone());

            re_log::debug!("Creating a new blueprint {id}");

            blueprint_db.set_store_info(re_log_types::SetStoreInfo {
                row_id: re_log_types::RowId::new(),
                info: re_log_types::StoreInfo {
                    application_id: id.as_str().into(),
                    store_id: id.clone(),
                    is_official_example: false,
                    started: re_log_types::Time::now(),
                    store_source: re_log_types::StoreSource::Other("viewer".to_owned()),
                    store_kind: StoreKind::Blueprint,
                },
            });

            blueprint_db
        })
    }

    // --

    pub fn purge_empty(&mut self) {
        self.entity_dbs.retain(|_, entity_db| !entity_db.is_empty());
    }

    pub fn drain_entity_dbs(&mut self) -> impl Iterator<Item = EntityDb> + '_ {
        self.entity_dbs.drain().map(|(_, store)| store)
    }
}
