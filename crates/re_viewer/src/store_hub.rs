use re_data_store::LogDb;
use re_log_types::{StoreId, StoreKind};

/// Stores many [`LogDb`]s of recordings and blueprints.
#[derive(Default)]
pub struct StoreHub {
    // TODO(emilk): two separate maps per [`StoreKind`].
    log_dbs: ahash::HashMap<StoreId, LogDb>,
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
            slf.log_db_entry(msg.store_id()).add(&msg)?;
        }
        Ok(slf)
    }

    /// Returns either a recording or blueprint [`LogDb`].
    /// One is created if it doesn't already exist.
    pub fn log_db_entry(&mut self, id: &StoreId) -> &mut LogDb {
        self.log_dbs
            .entry(id.clone())
            .or_insert_with(|| LogDb::new(id.clone()))
    }

    /// All loaded [`LogDb`], both recordings and blueprints, in arbitrary order.
    pub fn log_dbs(&self) -> impl Iterator<Item = &LogDb> {
        self.log_dbs.values()
    }

    /// All loaded [`LogDb`], both recordings and blueprints, in arbitrary order.
    pub fn log_dbs_mut(&mut self) -> impl Iterator<Item = &mut LogDb> {
        self.log_dbs.values_mut()
    }

    pub fn append(&mut self, mut other: Self) {
        for (id, log_db) in other.log_dbs.drain() {
            self.log_dbs.insert(id, log_db);
        }
    }

    // --

    pub fn contains_recording(&self, id: &StoreId) -> bool {
        debug_assert_eq!(id.kind, StoreKind::Recording);
        self.log_dbs.contains_key(id)
    }

    pub fn recording(&self, id: &StoreId) -> Option<&LogDb> {
        debug_assert_eq!(id.kind, StoreKind::Recording);
        self.log_dbs.get(id)
    }

    pub fn recording_mut(&mut self, id: &StoreId) -> Option<&mut LogDb> {
        debug_assert_eq!(id.kind, StoreKind::Recording);
        self.log_dbs.get_mut(id)
    }

    /// Creates one if it doesn't exist.
    pub fn recording_entry(&mut self, id: &StoreId) -> &mut LogDb {
        debug_assert_eq!(id.kind, StoreKind::Recording);
        self.log_dbs
            .entry(id.clone())
            .or_insert_with(|| LogDb::new(id.clone()))
    }

    pub fn insert_recording(&mut self, log_db: LogDb) {
        debug_assert_eq!(log_db.store_kind(), StoreKind::Recording);
        self.log_dbs.insert(log_db.store_id().clone(), log_db);
    }

    pub fn recordings(&self) -> impl Iterator<Item = &LogDb> {
        self.log_dbs
            .values()
            .filter(|log| log.store_kind() == StoreKind::Recording)
    }

    pub fn blueprints(&self) -> impl Iterator<Item = &LogDb> {
        self.log_dbs
            .values()
            .filter(|log| log.store_kind() == StoreKind::Blueprint)
    }

    // --

    pub fn contains_blueprint(&self, id: &StoreId) -> bool {
        debug_assert_eq!(id.kind, StoreKind::Blueprint);
        self.log_dbs.contains_key(id)
    }

    pub fn blueprint(&self, id: &StoreId) -> Option<&LogDb> {
        debug_assert_eq!(id.kind, StoreKind::Blueprint);
        self.log_dbs.get(id)
    }

    pub fn blueprint_mut(&mut self, id: &StoreId) -> Option<&mut LogDb> {
        debug_assert_eq!(id.kind, StoreKind::Blueprint);
        self.log_dbs.get_mut(id)
    }

    /// Creates one if it doesn't exist.
    pub fn blueprint_entry(&mut self, id: &StoreId) -> &mut LogDb {
        debug_assert_eq!(id.kind, StoreKind::Blueprint);

        self.log_dbs.entry(id.clone()).or_insert_with(|| {
            // TODO(jleibs): If the blueprint doesn't exist this probably means we are
            // initializing a new default-blueprint for the application in question.
            // Make sure it's marked as a blueprint.

            let mut blueprint_db = LogDb::new(id.clone());

            blueprint_db.add_begin_recording_msg(&re_log_types::SetRecordingInfo {
                row_id: re_log_types::RowId::random(),
                info: re_log_types::RecordingInfo {
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
        self.log_dbs.retain(|_, log_db| !log_db.is_empty());
    }

    pub fn purge_fraction_of_ram(&mut self, fraction_to_purge: f32) {
        re_tracing::profile_function!();

        for log_db in self.log_dbs.values_mut() {
            log_db.purge_fraction_of_ram(fraction_to_purge);
        }
    }
}
