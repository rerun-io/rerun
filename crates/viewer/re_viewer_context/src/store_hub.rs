use ahash::{HashMap, HashMapExt as _, HashSet};

use anyhow::Context as _;
use itertools::Itertools as _;

use nohash_hasher::IntMap;
use re_chunk_store::{
    ChunkStoreConfig, ChunkStoreGeneration, ChunkStoreStats, GarbageCollectionOptions,
    GarbageCollectionTarget,
};
use re_entity_db::EntityDb;
use re_log_types::{ApplicationId, ResolvedTimeRange, StoreId, StoreKind, TableId};
use re_query::CachesStats;
use re_types::components::Timestamp;

use crate::{
    BlueprintUndoState, Caches, StorageContext, StoreBundle, StoreContext, TableStore, TableStores,
};

#[derive(Clone)]
pub enum StoreHubEntry {
    Recording {
        store_id: StoreId,
        // TODO(grtlr): Add `applicationId` here.
    },
    Table {
        table_id: TableId,
    },
}

impl From<StoreId> for StoreHubEntry {
    fn from(store_id: StoreId) -> Self {
        Self::Recording { store_id }
    }
}

impl From<TableId> for StoreHubEntry {
    fn from(table_id: TableId) -> Self {
        Self::Table { table_id }
    }
}

impl StoreHubEntry {
    pub fn recording_ref(&self) -> Option<&StoreId> {
        match self {
            Self::Recording { store_id } => Some(store_id),
            Self::Table { .. } => None,
        }
    }

    pub fn table_ref(&self) -> Option<&TableId> {
        match self {
            Self::Table { table_id } => Some(table_id),
            Self::Recording { .. } => None,
        }
    }
}

/// Interface for accessing all blueprints and recordings
///
/// The [`StoreHub`] provides access to the [`EntityDb`] instances that are used
/// to store both blueprints and recordings.
///
/// Internally, the [`StoreHub`] tracks which [`ApplicationId`] and `recording
/// id` ([`StoreId`]) are currently active in the viewer. These can be configured
/// using [`StoreHub::set_active_recording_id`] and [`StoreHub::set_active_app`] respectively.
///
/// ## Blueprints
/// For each [`ApplicationId`], the [`StoreHub`] also keeps track of two blueprints:
/// * The active blueprint
/// * The default blueprint
///
/// Either on of these can be `None`.
///
/// The active blueprint is what the user would see and edit, if they were to select that app id.
/// If there is no active blueprint, the default will be cloned and made active.
///
/// The default blueprint is usually the blueprint set by the SDK.
/// This lets users reset the active blueprint to the one sent by the SDK.
#[derive(Default)]
pub struct StoreHub {
    /// How we load and save blueprints.
    persistence: BlueprintPersistence,

    active_entry: Option<StoreHubEntry>,
    active_application_id: Option<ApplicationId>,

    default_blueprint_by_app_id: HashMap<ApplicationId, StoreId>,
    active_blueprint_by_app_id: HashMap<ApplicationId, StoreId>,

    store_bundle: StoreBundle,
    table_stores: HashMap<TableId, TableStore>,

    /// These applications should enable the heuristics early next frame.
    should_enable_heuristics_by_app_id: HashSet<ApplicationId>,

    /// Things that need caching.
    caches_per_recording: HashMap<StoreId, Caches>,

    /// The [`ChunkStoreGeneration`] from when the [`EntityDb`] was last saved
    blueprint_last_save: HashMap<StoreId, ChunkStoreGeneration>,

    /// The [`ChunkStoreGeneration`] from when the [`EntityDb`] was last garbage collected
    blueprint_last_gc: HashMap<StoreId, ChunkStoreGeneration>,
}

/// Load a blueprint from persisted storage, e.g. disk.
/// Returns `Ok(None)` if no blueprint is found.
pub type BlueprintLoader =
    dyn Fn(&ApplicationId) -> anyhow::Result<Option<StoreBundle>> + Send + Sync;

/// Save a blueprint to persisted storage, e.g. disk.
pub type BlueprintSaver = dyn Fn(&ApplicationId, &EntityDb) -> anyhow::Result<()> + Send + Sync;

