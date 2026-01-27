use std::collections::BTreeMap;
use std::sync::{Arc, LazyLock};

use ahash::{HashMap, HashMapExt as _, HashSet};
use anyhow::Context as _;
use itertools::Itertools as _;
use nohash_hasher::IntMap;
use re_byte_size::{MemUsageNode, MemUsageTree, MemUsageTreeCapture};
use re_chunk_store::{
    ChunkStoreConfig, ChunkStoreGeneration, ChunkStoreStats, GarbageCollectionOptions,
    GarbageCollectionTarget,
};
use re_entity_db::{EntityDb, StoreBundle};
use re_log_channel::LogSource;
use re_log_types::{AbsoluteTimeRange, ApplicationId, StoreId, StoreKind, TableId};
use re_query::QueryCachesStats;
use re_sdk_types::archetypes;
use re_sdk_types::components::Timestamp;

use crate::{
    BlueprintUndoState, Caches, RecordingOrTable, StorageContext, StoreContext, TableStore,
    TableStores,
};

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
/// Either one of these can be `None`.
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

    active_recording_or_table: Option<RecordingOrTable>,
    active_application_id: Option<ApplicationId>,

    default_blueprint_by_app_id: HashMap<ApplicationId, StoreId>,
    active_blueprint_by_app_id: HashMap<ApplicationId, StoreId>,

    data_source_order: DataSourceOrder,
    store_bundle: StoreBundle,
    table_stores: HashMap<TableId, TableStore>,

    /// These applications should enable the heuristics early next frame.
    should_enable_heuristics_by_app_id: HashSet<ApplicationId>,

    /// Viewer caches (e.g. image decode cache).
    caches_per_recording: HashMap<StoreId, Caches>,

    /// The [`ChunkStoreGeneration`] from when the [`EntityDb`] was last saved
    blueprint_last_save: HashMap<StoreId, ChunkStoreGeneration>,

    /// The [`ChunkStoreGeneration`] from when the [`EntityDb`] was last garbage collected
    blueprint_last_gc: HashMap<StoreId, ChunkStoreGeneration>,
}

#[derive(Default)]
struct DataSourceOrder {
    next: u64,
    ordering: HashMap<LogSource, u64>,
}

impl DataSourceOrder {
    fn order_of(&self, source: &LogSource) -> u64 {
        self.ordering.get(source).copied().unwrap_or(u64::MAX)
    }

    fn add(&mut self, source: &LogSource) {
        if !self.ordering.contains_key(source) {
            self.next += 1;
            self.ordering.insert(source.clone(), self.next);
        }
    }
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

/// Convenient information used for `MemoryPanel`.
///
/// This is per [`StoreId`], which could be either a recording or a blueprint.
pub struct StoreStats {
    pub store_config: ChunkStoreConfig,
    pub store_stats: ChunkStoreStats,

    /// These are the query caches.
    pub query_cache_stats: QueryCachesStats,

    /// VRAM usage by different caches
    pub cache_vram_usage: MemUsageTree,
}

/// Convenient information used for `MemoryPanel`
#[derive(Default)]
pub struct StoreHubStats {
    pub store_stats: BTreeMap<StoreId, StoreStats>,

    /// Memory used by each [`TableStore`].
    pub table_stats: BTreeMap<TableId, u64>,
}

impl StoreHub {
    /// App ID used as a marker to display the welcome screen.
    pub fn welcome_screen_app_id() -> ApplicationId {
        "Welcome screen".into()
    }

