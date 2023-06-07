use ahash::HashMap;
use re_arrow_store::{DataStoreConfig, DataStoreStats};
use re_data_store::StoreDb;
use re_log_types::{ApplicationId, StoreId, StoreKind};

use lazy_static::lazy_static;
use parking_lot::Mutex;

lazy_static! {
    static ref GLOBAL_HUB: Mutex<StoreHubImpl> = Mutex::new(StoreHubImpl::default());
}

#[derive(Default)]
pub struct StoreHubStats {
    pub blueprint_stats: DataStoreStats,
    pub blueprint_config: DataStoreConfig,
    pub recording_stats: DataStoreStats,
    pub recording_config: DataStoreConfig,
}

pub struct StoreView<'a> {
    pub blueprint: Option<&'a StoreDb>,
    pub recording: Option<&'a StoreDb>,
    pub bundle: &'a StoreBundle,
}

pub struct StoreHub;

impl StoreHub {
    pub fn add_bundle(bundle: StoreBundle) {
        GLOBAL_HUB.lock().add_bundle(bundle);
    }

    pub fn with_view<R>(f: impl FnOnce(StoreView<'_>) -> R) -> R {
        GLOBAL_HUB.lock().with_view(f)
    }

    /// The selected/visible recording id, if any.
    pub fn set_recording_id(recording_id: StoreId) {
        GLOBAL_HUB.lock().set_recording_id(recording_id);
    }

    pub fn set_app_id(app_id: ApplicationId) {
        GLOBAL_HUB.lock().set_app_id(app_id);
    }

    pub fn set_blueprint_for_app_id(blueprint_id: StoreId, app_id: ApplicationId) {
        GLOBAL_HUB
            .lock()
            .set_blueprint_for_app_id(blueprint_id, app_id);
    }

    pub fn store_stats() -> StoreHubStats {
        // TODO
        StoreHubStats::default()
        /*
                self.recording_db(|store_db| {
            store_db
                .map(|store_db| DataStoreStats::from_store(&store_db.entity_db.data_store))
                .unwrap_or_default()
        });
        */
    }

    pub fn purge_empty() {
        GLOBAL_HUB.lock().purge_empty();
    }

    pub fn purge_fraction_of_ram(fraction_to_purge: f32) {
        GLOBAL_HUB.lock().purge_fraction_of_ram(fraction_to_purge);
    }

    pub fn access_bundle<R>(f: impl FnOnce(&StoreBundle) -> R) -> R {
        GLOBAL_HUB.lock().access_bundle(f)
    }

    pub fn access_store_db_mut<R>(store_id: &StoreId, f: impl FnOnce(&mut StoreDb) -> R) -> R {
        GLOBAL_HUB.lock().access_store_db_mut(store_id, f)
    }
}

#[derive(Default)]
struct StoreHubImpl {
    selected_rec_id: Option<StoreId>,
    application_id: Option<ApplicationId>,
    blueprint_by_app_id: HashMap<ApplicationId, StoreId>,
    store_dbs: StoreBundle,
}

impl StoreHubImpl {
    fn add_bundle(&mut self, bundle: StoreBundle) {
        self.store_dbs.append(bundle);

        // TODO: mutate app_id / selected_rec_id
    }

    fn with_view<R>(&mut self, f: impl FnOnce(StoreView<'_>) -> R) -> R {
        // First maybe create blueprint if it's necessary.
        // TODO(jleibs): Can we hold onto this version here instead of
        // requerying below?
        if let Some(id) = self.application_id.as_ref().map(|app_id| {
            self.blueprint_by_app_id
                .entry(app_id.clone())
                .or_insert_with(|| StoreId::from_string(StoreKind::Blueprint, app_id.clone().0))
        }) {
            self.store_dbs.blueprint_entry(id);
        }

        let recording = self
            .selected_rec_id
            .as_ref()
            .and_then(|id| self.store_dbs.recording(id));

        let blueprint: Option<&StoreDb> = self
            .application_id
            .as_ref()
            .and_then(|app_id| self.blueprint_by_app_id.get(app_id))
            .and_then(|id| self.store_dbs.blueprint(id));

        let view = StoreView {
            blueprint,
            recording,
            bundle: &self.store_dbs,
        };

        f(view)
    }

    /// The selected/visible recording id, if any.
    fn set_recording_id(&mut self, recording_id: StoreId) {
        self.selected_rec_id = Some(recording_id);
    }

    fn set_app_id(&mut self, app_id: ApplicationId) {
        self.application_id = Some(app_id);
    }

    pub fn set_blueprint_for_app_id(&mut self, blueprint_id: StoreId, app_id: ApplicationId) {
        self.blueprint_by_app_id.insert(app_id, blueprint_id);
    }

    pub fn access_bundle<R>(&self, f: impl FnOnce(&StoreBundle) -> R) -> R {
        f(&self.store_dbs)
    }

    pub fn access_store_db_mut<R>(
        &mut self,
        store_id: &StoreId,
        f: impl FnOnce(&mut StoreDb) -> R,
    ) -> R {
        f(self.store_dbs.store_db_entry(store_id))
    }

    pub fn purge_empty(&mut self) {
        self.store_dbs.purge_empty();
    }

    pub fn purge_fraction_of_ram(&mut self, fraction_to_purge: f32) {
        self.store_dbs.purge_fraction_of_ram(fraction_to_purge);
    }
}

/// Stores many [`StoreDb`]s of recordings and blueprints.
#[derive(Default)]
pub struct StoreBundle {
    // TODO(emilk): two separate maps per [`StoreKind`].
    store_dbs: ahash::HashMap<StoreId, StoreDb>,
}

impl StoreBundle {
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