/// Validate a blueprint against the current blueprint schema requirements.
pub type BlueprintValidator = dyn Fn(&EntityDb) -> bool + Send + Sync;

/// How to save and load blueprints
#[derive(Default)]
pub struct BlueprintPersistence {
    pub loader: Option<Box<BlueprintLoader>>,
    pub saver: Option<Box<BlueprintSaver>>,
    pub validator: Option<Box<BlueprintValidator>>,
}

/// Convenient information used for `MemoryPanel`
#[derive(Default)]
pub struct StoreHubStats {
    pub blueprint_stats: ChunkStoreStats,
    pub blueprint_cached_stats: CachesStats,
    pub blueprint_config: ChunkStoreConfig,

    pub recording_stats2: ChunkStoreStats,
    pub recording_cached_stats: CachesStats,
    pub recording_config2: ChunkStoreConfig,
}

impl StoreHub {
    /// App ID used as a marker to display the welcome screen.
    pub fn welcome_screen_app_id() -> ApplicationId {
        "Welcome screen".into()
    }

    /// Blueprint ID used for the default welcome screen blueprint
    fn welcome_screen_blueprint_id() -> StoreId {
        StoreId::from_string(
            StoreKind::Blueprint,
            Self::welcome_screen_app_id().to_string(),
        )
    }

    /// Used only for tests
    pub fn test_hub() -> Self {
        Self::new(
            BlueprintPersistence {
                loader: None,
                saver: None,
                validator: None,
            },
            &|_| {},
        )
    }

    /// Create a new [`StoreHub`].
    ///
    /// The [`StoreHub`] will contain a single empty blueprint associated with the app ID returned
    /// by `[StoreHub::welcome_screen_app_id]`. It should be used as a marker to display the welcome
    /// screen.
    pub fn new(
        persistence: BlueprintPersistence,
        setup_welcome_screen_blueprint: &dyn Fn(&mut EntityDb),
    ) -> Self {
        re_tracing::profile_function!();
        let mut default_blueprint_by_app_id = HashMap::new();
        let mut store_bundle = StoreBundle::default();

        default_blueprint_by_app_id.insert(
            Self::welcome_screen_app_id(),
            Self::welcome_screen_blueprint_id(),
        );

        let welcome_screen_blueprint =
            store_bundle.blueprint_entry(&Self::welcome_screen_blueprint_id());
        (setup_welcome_screen_blueprint)(welcome_screen_blueprint);

        let table_stores = if std::env::var("RERUN_EXPERIMENTAL_TABLE").is_ok() {
            let table_id = TableId::new("test123".to_owned());
            std::iter::once((table_id, TableStore::dummy())).collect()
        } else {
            TableStores::default()
        };

        Self {
            persistence,
            active_entry: None,
            active_application_id: None,

            default_blueprint_by_app_id,
            active_blueprint_by_app_id: Default::default(),
            store_bundle,

            should_enable_heuristics_by_app_id: Default::default(),

            caches_per_recording: Default::default(),
            blueprint_last_save: Default::default(),
            blueprint_last_gc: Default::default(),

            table_stores,
        }
    }

    // ---------------------
    // Accessors

    /// All the loaded recordings and blueprints.
    #[inline]
    pub fn store_bundle(&self) -> &StoreBundle {
        &self.store_bundle
    }