    /// Blueprint ID used for the default welcome screen blueprint
    fn welcome_screen_blueprint_id() -> StoreId {
        StoreId::new(
            StoreKind::Blueprint,
            Self::welcome_screen_app_id(),
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

        Self {
            persistence,
            active_recording_or_table: None,

            // No active app is only ever transitional and we react by it to go back to the
            // welcome/start screen.
            // During application startup we may decide early to switch to a different screen,
            // so make sure we start out with the welcome screen app already set, so we won't
            // don't override this in the first frame.
            active_application_id: Some(Self::welcome_screen_app_id()),

            default_blueprint_by_app_id,
            active_blueprint_by_app_id: Default::default(),
            store_bundle,

            should_enable_heuristics_by_app_id: Default::default(),

            data_source_order: Default::default(),
            caches_per_recording: Default::default(),
            blueprint_last_save: Default::default(),
            blueprint_last_gc: Default::default(),

            table_stores: TableStores::default(),
        }
    }

    // ---------------------
    // Accessors

    /// All the loaded recordings and blueprints.
    #[inline]
    pub fn store_bundle(&self) -> &StoreBundle {
        &self.store_bundle
    }

    /// All the loaded recordings and blueprints.
    #[inline]
    pub fn store_bundle_mut(&mut self) -> &mut StoreBundle {
        &mut self.store_bundle
    }

    /// Get a read-only [`StorageContext`] and optionally a [`StoreContext`] (if available) from the [`StoreHub`].
    ///
    /// All of the returned references to blueprints and recordings will have a
    /// matching [`ApplicationId`].
    pub fn read_context(&mut self) -> (StorageContext<'_>, Option<StoreContext<'_>>) {
        static EMPTY_ENTITY_DB: LazyLock<EntityDb> =
            LazyLock::new(|| EntityDb::new(re_log_types::StoreId::empty_recording()));
        static EMPTY_CACHES: LazyLock<Caches> =
            LazyLock::new(|| Caches::new(re_log_types::StoreId::empty_recording()));

        let store_context = 'ctx: {
            // If we have an app-id, then use it to look up the blueprint.
            let Some(app_id) = self.active_application_id.clone() else {
                break 'ctx None;
            };

            // Defensive coding: Check that default and active blueprints exists,
            // in case some of our book-keeping is broken.
            if let Some(blueprint_id) = self.default_blueprint_by_app_id.get(&app_id)
                && !self.store_bundle.contains(blueprint_id)
            {
                self.default_blueprint_by_app_id.remove(&app_id);
            }
            if let Some(blueprint_id) = self.active_blueprint_by_app_id.get(&app_id)
                && !self.store_bundle.contains(blueprint_id)
            {
                self.active_blueprint_by_app_id.remove(&app_id);
            }

            // If there's no active blueprint for this app, we must use the default blueprint, UNLESS
            // we're about to enable heuristics for this app.
            if !self.active_blueprint_by_app_id.contains_key(&app_id)
                && !self.should_enable_heuristics_by_app_id.contains(&app_id)
                && let Some(blueprint_id) = self.default_blueprint_by_app_id.get(&app_id).cloned()
            {
                self.set_cloned_blueprint_active_for_app(&blueprint_id)
                    .unwrap_or_else(|err| {
                        re_log::warn!("Failed to make blueprint active: {err}");
                    });
            }

            let active_blueprint = {
                // Get the id is of whatever blueprint is now active, falling back on the "app blueprint" if needed.
                let active_blueprint_id = self
                    .active_blueprint_by_app_id
                    .entry(app_id.clone())
                    .or_insert_with(|| StoreId::default_blueprint(app_id.clone()));

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
            let recording = if let Some(id) = &self.active_recording_or_table {
                match id {
                    RecordingOrTable::Recording { store_id } => {
                        let recording = self.store_bundle.get(store_id);

                        // If we can't get the recording, clear it.
                        if recording.is_none() {
                            self.active_recording_or_table = None;
                        }

                        recording
                    }
                    RecordingOrTable::Table { .. } => None,
                }
            } else {
                None
            };

            let should_enable_heuristics = self.should_enable_heuristics_by_app_id.remove(&app_id);
            let caches = self.active_caches();

            Some(StoreContext {
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

    /// Read-only access to a [`EntityDb`] by id
    pub fn entity_db(&self, store_id: &StoreId) -> Option<&EntityDb> {
        self.store_bundle.get(store_id)
    }

    pub fn data_source_order(&self, data_source: &LogSource) -> u64 {
        self.data_source_order.order_of(data_source)
    }

    /// Called once a frame to make sure the data source order is correct.
    pub fn update_data_source_order(&mut self, loading_sources: &[Arc<LogSource>]) {
        let keep: HashSet<&LogSource> = loading_sources
            .iter()
            .map(|source| &**source)
            .chain(
                self.store_bundle
                    .recordings()
                    .filter_map(|db| db.data_source.as_ref()),
            )
            .collect();
        self.data_source_order
            .ordering
            .retain(|source, _| keep.contains(source));

        for source in self
            .store_bundle
            .recordings()
            .filter_map(|db| db.data_source.as_ref())
            .chain(loading_sources.iter().map(|s| &**s))
        {
            self.data_source_order.add(source);
        }
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

    /// Inserts a new table into the store (potentially overwriting an existing entry).
    pub fn insert_table_store(&mut self, id: TableId, store: TableStore) -> Option<TableStore> {
        self.table_stores.insert(id, store)
    }

    fn remove_store(&mut self, store_id: &StoreId) {
        _ = self.caches_per_recording.remove(store_id);
        let removed_store = self.store_bundle.remove(store_id);

        let Some(removed_store) = removed_store else {
            return;
        };

        match removed_store.store_kind() {
            StoreKind::Recording => {
                let app_id = removed_store.application_id();

                let any_other_recordings_for_this_app = self
                    .store_bundle
                    .recordings()
                    .any(|rec| rec.application_id() == app_id);

                if !any_other_recordings_for_this_app {
                    re_log::trace!("Removed last recording of {app_id}. Closing app.");
                    self.close_app(app_id);
                }
            }
            StoreKind::Blueprint => {
                self.active_blueprint_by_app_id
                    .retain(|_, id| id != store_id);
                self.default_blueprint_by_app_id
                    .retain(|_, id| id != store_id);
            }
        }

        // Drop the store itself on a separate thread,
        // so that recursing through it and freeing the memory doesn’t block the UI thread.
        #[allow(
            clippy::allow_attributes,
            clippy::disallowed_types,
            reason = "If this thread spawn fails due to running on Wasm (or for any other reason),
                      the error will be ignored and the store will be dropped on this thread."
        )]
        let (Ok(_) | Err(_)) = std::thread::Builder::new()
            .name("drop-removed-store".into())
            .spawn(|| {
                re_tracing::profile_scope!("drop store");
                drop(removed_store);
            });
    }

    pub fn remove(&mut self, entry: &RecordingOrTable) {
        match entry {
            RecordingOrTable::Recording { store_id } => {
                self.remove_store(store_id);
            }
            RecordingOrTable::Table { table_id } => {
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
            self.remove(&RecordingOrTable::Recording { store_id: store });
        }
    }

    /// Tries to find a recording store by its data source.
    ///
    /// Ignores any blueprint stores.
    ///
    /// If the data source is a grpc uri, it will ignore any fragments.
    /// If the data source is a http url, it will ignore the follow flag.
    pub fn find_recording_store_by_source(
        &self,
        data_source: &re_log_channel::LogSource,
    ) -> Option<&EntityDb> {
        self.store_bundle.entity_dbs().find(|db| {
            db.store_id().is_recording()
                && db
                    .data_source
                    .as_ref()
                    .is_some_and(|ds| ds.is_same_ignoring_uri_fragments(data_source))
        })
    }

    /// Remove all open recordings and applications, and go to the welcome page.
    pub fn clear_entries(&mut self) {
        // Keep only the welcome screen:
        let mut store_ids_retained = HashSet::default();
        self.store_bundle.retain(|db| {
            if db.application_id() == &Self::welcome_screen_app_id() {
                store_ids_retained.insert(db.store_id().clone());
                true
            } else {
                false
            }
        });
        self.caches_per_recording
            .retain(|store_id, _| store_ids_retained.contains(store_id));

        self.table_stores.clear();
        self.active_application_id = Some(Self::welcome_screen_app_id());
        self.active_recording_or_table = None;
    }

    // ---------------------
    // Active app

    /// Change the active [`ApplicationId`].
    ///
    /// Will ignore this request if the application id has no matching recording,
    /// unless no app id has been set yet at all so far.
    #[expect(clippy::needless_pass_by_value)]
    pub fn set_active_app(&mut self, app_id: ApplicationId) {
        // If we don't know of a blueprint for this `ApplicationId` yet,
        // try to load one from the persisted store
        if !self.active_blueprint_by_app_id.contains_key(&app_id)
            && let Err(err) = self.try_to_load_persisted_blueprint(&app_id)
        {
            re_log::warn!("Failed to load persisted blueprint: {err}");
        }

        if self.active_application_id.as_ref() == Some(&app_id) {
            return;
        }

        // If this is the welcome screen, or we didn't have any app id at all so far,
        // we set the active application_id even if we don't find a matching recording.
        // (otherwise we don't, because we don't want to leave towards a state without any recording if we don't have to)
        if Self::welcome_screen_app_id() == app_id || self.active_application_id.is_none() {
            self.active_application_id = Some(app_id.clone());
            self.active_recording_or_table = None;
        }

        // Find any matching recording and activate it
        for rec in self.store_bundle.recordings().sorted_by_key(|entity_db| {
            entity_db.recording_info_property::<Timestamp>(
                archetypes::RecordingInfo::descriptor_start_time().component,
            )
        }) {
            if rec.application_id() == &app_id {
                self.active_application_id = Some(app_id.clone());
                self.active_recording_or_table = Some(RecordingOrTable::Recording {
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
            if db.application_id() == app_id {
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
    pub fn active_store_id(&self) -> Option<&StoreId> {
        self.active_recording_or_table.as_ref()?.recording_ref()
    }

    /// Directly access the [`EntityDb`] for the active recording.
    #[inline]
    pub fn active_recording(&self) -> Option<&EntityDb> {
        match self.active_recording_or_table.as_ref() {
            Some(RecordingOrTable::Recording { store_id }) => self.store_bundle.get(store_id),
            _ => None,
        }
    }

    /// Directly access the [`EntityDb`] for the active recording.
    #[inline]
    pub fn active_recording_mut(&mut self) -> Option<&mut EntityDb> {
        match self.active_recording_or_table.as_mut() {
            Some(RecordingOrTable::Recording { store_id }) => self.store_bundle.get_mut(store_id),
            _ => None,
        }
    }

    /// Currently active recording or table, if any.
    pub fn active_recording_or_table(&self) -> Option<&RecordingOrTable> {
        self.active_recording_or_table.as_ref()
    }

    /// Directly access the [`Caches`] for the active recording.
    ///
    /// This returns `None` only if there is no active recording: the cache itself is always
    /// present if there's an active recording.
    #[inline]
    pub fn active_caches(&self) -> Option<&Caches> {
        let store_id = self.active_store_id()?;
        let caches = self.caches_per_recording.get(store_id);

        debug_assert!(
            caches.is_some(),
            "active recordings should always have associated caches",
        );

        caches
    }

    /// Get the [`Caches`] for a given store.
    ///
    /// Returns `None` if no caches exist for this store.
    pub fn caches_for_store(&self, store_id: &StoreId) -> Option<&Caches> {
        self.caches_per_recording.get(store_id)
    }

    /// Change the active/visible recording id.
    ///
    /// This will also change the application-id to match the newly active recording.
    pub fn set_active_recording_id(&mut self, recording_id: StoreId) {
        debug_assert!(recording_id.is_recording());

        // If this recording corresponds to an app that we know about, then update the app-id.
        if let Some(app_id) = self
            .store_bundle
            .get(&recording_id)
            .as_ref()
            .map(|recording| recording.application_id().clone())
        {
            self.set_active_app(app_id);
        }

        self.active_recording_or_table = Some(RecordingOrTable::Recording {
            store_id: recording_id.clone(),
        });

        // Make sure the active recording has associated caches, always.
        _ = self
            .caches_per_recording
            .entry(recording_id.clone())
            .or_insert_with(|| Caches::new(recording_id));
    }

    /// Activate a recording by its [`StoreId`].
    pub fn set_active_recording(&mut self, store_id: StoreId) {
        match store_id.kind() {
            StoreKind::Recording => self.set_active_recording_id(store_id),
            StoreKind::Blueprint => {
                re_log::debug!("Tried to activate the blueprint {store_id:?} as a recording.");
            }
        }
    }

    // ---------------------
    // Default blueprint

    pub fn default_blueprint_id_for_app(&self, app_id: &ApplicationId) -> Option<&StoreId> {
        self.default_blueprint_by_app_id.get(app_id)
    }

    pub fn default_blueprint_for_app(&self, app_id: &ApplicationId) -> Option<&EntityDb> {
        let id = self.default_blueprint_id_for_app(app_id)?;
        self.store_bundle.get(id)
    }

    /// Change which blueprint is the default for a given [`ApplicationId`]
    #[inline]
    pub fn set_default_blueprint_for_app(&mut self, blueprint_id: &StoreId) -> anyhow::Result<()> {
        let blueprint = self
            .store_bundle
            .get(blueprint_id)
            .context("missing blueprint")?;

        // TODO(#6282): Improve this error message.
        if let Some(validator) = &self.persistence.validator
            && !(validator)(blueprint)
        {
            anyhow::bail!("Blueprint failed validation");
        }

        re_log::trace!(
            "Switching default blueprint for '{:?}' to '{:?}'",
            blueprint_id.application_id(),
            blueprint_id
        );
        self.default_blueprint_by_app_id
            .insert(blueprint_id.application_id().clone(), blueprint_id.clone());

        Ok(())
    }

    // ---------------------
    // Active blueprint

    /// What is the active blueprint for the active application?
    pub fn active_blueprint_id(&self) -> Option<&StoreId> {
        let app_id = self.active_app()?;
        self.active_blueprint_id_for_app(app_id)
    }

    /// Active blueprint for currently active application.
    pub fn active_blueprint(&self) -> Option<&EntityDb> {
        let id = self.active_blueprint_id()?;
        self.store_bundle.get(id)
    }

    pub fn active_blueprint_id_for_app(&self, app_id: &ApplicationId) -> Option<&StoreId> {
        self.active_blueprint_by_app_id.get(app_id)
    }

    pub fn active_blueprint_for_app(&self, app_id: &ApplicationId) -> Option<&EntityDb> {
        let id = self.active_blueprint_id_for_app(app_id)?;
        self.store_bundle.get(id)
    }

    /// Make blueprint active for a given [`ApplicationId`]
    ///
    /// We never activate a blueprint directly. Instead, we clone it and activate the clone.
    //TODO(jleibs): In the future this can probably be handled with snapshots instead.
    pub fn set_cloned_blueprint_active_for_app(
        &mut self,
        blueprint_id: &StoreId,
    ) -> anyhow::Result<()> {
        let app_id = blueprint_id.application_id().clone();
        let new_id = StoreId::random(StoreKind::Blueprint, app_id.clone());

        re_log::trace!(
            "Cloning '{blueprint_id:?}' as '{new_id:?}' the active blueprint for '{app_id}'"
        );

        let blueprint = self
            .store_bundle
            .get(blueprint_id)
            .context("missing blueprint")?;

        // TODO(#6282): Improve this error message.
        if let Some(validator) = &self.persistence.validator
            && !(validator)(blueprint)
        {
            anyhow::bail!("Blueprint failed validation");
        }

        let new_blueprint = blueprint.clone_with_new_id(new_id.clone())?;

        self.store_bundle.insert(new_blueprint);

        self.active_blueprint_by_app_id.insert(app_id, new_id);

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
        if let Some(app_id) = &self.active_application_id
            && let Some(blueprint_id) = self.active_blueprint_by_app_id.remove(app_id)
        {
            re_log::debug!("Clearing blueprint for {app_id}: {blueprint_id:?}");
            self.remove_store(&blueprint_id);
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
    ///
    /// `time_cursor_for` can be used for more intelligent targeting of what is to be removed.
    ///
    /// Returns the number of bytes freed.
    //
    // NOTE: If you touch any of this, make sure to play around with our GC stress test scripts
    // available under `$WORKSPACE_ROOT/tests/python/gc_stress`.
    pub fn purge_fraction_of_ram(
        &mut self,
        fraction_to_purge: f32,
        time_cursor_for: &dyn Fn(
            &StoreId,
        )
            -> Option<(re_log_types::Timeline, re_log_types::TimeInt)>,
    ) -> u64 {
        re_tracing::profile_function!();

        #[expect(clippy::iter_over_hash_type)]
        for cache in self.caches_per_recording.values_mut() {
            cache.purge_memory();
        }

        let active_store_id = self.active_store_id().cloned();
        let background_recording_ids = self
            .store_bundle
            .recordings()
            .map(|db| db.store_id().clone())
            .filter(|store_id| Some(store_id) != active_store_id.as_ref())
            .collect::<Vec<_>>();

        let mut num_bytes_freed = 0;

        for store_id in background_recording_ids {
            let time_cursor = time_cursor_for(&store_id);
            num_bytes_freed +=
                self.purge_fraction_of_ram_for(fraction_to_purge, &store_id, time_cursor);
        }

        if num_bytes_freed == 0
            && let Some(active_store_id) = active_store_id
        {
            // We didn't free any memory from the background recordings,
            // so try the active one:
            let time_cursor = time_cursor_for(&active_store_id);
            num_bytes_freed +=
                self.purge_fraction_of_ram_for(fraction_to_purge, &active_store_id, time_cursor);
        }

        num_bytes_freed
    }

    /// Call [`EntityDb::purge_fraction_of_ram`] on every recording
    ///
    /// `time_cursor_for` can be used for more intelligent targeting of what is to be removed.
    //
    // NOTE: If you touch any of this, make sure to play around with our GC stress test scripts
    // available under `$WORKSPACE_ROOT/tests/python/gc_stress`.
    fn purge_fraction_of_ram_for(
        &mut self,
        fraction_to_purge: f32,
        store_id: &StoreId,
        time_cursor: Option<(re_log_types::Timeline, re_log_types::TimeInt)>,
    ) -> u64 {
        re_tracing::profile_function!();
        let is_active_recording = Some(store_id) == self.active_store_id();

        let store_bundle = &mut self.store_bundle;

        let is_last_recording = store_bundle.recordings().count() == 1;

        let Some(entity_db) = store_bundle.get_mut(store_id) else {
            if cfg!(debug_assertions) {
                unreachable!();
            }
            return 0; // unreachable
        };

        let store_size_before = entity_db
            .storage_engine()
            .store()
            .stats()
            .total()
            .total_size_bytes;
        let store_events = entity_db.purge_fraction_of_ram(fraction_to_purge, time_cursor);
        let store_size_after = entity_db
            .storage_engine()
            .store()
            .stats()
            .total()
            .total_size_bytes;

        if let Some(caches) = self.caches_per_recording.get_mut(store_id) {
            caches.on_store_events(&store_events, entity_db);
        }

        let num_bytes_freed = store_size_before.saturating_sub(store_size_after);

        // No point keeping an empty recording around… but don't close the active one.
        if entity_db.is_empty() && !is_active_recording {
            self.remove_store(store_id);
            return store_size_before;
        }

        if num_bytes_freed == 0 {
            // Running the GC didn't do anything.
            //
            // That's because all that's left in that store is protected rows: it's time to remove it
            // entirely, unless it's the last recording still standing, in which case we're better off
            // keeping some data around to show the user rather than a blank screen.
            //
            // If the user needs the memory for something else, they will get it back as soon as they
            // log new things anyhow.
            if !is_last_recording {
                self.remove_store(store_id);
                return store_size_before;
            }
        }

        num_bytes_freed
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
                re_log_channel::LogSource::RrdHttpStream { url, .. } => url != uri,

                re_log_channel::LogSource::RedapGrpcStream { uri: redap_uri, .. } => {
                    redap_uri.to_string() != uri
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
                if let Some(undo) = undo_state.get(blueprint_id)
                    && let Some(time) = undo.oldest_undo_point()
                {
                    // Save everything that we could want to undo to:
                    protected_time_ranges.insert(
                        crate::blueprint_timeline(),
                        AbsoluteTimeRange::new(time, re_chunk::TimeInt::MAX),
                    );
                }

                let store_events = blueprint.gc(&GarbageCollectionOptions {
                    // TODO(#8249): configure blueprint GC to remove an entity if all that remains of it is a recursive clear
                    target: GarbageCollectionTarget::Everything,
                    protect_latest: 1, // keep the latest instance of everything, or we will forget things that haven't changed in a while
                    time_budget: re_entity_db::DEFAULT_GC_TIME_BUDGET,
                    protected_time_ranges,
                    furthest_from: None,
                    // There is no point in keeping old virtual indices for blueprint data.
                    perform_deep_deletions: true,
                });
                if !store_events.is_empty() {
                    re_log::debug!("Garbage-collected blueprint store");
                    if let Some(caches) = self.caches_per_recording.get_mut(blueprint_id) {
                        caches.on_store_events(&store_events, blueprint);
                    }
                }

                self.blueprint_last_gc
                    .insert(blueprint_id.clone(), blueprint.generation());
            }
        }
    }

    /// See [`crate::Caches::begin_frame`].
    pub fn begin_frame_caches(&mut self) {
        self.caches_per_recording.retain(|store_id, caches| {
            if self.store_bundle.contains(store_id) {
                caches.begin_frame();
                true // keep caches for existing recordings
            } else {
                false // remove caches for recordings that no longer exist
            }
        });
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

        #[expect(clippy::iter_over_hash_type)]
        for (app_id, blueprint_id) in &self.active_blueprint_by_app_id {
            if app_id == &Self::welcome_screen_app_id() {
                continue; // Don't save changes to the welcome screen
            }

            let Some(blueprint) = self.store_bundle.get_mut(blueprint_id) else {
                re_log::debug!("Failed to find blueprint {blueprint_id:?}.");
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

        if let Some(loader) = &self.persistence.loader
            && let Some(bundle) = (loader)(app_id)?
        {
            self.load_blueprint_store(bundle, app_id)?;
        }

        Ok(())
    }

    /// Load a blueprint and make it active for the given `ApplicationId`.
    pub fn load_blueprint_store(
        &mut self,
        mut blueprint_bundle: StoreBundle,
        app_id: &ApplicationId,
    ) -> anyhow::Result<()> {
        re_tracing::profile_function!();

        for store in blueprint_bundle.drain_entity_dbs() {
            match store.store_kind() {
                StoreKind::Recording => {
                    anyhow::bail!(
                        "Found a recording in a blueprint file: {:?}",
                        store.store_id()
                    );
                }
                StoreKind::Blueprint => {}
            }

            if store.application_id() != app_id {
                anyhow::bail!("Found app_id {}; expected {app_id}", store.application_id());
            }

            // We found the blueprint we were looking for; make it active.
            // borrow-checker won't let us just call `self.set_blueprint_for_app_id`
            re_log::debug!(
                "Activating new blueprint {:?} for {app_id}.",
                store.store_id(),
            );
            self.active_blueprint_by_app_id
                .insert(app_id.clone(), store.store_id().clone());
            self.blueprint_last_save
                .insert(store.store_id().clone(), store.generation());
            self.store_bundle.insert(store);
        }

        Ok(())
    }

    /// Populate a [`StoreHubStats`].
    pub fn stats(&self) -> StoreHubStats {
        re_tracing::profile_function!();

        let Self {
            persistence: _,
            active_recording_or_table: _,
            active_application_id: _,
            default_blueprint_by_app_id: _,
            active_blueprint_by_app_id: _,
            store_bundle,
            table_stores,
            data_source_order: _,
            should_enable_heuristics_by_app_id: _,
            caches_per_recording,
            blueprint_last_save: _,
            blueprint_last_gc: _,
        } = self;

        let mut store_stats = BTreeMap::new();

        for store in store_bundle.entity_dbs() {
            let store_id = store.store_id();
            let engine = store.storage_engine();
            let cache_vram_usage = caches_per_recording
                .get(store_id)
                .map(|caches| caches.vram_usage())
                .unwrap_or_default();
            store_stats.insert(
                store_id.clone(),
                StoreStats {
                    store_config: engine.store().config().clone(),
                    store_stats: engine.store().stats(),
                    query_cache_stats: engine.cache().stats(),
                    cache_vram_usage,
                },
            );
        }

        let mut table_stats = BTreeMap::new();

        #[expect(clippy::iter_over_hash_type)]
        for (table_id, table_store) in table_stores {
            table_stats.insert(table_id.clone(), table_store.total_size_bytes());
        }

        StoreHubStats {
            store_stats,
            table_stats,
        }
    }
}

impl MemUsageTreeCapture for StoreHub {
    #[expect(clippy::iter_over_hash_type)]
    fn capture_mem_usage_tree(&self) -> MemUsageTree {
        re_tracing::profile_function!();

        let Self {
            store_bundle,
            table_stores,
            caches_per_recording,

            // Small stuff:
            persistence: _,
            active_recording_or_table: _,
            active_application_id: _,
            default_blueprint_by_app_id: _,
            active_blueprint_by_app_id: _,
            data_source_order: _,
            should_enable_heuristics_by_app_id: _,
            blueprint_last_save: _,
            blueprint_last_gc: _,
        } = self;

        let mut node = MemUsageNode::new();

        // Collect all store IDs from both store_bundle and caches_per_recording
        let mut all_store_ids = std::collections::BTreeSet::new();
        for entity_db in store_bundle.entity_dbs() {
            all_store_ids.insert(entity_db.store_id().clone());
        }
        for store_id in caches_per_recording.keys() {
            all_store_ids.insert(store_id.clone());
        }

        // Group stores by recording ID, combining EntityDb and Caches
        let mut stores_node = MemUsageNode::new();
        for store_id in all_store_ids {
            let recording_id = format!("{store_id:?}");
            let mut recording_node = MemUsageNode::new();

            // Add EntityDb if it exists
            if let Some(entity_db) = store_bundle.get(&store_id) {
                recording_node.add("EntityDb", entity_db.capture_mem_usage_tree());
            }

            // Add Caches if they exist for this store
            if let Some(caches) = caches_per_recording.get(&store_id) {
                recording_node.add("Caches", caches.capture_mem_usage_tree());
            }

            stores_node.add(recording_id, recording_node.into_tree());
        }
        node.add("stores", stores_node.into_tree());

        // table_stores
        let mut table_stores_node = MemUsageNode::new();
        for (table_id, table_store) in table_stores {
            let name = format!("{table_id:?}");
            table_stores_node.add(name, MemUsageTree::Bytes(table_store.total_size_bytes()));
        }
        node.add("TableStores", table_stores_node.into_tree());

        node.into_tree()
    }
}
