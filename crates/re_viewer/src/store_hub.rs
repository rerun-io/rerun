use re_data_store::StoreDb;
use re_log_types::{StoreId, StoreKind};

/// Stores many [`StoreDb`]s of recordings and blueprints.
#[derive(Default)]
pub struct StoreHub {
    // TODO(emilk): two separate maps per [`StoreKind`].
    store_dbs: ahash::HashMap<StoreId, StoreDb>,
}

impl StoreHub {
    /// Decode an rrd stream.
    /// It can theoretically contain multiple recordings, and blueprints.
    pub fn from_rrd(read: impl std::io::Read) -> anyhow::Result<Self> {
        re_tracing::profile_function!();

        let decoder = re_log_encoding::decoder::Decoder::new(read)?;

        let mut slf = Self::default();

        for msg in decoder {
            let msg = msg?;
            slf.store_db_entry(msg.store_id()).add(&msg)?;
        }
        Ok(slf)
    }

    /// Returns either a recording or blueprint [`StoreDb`].
    /// One is created if it doesn't already exist.
    pub fn store_db_entry(&mut self, id: &StoreId) -> &mut StoreDb {
        self.store_dbs
            .entry(id.clone())
            .or_insert_with(|| StoreDb::new(id.clone()))
    }

    /// All loaded [`StoreDb`], both recordings and blueprints, in arbitrary order.
    pub fn store_dbs(&self) -> impl Iterator<Item = &StoreDb> {
        self.store_dbs.values()
    }

    /// All loaded [`StoreDb`], both recordings and blueprints, in arbitrary order.
    pub fn store_dbs_mut(&mut self) -> impl Iterator<Item = &mut StoreDb> {
        self.store_dbs.values_mut()
    }

    pub fn append(&mut self, mut other: Self) {
        for (id, store_db) in other.store_dbs.drain() {
            self.store_dbs.insert(id, store_db);
        }
    }

    // --

    pub fn contains_recording(&self, id: &StoreId) -> bool {
        debug_assert_eq!(id.kind, StoreKind::Recording);
        self.store_dbs.contains_key(id)
    }

    pub fn recording(&self, id: &StoreId) -> Option<&StoreDb> {
        debug_assert_eq!(id.kind, StoreKind::Recording);
        self.store_dbs.get(id)
    }

    pub fn recording_mut(&mut self, id: &StoreId) -> Option<&mut StoreDb> {
        debug_assert_eq!(id.kind, StoreKind::Recording);
        self.store_dbs.get_mut(id)
    }

    /// Creates one if it doesn't exist.
    pub fn recording_entry(&mut self, id: &StoreId) -> &mut StoreDb {
        debug_assert_eq!(id.kind, StoreKind::Recording);
        self.store_dbs
            .entry(id.clone())
            .or_insert_with(|| StoreDb::new(id.clone()))
    }

    pub fn insert_recording(&mut self, store_db: StoreDb) {
        debug_assert_eq!(store_db.store_kind(), StoreKind::Recording);
        self.store_dbs.insert(store_db.store_id().clone(), store_db);
    }

    pub fn recordings(&self) -> impl Iterator<Item = &StoreDb> {
        self.store_dbs
            .values()
            .filter(|log| log.store_kind() == StoreKind::Recording)
    }

    pub fn blueprints(&self) -> impl Iterator<Item = &StoreDb> {
        self.store_dbs
            .values()
            .filter(|log| log.store_kind() == StoreKind::Blueprint)
    }

    // --

    pub fn contains_blueprint(&self, id: &StoreId) -> bool {
        debug_assert_eq!(id.kind, StoreKind::Blueprint);
        self.store_dbs.contains_key(id)
    }

    pub fn blueprint(&self, id: &StoreId) -> Option<&StoreDb> {
        debug_assert_eq!(id.kind, StoreKind::Blueprint);
        self.store_dbs.get(id)
    }

    pub fn blueprint_mut(&mut self, id: &StoreId) -> Option<&mut StoreDb> {
        debug_assert_eq!(id.kind, StoreKind::Blueprint);
        self.store_dbs.get_mut(id)
    }

    /// Creates one if it doesn't exist.
    pub fn blueprint_entry(&mut self, id: &StoreId) -> &mut StoreDb {
        debug_assert_eq!(id.kind, StoreKind::Blueprint);

        self.store_dbs.entry(id.clone()).or_insert_with(|| {
            // TODO(jleibs): If the blueprint doesn't exist this probably means we are
            // initializing a new default-blueprint for the application in question.
            // Make sure it's marked as a blueprint.

            let mut blueprint_db = StoreDb::new(id.clone());

            blueprint_db.add_begin_recording_msg(&re_log_types::SetStoreInfo {
                row_id: re_log_types::RowId::random(),
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
        self.store_dbs.retain(|_, store_db| !store_db.is_empty());
    }

    pub fn purge_fraction_of_ram(&mut self, fraction_to_purge: f32) {
        re_tracing::profile_function!();

        for store_db in self.store_dbs.values_mut() {
            store_db.purge_fraction_of_ram(fraction_to_purge);
        }
    }
}