    /// Get a read-only [`StorageContext`] and optionally a [`StoreContext`] (if available) from the [`StoreHub`].
    ///
    /// All of the returned references to blueprints and recordings will have a
    /// matching [`ApplicationId`].
    pub fn read_context(&mut self) -> (StorageContext<'_>, Option<StoreContext<'_>>) {
        static EMPTY_ENTITY_DB: once_cell::sync::Lazy<EntityDb> =
            once_cell::sync::Lazy::new(|| EntityDb::new(re_log_types::StoreId::empty_recording()));
        static EMPTY_CACHES: once_cell::sync::Lazy<Caches> =
            once_cell::sync::Lazy::new(Default::default);

        let store_context = 'ctx: {
            // If we have an app-id, then use it to look up the blueprint.
            let Some(app_id) = self.active_application_id.clone() else {
                break 'ctx None;
            };

            // Defensive coding: Check that default and active blueprints exists,
            // in case some of our book-keeping is broken.
            if let Some(blueprint_id) = self.default_blueprint_by_app_id.get(&app_id) {
                if !self.store_bundle.contains(blueprint_id) {
                    self.default_blueprint_by_app_id.remove(&app_id);
                }
            }
            if let Some(blueprint_id) = self.active_blueprint_by_app_id.get(&app_id) {
                if !self.store_bundle.contains(blueprint_id) {
                    self.active_blueprint_by_app_id.remove(&app_id);
                }
            }

            // If there's no active blueprint for this app, we must use the default blueprint, UNLESS
            // we're about to enable heuristics for this app.
            if !self.active_blueprint_by_app_id.contains_key(&app_id)
                && !self.should_enable_heuristics_by_app_id.contains(&app_id)
            {
                if let Some(blueprint_id) = self.default_blueprint_by_app_id.get(&app_id).cloned() {
                    self.set_cloned_blueprint_active_for_app(&app_id, &blueprint_id)
                        .unwrap_or_else(|err| {
                            re_log::warn!("Failed to make blueprint active: {err}");
                        });
                }
            }

            let active_blueprint = {
                // Get the id is of whatever blueprint is now active, falling back on the "app blueprint" if needed.
                let active_blueprint_id = self
                    .active_blueprint_by_app_id
                    .entry(app_id.clone())
                    .or_insert_with(|| {
                        StoreId::from_string(StoreKind::Blueprint, app_id.clone().0)
                    });

                // Get or create the blueprint:
                self.store_bundle.blueprint_entry(active_blueprint_id);
                let Some(active_blueprint) = self.store_bundle.get(active_blueprint_id) else {
                    break 'ctx None;
                };
                active_blueprint
            };

            let default_blueprint = self
                .default_blueprint_by_app_id
                .get(&app_id)
                .and_then(|id| self.store_bundle.get(id));

            // Calls `store_bundle.get()` internally and can therefore vary from the active entry.
            let recording = self.active_entry.as_ref().and_then(|id| match id {
                StoreHubEntry::Recording { store_id } => self.store_bundle.get(store_id),
                StoreHubEntry::Table { .. } => None,
            });

            if recording.is_none() && self.active_entry.is_none() {
                self.active_entry = None;
            }

            let should_enable_heuristics = self.should_enable_heuristics_by_app_id.remove(&app_id);
            let caches = self.active_caches();

            Some(StoreContext {
                app_id,
                blueprint: active_blueprint,
                default_blueprint,
                recording: recording.unwrap_or(&EMPTY_ENTITY_DB),
                caches: caches.unwrap_or(&EMPTY_CACHES),
                should_enable_heuristics,
            })
        };

