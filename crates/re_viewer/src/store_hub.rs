use ahash::HashMap;
use itertools::Itertools;
use re_arrow_store::{DataStoreConfig, DataStoreStats};
use re_data_store::StoreDb;
use re_log_types::{ApplicationId, StoreId, StoreKind};
use re_viewer_context::StoreContext;

/// Interface for accessing all blueprints and recordings
///
/// The [`StoreHub`] provides access to the [`StoreDb`] instances that are used
/// to store both blueprints and recordings.
///
/// Internally, the [`StoreHub`] tracks which [`ApplicationId`] and `recording
/// id` ([`StoreId`]) are currently selected in the viewer. These can be configured
/// using [`StoreHub::set_recording_id`] and [`StoreHub::set_app_id`] respectively.
#[derive(Default)]
pub struct StoreHub {
    selected_rec_id: Option<StoreId>,
    application_id: Option<ApplicationId>,
    blueprint_by_app_id: HashMap<ApplicationId, StoreId>,
    store_dbs: StoreBundle,
}

/// Convenient information used for `MemoryPanel`
#[derive(Default)]
pub struct StoreHubStats {
    pub blueprint_stats: DataStoreStats,
    pub blueprint_config: DataStoreConfig,
    pub recording_stats: DataStoreStats,
    pub recording_config: DataStoreConfig,
}

impl StoreHub {
    /// Add a [`StoreBundle`] to the [`StoreHub`]
    pub fn add_bundle(&mut self, bundle: StoreBundle) {
        self.store_dbs.append(bundle);
    }

    /// Get a read-only [`StoreContext`] from the [`StoreHub`] if one is available.
    ///
    /// All of the returned references to blueprints and recordings will have a
    /// matching [`ApplicationId`].
    pub fn read_context(&mut self) -> Option<StoreContext<'_>> {
        // If we have an app-id, then use it to look up the blueprint.
        let blueprint_id = self.application_id.as_ref().map(|app_id| {
            self.blueprint_by_app_id
                .entry(app_id.clone())
                .or_insert_with(|| StoreId::from_string(StoreKind::Blueprint, app_id.clone().0))
        });

        // As long as we have a blueprint-id, create the blueprint.
        blueprint_id
            .as_ref()
            .map(|id| self.store_dbs.blueprint_entry(id));

        // If we have a blueprint, we can return the `StoreContext`. In most
        // cases it should have already existed or been created above.
        blueprint_id
            .and_then(|id| self.store_dbs.blueprint(id))
            .map(|blueprint| {
                let recording = self
                    .selected_rec_id
                    .as_ref()
                    .and_then(|id| self.store_dbs.recording(id));

                // TODO(antoine): The below filter will limit our recording view to the current
                // `ApplicationId`. Leaving this commented out for now since that is a bigger
                // behavioral change we might want to plan/communicate around as it breaks things
                // like --split-recordings in the api_demo.
                StoreContext {
                    blueprint,
                    recording,
                    alternate_recordings: self
                        .store_dbs
                        .recordings()
                        //.filter(|rec| rec.app_id() == self.application_id.as_ref())
                        .collect_vec(),
                }
            })
    }

    /// Change the selected/visible recording id.
    /// This will also change the application-id to match the newly selected recording.
    pub fn set_recording_id(&mut self, recording_id: StoreId) {
        // If this recording corresponds to an app that we know about, then apdate the app-id.
        if let Some(app_id) = self
            .store_dbs
            .recording(&recording_id)
            .as_ref()
            .and_then(|recording| recording.app_id())
        {
            self.set_app_id(app_id.clone());
        }

        self.selected_rec_id = Some(recording_id);
    }

    /// Change the selected [`ApplicationId`]
    pub fn set_app_id(&mut self, app_id: ApplicationId) {
        self.application_id = Some(app_id);
    }

    /// Change which blueprint is active for a given [`ApplicationId`]
    pub fn set_blueprint_for_app_id(&mut self, blueprint_id: StoreId, app_id: ApplicationId) {
        self.blueprint_by_app_id.insert(app_id, blueprint_id);
    }

    /// Mutable access to a [`StoreDb`] by id
    pub fn store_db_mut(&mut self, store_id: &StoreId) -> &mut StoreDb {
        self.store_dbs.store_db_entry(store_id)
    }

    /// Remove any empty [`StoreDb`]s from the hub
    pub fn purge_empty(&mut self) {
        self.store_dbs.purge_empty();
    }

    /// Call [`StoreDb::purge_fraction_of_ram`] on every recording
    pub fn purge_fraction_of_ram(&mut self, fraction_to_purge: f32) {
        self.store_dbs.purge_fraction_of_ram(fraction_to_purge);
    }

    /// Directly access the [`StoreDb`] for the selected recording
    pub fn current_recording(&self) -> Option<&StoreDb> {
        self.selected_rec_id
            .as_ref()
            .and_then(|id| self.store_dbs.recording(id))
    }

    /// Check whether the [`StoreHub`] contains the referenced recording
    pub fn contains_recording(&self, id: &StoreId) -> bool {
        self.store_dbs.contains_recording(id)
    }

    /// Populate a [`StoreHubStats`] based on the selected app.
    // TODO(jleibs): We probably want stats for all recordings, not just
    // the currently selected recording.
    pub fn stats(&mut self) -> StoreHubStats {
        if let Some(ctx) = self.read_context() {
            let blueprint_stats = DataStoreStats::from_store(&ctx.blueprint.entity_db.data_store);

            let blueprint_config = ctx.blueprint.entity_db.data_store.config().clone();

            let recording_stats = ctx
                .recording
                .map(|store_db| DataStoreStats::from_store(&store_db.entity_db.data_store))
                .unwrap_or_default();

            let recording_config = ctx
                .recording
                .map(|store_db| store_db.entity_db.data_store.config().clone())
                .unwrap_or_default();

            StoreHubStats {
                blueprint_stats,
                blueprint_config,
                recording_stats,
                recording_config,
            }
        } else {
            StoreHubStats::default()
        }
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