        (
            StorageContext {
                hub: self,
                bundle: &self.store_bundle,
                tables: &self.table_stores,
            },
            store_context,
        )
    }

    /// Mutable access to a [`EntityDb`] by id
    pub fn entity_db_mut(&mut self, store_id: &StoreId) -> &mut EntityDb {
        self.store_bundle.entry(store_id)
    }

    // ---------------------
    // Add and remove stores

    /// Insert a new recording or blueprint into the [`StoreHub`].
    ///
    /// Note that the recording is not automatically made active. Use [`StoreHub::set_active_recording_id`]
    /// if needed.
    pub fn insert_entity_db(&mut self, entity_db: EntityDb) {
        self.store_bundle.insert(entity_db);
    }

    fn remove_store(&mut self, store_id: &StoreId) {
        _ = self.caches_per_recording.remove(store_id);
        let removed_store = self.store_bundle.remove(store_id);

        let Some(removed_store) = removed_store else {
            return;
        };

        match removed_store.store_kind() {
            StoreKind::Recording => {
                if let Some(app_id) = removed_store.app_id().cloned() {
                    let any_other_recordings_for_this_app = self
                        .store_bundle
                        .recordings()
                        .any(|rec| rec.app_id() == Some(&app_id));

                    if !any_other_recordings_for_this_app {
                        re_log::trace!("Removed last recording of {app_id}. Closing app.");
                        self.close_app(&app_id);
                    }
                }
            }
            StoreKind::Blueprint => {
                self.active_blueprint_by_app_id
                    .retain(|_, id| id != store_id);
                self.default_blueprint_by_app_id
                    .retain(|_, id| id != store_id);
            }
        }
    }

    pub fn remove(&mut self, entry: &StoreHubEntry) {
        match entry {
            StoreHubEntry::Recording { store_id } => {
                self.remove_store(store_id);
            }
            StoreHubEntry::Table { table_id } => {
                self.table_stores.remove(table_id);
            }
        }
    }

    pub fn retain_recordings(&mut self, mut should_retain: impl FnMut(&EntityDb) -> bool) {
        let stores_to_remove: Vec<StoreId> = self
            .store_bundle
            .entity_dbs()
            .filter_map(|store| {
                if should_retain(store) {
                    None
                } else {
                    Some(store.store_id().clone())
                }
            })
            .collect();
        for store in stores_to_remove {
            self.remove(&StoreHubEntry::Recording { store_id: store });
        }
    }

    /// Remove all open recordings and applications, and go to the welcome page.
    pub fn clear_recordings(&mut self) {
        // Keep only the welcome screen:
        let mut store_ids_retained = HashSet::default();
        self.store_bundle.retain(|db| {
            if db.app_id() == Some(&Self::welcome_screen_app_id()) {
                store_ids_retained.insert(db.store_id().clone());
                true
            } else {
                false
            }
        });
        self.caches_per_recording
            .retain(|store_id, _| store_ids_retained.contains(store_id));

        self.active_entry = None;
        self.active_application_id = Some(Self::welcome_screen_app_id());
    }

    // ---------------------
    // Active app

    /// Change the active [`ApplicationId`]
    #[allow(clippy::needless_pass_by_value)]
    pub fn set_active_app(&mut self, app_id: ApplicationId) {
        // If we don't know of a blueprint for this `ApplicationId` yet,
        // try to load one from the persisted store
        if !self.active_blueprint_by_app_id.contains_key(&app_id) {
            if let Err(err) = self.try_to_load_persisted_blueprint(&app_id) {
                re_log::warn!("Failed to load persisted blueprint: {err}");
            }
        }

        if self.active_application_id.as_ref() == Some(&app_id) {
            return;
        }

        self.active_application_id = Some(app_id.clone());
        self.active_entry = None;

        // Find any matching recording and activate it
        for rec in self
            .store_bundle
            .recordings()
            .sorted_by_key(|entity_db| entity_db.recording_property::<Timestamp>())
        {
            if rec.app_id() == Some(&app_id) {
                self.active_entry = Some(StoreHubEntry::Recording {
                    store_id: rec.store_id().clone(),
                });
                return;
            }
        }
    }

    /// Close this application and all its recordings.
    pub fn close_app(&mut self, app_id: &ApplicationId) {
        if let Err(err) = self.save_app_blueprints() {
            re_log::warn!("Failed to save blueprints: {err}");
        }

        let mut store_ids_removed = HashSet::default();
        self.store_bundle.retain(|db| {
            if db.app_id() == Some(app_id) {
                store_ids_removed.insert(db.store_id().clone());
                false
            } else {
                true
            }
        });
        self.caches_per_recording
            .retain(|store_id, _| !store_ids_removed.contains(store_id));

        if self.active_application_id.as_ref() == Some(app_id) {
            self.active_application_id = None;
            self.active_entry = None;
        }

        self.default_blueprint_by_app_id.remove(app_id);
        self.active_blueprint_by_app_id.remove(app_id);
    }

    #[inline]
    pub fn active_app(&self) -> Option<&ApplicationId> {
        self.active_application_id.as_ref()
    }

    // ---------------------
    // Active recording

    /// The recording id for the active recording.
    #[inline]
    pub fn active_recording_id(&self) -> Option<&StoreId> {
        self.active_entry.as_ref().and_then(|e| e.recording_ref())
    }

    /// Directly access the [`EntityDb`] for the active recording.
    #[inline]
    pub fn active_recording(&self) -> Option<&EntityDb> {
        match self.active_entry.as_ref() {
            Some(StoreHubEntry::Recording { store_id }) => self.store_bundle.get(store_id),
            _ => None,
        }
    }

    /// The table id for the active table.
    #[inline]
    pub fn active_table_id(&self) -> Option<&TableId> {
        self.active_entry.as_ref().and_then(|e| e.table_ref())
    }

    /// Directly access the [`Caches`] for the active recording.
    ///
    /// This returns `None` only if there is no active recording: the cache itself is always
    /// present if there's an active recording.
    #[inline]
    pub fn active_caches(&self) -> Option<&Caches> {
        self.active_recording_id().and_then(|store_id| {
            let caches = self.caches_per_recording.get(store_id);

            debug_assert!(
                caches.is_some(),
                "active recordings should always have associated caches",
            );

            caches
        })
    }

    /// Change the active/visible recording id.
    ///
    /// This will also change the application-id to match the newly active recording.
    pub fn set_active_recording_id(&mut self, recording_id: StoreId) {
        debug_assert_eq!(recording_id.kind, StoreKind::Recording);

        // If this recording corresponds to an app that we know about, then update the app-id.
        if let Some(app_id) = self
            .store_bundle
            .get(&recording_id)
            .as_ref()
            .and_then(|recording| recording.app_id())
            .cloned()
        {
            self.set_active_app(app_id);
        }

        self.active_entry = Some(StoreHubEntry::Recording {
            store_id: recording_id.clone(),
        });

        // Make sure the active recording has associated caches, always.
        _ = self.caches_per_recording.entry(recording_id).or_default();
    }

    /// Activate an [`Entry`]
    pub fn set_active_entry(&mut self, entry: StoreHubEntry) {
        match entry {
            StoreHubEntry::Recording { store_id } => self.set_activate_recording(store_id),
            StoreHubEntry::Table { table_id } => self.set_activate_table(table_id),
        }
    }

    /// Activate a recording by its [`StoreId`].
    pub fn set_activate_recording(&mut self, store_id: StoreId) {
        match store_id.kind {
            StoreKind::Recording => self.set_active_recording_id(store_id),
            StoreKind::Blueprint => {
                re_log::debug!("Tried to activate the blueprint {store_id} as a recording.");
            }
        }
    }

    /// Activate a recording by its [`TableId`].
    fn set_activate_table(&mut self, table_id: TableId) {
        self.active_entry = Some(StoreHubEntry::Table { table_id });
    }

    // ---------------------
    // Default blueprint

    pub fn default_blueprint_id_for_app(&self, app_id: &ApplicationId) -> Option<&StoreId> {
        self.default_blueprint_by_app_id.get(app_id)
    }

    pub fn default_blueprint_for_app(&self, app_id: &ApplicationId) -> Option<&EntityDb> {
        self.default_blueprint_id_for_app(app_id)
            .and_then(|id| self.store_bundle.get(id))
    }

    /// Change which blueprint is the default for a given [`ApplicationId`]
    #[inline]
    pub fn set_default_blueprint_for_app(
        &mut self,
        app_id: &ApplicationId,
        blueprint_id: &StoreId,
    ) -> anyhow::Result<()> {
        let blueprint = self
            .store_bundle
            .get(blueprint_id)
            .context("missing blueprint")?;

        // TODO(#6282): Improve this error message.
        if let Some(validator) = &self.persistence.validator {
            if !(validator)(blueprint) {
                anyhow::bail!("Blueprint failed validation");
            }
        }

        re_log::trace!("Switching default blueprint for '{app_id}' to '{blueprint_id}'");
        self.default_blueprint_by_app_id
            .insert(app_id.clone(), blueprint_id.clone());

        Ok(())
    }

    // ---------------------
    // Active blueprint

    /// What is the active blueprint for the active application?
    pub fn active_blueprint_id(&self) -> Option<&StoreId> {
        self.active_app()
            .and_then(|app_id| self.active_blueprint_id_for_app(app_id))
    }

    pub fn active_blueprint_id_for_app(&self, app_id: &ApplicationId) -> Option<&StoreId> {
        self.active_blueprint_by_app_id.get(app_id)
    }

    pub fn active_blueprint_for_app(&self, app_id: &ApplicationId) -> Option<&EntityDb> {
        self.active_blueprint_id_for_app(app_id)
            .and_then(|id| self.store_bundle.get(id))
    }

    /// Make blueprint active for a given [`ApplicationId`]
    ///
    /// We never activate a blueprint directly. Instead, we clone it and activate the clone.
    //TODO(jleibs): In the future this can probably be handled with snapshots instead.
    pub fn set_cloned_blueprint_active_for_app(
        &mut self,
        app_id: &ApplicationId,
        blueprint_id: &StoreId,
    ) -> anyhow::Result<()> {
        let new_id = StoreId::random(StoreKind::Blueprint);

        re_log::trace!(
            "Cloning '{blueprint_id}' as '{new_id}' the active blueprint for '{app_id}' to '{blueprint_id}'"
        );

        let blueprint = self
            .store_bundle
            .get(blueprint_id)
            .context("missing blueprint")?;

        // TODO(#6282): Improve this error message.
        if let Some(validator) = &self.persistence.validator {
            if !(validator)(blueprint) {
                anyhow::bail!("Blueprint failed validation");
            }
        }

        let new_blueprint = blueprint.clone_with_new_id(new_id.clone())?;

        self.store_bundle.insert(new_blueprint);

        self.active_blueprint_by_app_id
            .insert(app_id.clone(), new_id);

        Ok(())
    }

    /// Is the given blueprint id the active blueprint for any app id?
    pub fn is_active_blueprint(&self, blueprint_id: &StoreId) -> bool {
        self.active_blueprint_by_app_id
            .values()
            .any(|id| id == blueprint_id)
    }

    /// Clear the currently active blueprint
    pub fn clear_active_blueprint(&mut self) {
        if let Some(app_id) = &self.active_application_id {
            if let Some(blueprint_id) = self.active_blueprint_by_app_id.remove(app_id) {
                re_log::debug!("Clearing blueprint for {app_id}: {blueprint_id}");
                self.remove_store(&blueprint_id);
            }
        }
    }

    /// Clear the currently active blueprint and enable the heuristics to generate a new one.
    ///
    /// These keeps the default blueprint as is, so the user may reset to the default blueprint
    /// afterward.
    pub fn clear_active_blueprint_and_generate(&mut self) {
        self.clear_active_blueprint();

        // Simply clearing the default blueprint would trigger a reset to default. Instead, we must
        // actively set the blueprint to use the heuristics, so we store a flag so we do that early
        // next frame.
        if let Some(app_id) = self.active_app() {
            self.should_enable_heuristics_by_app_id
                .insert(app_id.clone());
        }
    }

    // ---------------------
    // Misc operations

    /// Cloned blueprints are the ones the user has edited,
    /// i.e. NOT sent from the SDK.
    pub fn clear_all_cloned_blueprints(&mut self) {
        self.retain_recordings(|db| match db.store_kind() {
            StoreKind::Recording => true,
            StoreKind::Blueprint => db.cloned_from().is_none(),
        });
    }

    /// Remove any empty [`EntityDb`]s from the hub
    pub fn purge_empty(&mut self) {
        self.retain_recordings(|entity_db| !entity_db.is_empty());
    }

    /// Call [`EntityDb::purge_fraction_of_ram`] on every recording
    //
    // NOTE: If you touch any of this, make sure to play around with our GC stress test scripts
    // available under `$WORKSPACE_ROOT/tests/python/gc_stress`.
    pub fn purge_fraction_of_ram(&mut self, fraction_to_purge: f32) {
        re_tracing::profile_function!();

        let Some(store_id) = self.store_bundle.find_oldest_modified_recording() else {
            return;
        };

        if let Some(caches) = self.caches_per_recording.get_mut(&store_id) {
            caches.purge_memory();
        }

        let store_bundle = &mut self.store_bundle;

        let Some(entity_db) = store_bundle.get_mut(&store_id) else {
            if cfg!(debug_assertions) {
                unreachable!();
            }
            return; // unreachable
        };

        let store_size_before = entity_db
            .storage_engine()
            .store()
            .stats()
            .total()
            .total_size_bytes;
        let store_events = entity_db.purge_fraction_of_ram(fraction_to_purge);
        let store_size_after = entity_db
            .storage_engine()
            .store()
            .stats()
            .total()
            .total_size_bytes;

        if let Some(caches) = self.caches_per_recording.get_mut(&store_id) {
            caches.on_store_events(&store_events);
        }

        // No point keeping an empty recording around.
        if entity_db.is_empty() {
            self.remove_store(&store_id);
            return;
        }

        // Running the GC didn't do anything.
        //
        // That's because all that's left in that store is protected rows: it's time to remove it
        // entirely, unless it's the last recording still standing, in which case we're better off
        // keeping some data around to show the user rather than a blank screen.
        //
        // If the user needs the memory for something else, they will get it back as soon as they
        // log new things anyhow.
        let num_recordings = store_bundle.recordings().count();
        if store_size_before == store_size_after && num_recordings > 1 {
            self.remove_store(&store_id);
        }

        // Either we've reached our target goal or we couldn't fetch memory stats, in which case
        // we still consider that we're done anyhow.

        // NOTE: It'd be tempting to loop through recordings here, as long as we haven't reached
        // our actual target goal.
        // We cannot do that though: there are other subsystems that need to release memory before
        // we can get an accurate reading of the current memory used and decide if we should go on.
    }

    /// Remove any recordings with a network source pointing at this `uri`.
    pub fn remove_recording_by_uri(&mut self, uri: &str) {
        self.retain_recordings(|db| {
            let Some(data_source) = &db.data_source else {
                // no data source, keep
                return true;
            };

            // retain only sources which:
            // - aren't network sources
            // - don't point at the given `uri`
            match data_source {
                re_smart_channel::SmartChannelSource::RrdHttpStream { url, .. } => url != uri,
                re_smart_channel::SmartChannelSource::RedapGrpcStream(endpoint) => {
                    endpoint.to_string() != uri
                }
                _ => true,
            }
        });
    }

    pub fn gc_blueprints(&mut self, undo_state: &HashMap<StoreId, BlueprintUndoState>) {
        re_tracing::profile_function!();

        for blueprint_id in self
            .active_blueprint_by_app_id
            .values()
            .chain(self.default_blueprint_by_app_id.values())
        {
            if let Some(blueprint) = self.store_bundle.get_mut(blueprint_id) {
                if self.blueprint_last_gc.get(blueprint_id) == Some(&blueprint.generation()) {
                    continue; // no change since last gc
                }

                let mut protected_time_ranges = IntMap::default();
                if let Some(undo) = undo_state.get(blueprint_id) {
                    if let Some(time) = undo.oldest_undo_point() {
                        // Save everything that we could want to undo to:
                        protected_time_ranges.insert(
                            crate::blueprint_timeline(),
                            ResolvedTimeRange::new(time, re_chunk::TimeInt::MAX),
                        );
                    }
                }

                let store_events = blueprint.gc(&GarbageCollectionOptions {
                    // TODO(#8249): configure blueprint GC to remove an entity if all that remains of it is a recursive clear
                    target: GarbageCollectionTarget::Everything,
                    protect_latest: 1, // keep the latest instance of everything, or we will forget things that haven't changed in a while
                    time_budget: re_entity_db::DEFAULT_GC_TIME_BUDGET,
                    protected_time_ranges,
                });
                if !store_events.is_empty() {
                    re_log::debug!("Garbage-collected blueprint store");
                    if let Some(caches) = self.caches_per_recording.get_mut(blueprint_id) {
                        caches.on_store_events(&store_events);
                    }
                }

                self.blueprint_last_gc
                    .insert(blueprint_id.clone(), blueprint.generation());
            }
        }
    }

    /// See `re_viewer_context::Cache::begin_frame`.
    pub fn begin_frame(&mut self, renderer_active_frame_idx: u64) {
        if let Some(store_id) = self.active_recording_id().cloned() {
            if let Some(caches) = self.caches_per_recording.get_mut(&store_id) {
                caches.begin_frame(renderer_active_frame_idx);
            }
        }
    }

    /// Persist any in-use blueprints to durable storage.
    pub fn save_app_blueprints(&mut self) -> anyhow::Result<()> {
        let Some(saver) = &self.persistence.saver else {
            return Ok(());
        };

        re_tracing::profile_function!();

        // Because we save blueprints based on their `ApplicationId`, we only
        // save the blueprints referenced by `blueprint_by_app_id`, even though
        // there may be other Blueprints in the Hub.

        for (app_id, blueprint_id) in &self.active_blueprint_by_app_id {
            if app_id == &Self::welcome_screen_app_id() {
                continue; // Don't save changes to the welcome screen
            }

            let Some(blueprint) = self.store_bundle.get_mut(blueprint_id) else {
                re_log::debug!("Failed to find blueprint {blueprint_id}.");
                continue;
            };
            if self.blueprint_last_save.get(blueprint_id) == Some(&blueprint.generation()) {
                continue; // no change since last save
            }

            (saver)(app_id, blueprint)?;
            self.blueprint_last_save
                .insert(blueprint_id.clone(), blueprint.generation());
        }

        Ok(())
    }

    /// Try to load the persisted blueprint for the given `ApplicationId`.
    /// Note: If no blueprint exists at the expected path, the result is still considered `Ok`.
    /// It is only an `Error` if a blueprint exists but fails to load.
    fn try_to_load_persisted_blueprint(&mut self, app_id: &ApplicationId) -> anyhow::Result<()> {
        re_tracing::profile_function!();

        let Some(loader) = &self.persistence.loader else {
            return Ok(());
        };

        if let Some(mut bundle) = (loader)(app_id)? {
            for store in bundle.drain_entity_dbs() {
                match store.store_kind() {
                    StoreKind::Recording => {
                        anyhow::bail!(
                            "Found a recording in a blueprint file: {:?}",
                            store.store_id()
                        );
                    }
                    StoreKind::Blueprint => {}
                }

                if store.app_id() != Some(app_id) {
                    if let Some(store_app_id) = store.app_id() {
                        anyhow::bail!("Found app_id {store_app_id}; expected {app_id}");
                    } else {
                        anyhow::bail!("Found store without an app_id");
                    }
                }

                // We found the blueprint we were looking for; make it active.
                // borrow-checker won't let us just call `self.set_blueprint_for_app_id`
                re_log::debug!(
                    "Activating new blueprint {} for {app_id}; loaded from disk",
                    store.store_id(),
                );
                self.active_blueprint_by_app_id
                    .insert(app_id.clone(), store.store_id().clone());
                self.blueprint_last_save
                    .insert(store.store_id().clone(), store.generation());
                self.store_bundle.insert(store);
            }
        }

        Ok(())
    }

    /// Populate a [`StoreHubStats`] based on the active app.
    //
    // TODO(jleibs): We probably want stats for all recordings, not just the active recording.
    pub fn stats(&self) -> StoreHubStats {
        re_tracing::profile_function!();

        // If we have an app-id, then use it to look up the blueprint.
        let blueprint = self
            .active_application_id
            .as_ref()
            .and_then(|app_id| self.active_blueprint_by_app_id.get(app_id))
            .and_then(|blueprint_id| self.store_bundle.get(blueprint_id))
            .map(|entity_db| entity_db.storage_engine());
        let blueprint = blueprint.as_ref();

        let blueprint_stats = blueprint
            .map(|engine| engine.store().stats())
            .unwrap_or_default();

        let blueprint_cached_stats = blueprint
            .map(|engine| engine.cache().stats())
            .unwrap_or_default();

        let blueprint_config = blueprint
            .map(|engine| engine.store().config().clone())
            .unwrap_or_default();

        let recording = self
            .active_recording()
            .map(|entity_db| entity_db.storage_engine());
        let recording = recording.as_ref();

        let recording_stats2 = recording
            .map(|engine| engine.store().stats())
            .unwrap_or_default();

        let recording_cached_stats = recording
            .map(|engine| engine.cache().stats())
            .unwrap_or_default();

        let recording_config2 = recording
            .map(|engine| engine.store().config().clone())
            .unwrap_or_default();

        StoreHubStats {
            blueprint_stats,
            blueprint_cached_stats,
            blueprint_config,

            recording_stats2,
            recording_cached_stats,
            recording_config2,
        }
    }
}
