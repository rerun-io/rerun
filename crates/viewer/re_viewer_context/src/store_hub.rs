use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, LazyLock};

use ahash::{HashMap, HashMapExt as _, HashSet};
use anyhow::Context as _;
use nohash_hasher::IntMap;
use re_byte_size::{MemUsageNode, MemUsageTree, MemUsageTreeCapture, SizeBytes as _};
use re_chunk_store::{
    ChunkStoreConfig, ChunkStoreGeneration, ChunkStoreStats, GarbageCollectionOptions,
    GarbageCollectionTarget,
};
use re_entity_db::{EntityDb, StoreBundle};
use re_log::debug_assert;
use re_log_channel::LogSource;
use re_log_types::{AbsoluteTimeRange, ApplicationId, StoreId, StoreKind, TableId, TimelinePoint};
use re_query::QueryCachesStats;
use re_sdk_types::archetypes;
use re_sdk_types::components::Timestamp;

use crate::{
    ActiveStoreContext, BlueprintUndoState, RecordingOrTable, Route, StorageContext, StoreCache,
    TableStore, TableStores, ViewClassRegistry,
};

// ---

/// Per-frame usage tracking for an [`EntityDb`].
///
/// Tracks two states giving context to how it's used:
/// - `was_preview`: If the entity db was used the render a preview last frame.
/// - `opened`: If the entity db was explicitly opened by the user and should be
///   shown in the recording list. This is not tracked per frame.
pub struct EntityDbUsages {
    /// Whether this store was rendered as a preview cell in the previous frame.
    prev_preview: bool,

    /// Whether this store is being rendered as a preview cell this frame.
    new_preview: std::sync::atomic::AtomicBool,

    /// True if the user has explicitly opened this store.
    ///
    /// Unlike the frame-based preview flag, this persists across frames
    /// and is not reset by [`Self::update`].
    pub opened: bool,
}

impl Clone for EntityDbUsages {
    fn clone(&self) -> Self {
        Self {
            prev_preview: self.prev_preview,
            new_preview: std::sync::atomic::AtomicBool::new(false),
            opened: self.opened,
        }
    }
}

impl EntityDbUsages {
    fn new() -> Self {
        Self {
            prev_preview: false,
            new_preview: std::sync::atomic::AtomicBool::new(false),
            opened: false,
        }
    }

    /// Returns whether this store was rendered as a preview in the previous frame.
    pub fn was_preview(&self) -> bool {
        self.prev_preview
    }

    /// Mark this store as being rendered as a preview this frame.
    pub fn mark_preview(&self) {
        self.new_preview
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Call once per frame, before any rendering.
    pub fn update(&mut self) {
        self.prev_preview = self
            .new_preview
            .swap(false, std::sync::atomic::Ordering::Relaxed);
    }
}

// ---

/// Interface for accessing all blueprints and recordings.
///
/// The [`StoreHub`] provides access to the [`EntityDb`] instances that are used
/// to store both blueprints and recordings.
///
/// What is currently "active" (which recording, which app) is determined by [`Route`],
/// not by the [`StoreHub`] itself. See [`StoreHub::read_context`].
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
pub struct StoreHub {
    /// How we load and save blueprints.
    persistence: BlueprintPersistence,

    default_blueprint_by_app_id: HashMap<ApplicationId, StoreId>,
    active_blueprint_by_app_id: HashMap<ApplicationId, StoreId>,

    data_source_order: DataSourceOrder,
    store_bundle: StoreBundle,
    table_stores: HashMap<TableId, TableStore>,

    /// These applications should enable the heuristics early next frame.
    should_enable_heuristics_by_app_id: HashSet<ApplicationId>,

    /// Viewer-specific state (caches, subscribers, etc.) per store.
    store_caches: HashMap<StoreId, StoreCache>,

    /// The [`ChunkStoreGeneration`] from when the [`EntityDb`] was last saved
    blueprint_last_save: HashMap<StoreId, ChunkStoreGeneration>,

    /// The [`ChunkStoreGeneration`] from when the [`EntityDb`] was last garbage collected
    blueprint_last_gc: HashMap<StoreId, ChunkStoreGeneration>,

    /// Per-frame usage tracking for each store, and whether the user opened it.
    store_usages: HashMap<StoreId, EntityDbUsages>,
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

/// Delete a persisted blueprint from storage, e.g. disk.
pub type BlueprintDeleter = dyn Fn(&ApplicationId) -> anyhow::Result<()> + Send + Sync;

/// How to save and load blueprints
#[derive(Default)]
pub struct BlueprintPersistence {
    pub loader: Option<Box<BlueprintLoader>>,
    pub saver: Option<Box<BlueprintSaver>>,
    pub validator: Option<Box<BlueprintValidator>>,
    pub deleter: Option<Box<BlueprintDeleter>>,
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
    pub fn welcome_screen_app_id() -> &'static ApplicationId {
        static APP_ID: LazyLock<ApplicationId> = LazyLock::new(|| "Welcome screen".into());
        &APP_ID
    }

    /// Blueprint ID used for the default welcome screen blueprint
    fn welcome_screen_blueprint_id() -> StoreId {
        StoreId::new(
            StoreKind::Blueprint,
            Self::welcome_screen_app_id().clone(),
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
                deleter: None,
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
            Self::welcome_screen_app_id().clone(),
            Self::welcome_screen_blueprint_id(),
        );

        let welcome_screen_blueprint =
            store_bundle.blueprint_entry(&Self::welcome_screen_blueprint_id());
        (setup_welcome_screen_blueprint)(welcome_screen_blueprint);

        Self {
            persistence,

            default_blueprint_by_app_id,
            active_blueprint_by_app_id: Default::default(),
            store_bundle,

            should_enable_heuristics_by_app_id: Default::default(),

            data_source_order: Default::default(),
            store_caches: Default::default(),
            blueprint_last_save: Default::default(),
            blueprint_last_gc: Default::default(),

            store_usages: Default::default(),

            table_stores: TableStores::default(),
        }
    }

    // ---------------------
    // Usage tracking

    /// Mark a store as being rendered as a preview this frame.
    ///
    /// Safe to call with a shared reference — uses atomics internally.
    /// Has no effect if the store is not (yet) tracked.
    pub fn mark_preview(&self, store_id: &StoreId) {
        if let Some(usages) = self.store_usages.get(store_id) {
            usages.mark_preview();
        }
    }

    /// Whether a store was rendered as a preview in the previous frame.
    ///
    /// This is true regardless of whether the store is also opened.
    pub fn was_preview(&self, store_id: &StoreId) -> bool {
        self.store_usages
            .get(store_id)
            .is_some_and(|u| u.was_preview())
    }

    /// Returns the usage tracking for a store.
    pub fn usage(&self, store_id: &StoreId) -> EntityDbUsages {
        self.store_usages
            .get(store_id)
            .cloned()
            .unwrap_or_else(EntityDbUsages::new)
    }

    /// Set or clear the [`EntityDbUsages::opened`] flag for a store.
    pub fn set_opened(&mut self, store_id: &StoreId, opened: bool) {
        self.store_usages
            .entry(store_id.clone())
            .or_insert_with(EntityDbUsages::new)
            .opened = opened;
    }

    /// Whether the user has explicitly opened this store.
    pub fn is_opened(&self, store_id: &StoreId) -> bool {
        self.store_usages.get(store_id).is_some_and(|u| u.opened)
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

    /// Get a read-only [`StorageContext`] and [`ActiveStoreContext`] from the [`StoreHub`].
    ///
    /// The [`ActiveStoreContext`] is only returned for routes that have an associated [`ApplicationId`]
    /// with an active blueprint — otherwise, `None` is returned and callers are expected to handle the absence explicitly.
    ///
    /// When returned, all of the references to blueprints and recordings will
    /// have a matching [`ApplicationId`].
    pub fn read_context(
        &mut self,
        route: &Route,
    ) -> (StorageContext<'_>, Option<ActiveStoreContext<'_>>) {
        // Used as stand-ins within the `Some` branch when only parts of a
        // context are available (e.g. we have a blueprint but no recording).
        static EMPTY_RECORDING: LazyLock<EntityDb> =
            LazyLock::new(|| EntityDb::new(re_log_types::StoreId::empty_recording()));
        static EMPTY_CACHES: LazyLock<StoreCache> = LazyLock::new(|| {
            StoreCache::empty(
                &ViewClassRegistry::default(),
                re_log_types::StoreId::empty_recording(),
            )
        });

        let store_context = 'ctx: {
            // If we have an app-id, then use it to look up the blueprint.
            let Some(app_id) = route.app_id() else {
                break 'ctx None;
            };

            self.ensure_active_blueprint_for_app(app_id);
            let should_enable_heuristics = self.should_enable_heuristics_by_app_id.remove(app_id);

            let active_blueprint = {
                let Some(active_blueprint) = self.active_blueprint_for_app(app_id) else {
                    break 'ctx None;
                };
                active_blueprint
            };

            let default_blueprint = self
                .default_blueprint_by_app_id
                .get(app_id)
                .and_then(|id| self.store_bundle.get(id));

            let recording = route
                .recording_id()
                .and_then(|store_id| self.store_bundle.get(store_id));
            let caches = route
                .recording_id()
                .and_then(|store_id| self.store_caches.get(store_id));

            let caches = caches.unwrap_or_else(|| {
                if recording.is_some() {
                    re_log::debug_warn!("Active recording is missing cache");
                }
                &EMPTY_CACHES
            });

            Some(ActiveStoreContext {
                blueprint: active_blueprint,
                default_blueprint,
                recording: recording.unwrap_or(&EMPTY_RECORDING),
                caches,
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

    /// Mutable access to a [`EntityDb`] by id.
    ///
    /// Creates it if it does not already exist.
    // NOTE(grtlr): The sentence above should be read as a _warning_. If we call this
    // function with the expectance to create a new store, the call site has to make sure
    // to also do all of the required book-keeping.
    pub fn entity_db_entry(&mut self, store_id: &StoreId) -> &mut EntityDb {
        self.store_bundle.entry(store_id)
    }

    /// Mutable access to a [`EntityDb`] by id.
    pub fn entity_db_mut(&mut self, store_id: &StoreId) -> Option<&mut EntityDb> {
        self.store_bundle.get_mut(store_id)
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
    /// Note that the recording is not automatically made active. Use [`StoreHub::load_blueprint_and_caches`]
    /// if needed.
    pub fn insert_entity_db(&mut self, entity_db: EntityDb) {
        self.store_bundle.insert(entity_db);
    }

    /// Add a chunk to a store and forward events to the store's [`StoreCache`] (if one exists).
    ///
    /// This is the correct way to add data when a [`StoreCache`] may already exist,
    /// e.g. in test harnesses that bypass the normal message channel.
    pub fn add_chunk_for_tests(
        &mut self,
        store_id: &StoreId,
        chunk: &std::sync::Arc<re_chunk::Chunk>,
    ) -> anyhow::Result<Vec<re_chunk_store::ChunkStoreEvent>> {
        let entity_db = self
            .store_bundle
            .get_mut(store_id)
            .context("missing store")?;
        let events = entity_db.add_chunk(chunk)?;

        // Forward events to the cache so subscribers stay up to date.
        let entity_db = self
            .store_bundle
            .get(store_id)
            .expect("store was just accessed");
        if let Some(cache) = self.store_caches.get_mut(store_id) {
            cache.on_store_events(&events, entity_db);
        }

        Ok(events)
    }

    /// Inserts a new table into the store (potentially overwriting an existing entry).
    pub fn insert_table_store(&mut self, id: TableId, store: TableStore) -> Option<TableStore> {
        self.table_stores.insert(id, store)
    }

    fn remove_store(&mut self, store_id: &StoreId) {
        _ = self.store_caches.remove(store_id);
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
            if db.application_id() == Self::welcome_screen_app_id() {
                store_ids_retained.insert(db.store_id().clone());
                true
            } else {
                false
            }
        });
        self.store_caches
            .retain(|store_id, _| store_ids_retained.contains(store_id));

        self.table_stores.clear();
    }

    // ---------------------
    // App management

    /// Load persisted blueprints for the given [`ApplicationId`], if they exist and aren't there already.
    pub fn load_persisted_blueprints_for_app(&mut self, app_id: &ApplicationId) {
        if !self.active_blueprint_by_app_id.contains_key(app_id)
            && let Err(err) = self.try_to_load_persisted_blueprint(app_id)
        {
            re_log::warn!("Failed to load persisted blueprint: {err}");
        }
    }

    /// Find the earliest recording for the given [`ApplicationId`].
    pub fn earliest_recording_for_app(&self, app_id: &ApplicationId) -> Option<StoreId> {
        self.store_bundle
            .recordings()
            .filter(|rec| rec.application_id() == app_id)
            .min_by_key(|entity_db| {
                entity_db.recording_info_property::<Timestamp>(
                    archetypes::RecordingInfo::descriptor_start_time().component,
                )
            })
            .map(|rec| rec.store_id().clone())
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
        self.store_caches
            .retain(|store_id, _| !store_ids_removed.contains(store_id));

        self.default_blueprint_by_app_id.remove(app_id);
        self.active_blueprint_by_app_id.remove(app_id);
    }

    // ---------------------
    // Recording management

    /// Get the [`StoreCache`] for a given store.
    ///
    /// Returns `None` if no state exists for this store.
    pub fn store_caches(&self, store_id: &StoreId) -> Option<&StoreCache> {
        self.store_caches.get(store_id)
    }

    /// Get both the [`EntityDb`] and [`StoreCache`] for a given store.
    ///
    /// Uses split borrows to allow simultaneous access.
    pub fn entity_db_and_cache(
        &mut self,
        store_id: &StoreId,
        view_class_registry: &ViewClassRegistry,
    ) -> Option<(&EntityDb, &mut StoreCache)> {
        let entity_db = self.store_bundle.get(store_id)?;
        let cache = self
            .store_caches
            .entry(store_id.clone())
            .or_insert_with(|| StoreCache::new(view_class_registry, entity_db));
        Some((entity_db, cache))
    }

    /// Ensure caches and blueprints are set up for the given recording.
    ///
    /// Call this when a recording becomes active (e.g. via [`Route::LocalRecording`]).
    // TODO(RR-3033): get rid of this?
    pub fn load_blueprint_and_caches(
        &mut self,
        recording_id: &StoreId,
        view_class_registry: &ViewClassRegistry,
    ) {
        debug_assert!(recording_id.is_recording());

        // Ensure persisted blueprints are loaded for this recording's app.
        if let Some(app_id) = self
            .store_bundle
            .get(recording_id)
            .map(|recording| recording.application_id().clone())
        {
            self.load_persisted_blueprints_for_app(&app_id);
        }

        let store_bundle = &self.store_bundle;
        let store_caches = &mut self.store_caches;
        if let Some(entity_db) = store_bundle.get(recording_id) {
            store_caches
                .entry(recording_id.clone())
                .or_insert_with(|| StoreCache::new(view_class_registry, entity_db));
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

    /// Ensure there is an active blueprint for the given app.
    ///
    /// First cleans up stale references, then:
    /// - If there is already an active blueprint, does nothing.
    /// - If heuristics are about to be enabled, creates a new empty blueprint.
    /// - If there is a default blueprint, clones it and makes it active.
    /// - Otherwise, creates an empty blueprint and registers it as active.
    pub fn ensure_active_blueprint_for_app(&mut self, app_id: &ApplicationId) {
        // Clean up stale references in case our book-keeping is broken.
        if let Some(blueprint_id) = self.default_blueprint_by_app_id.get(app_id)
            && !self.store_bundle.contains(blueprint_id)
        {
            self.default_blueprint_by_app_id.remove(app_id);
        }
        if let Some(blueprint_id) = self.active_blueprint_by_app_id.get(app_id)
            && !self.store_bundle.contains(blueprint_id)
        {
            self.active_blueprint_by_app_id.remove(app_id);
        }

        if self.active_blueprint_by_app_id.contains_key(app_id) {
            return;
        }

        // If heuristics are about to run, create a new empty blueprint to write into.
        if self.should_enable_heuristics_by_app_id.contains(app_id) {
            let blueprint_id = StoreId::default_blueprint(app_id.clone());
            self.store_bundle.blueprint_entry(&blueprint_id);
            self.active_blueprint_by_app_id
                .insert(app_id.clone(), blueprint_id);
            return;
        }

        // Try to clone the default blueprint.
        if let Some(blueprint_id) = self.default_blueprint_by_app_id.get(app_id).cloned() {
            self.set_cloned_blueprint_active_for_app(&blueprint_id)
                .unwrap_or_else(|err| {
                    re_log::warn!("Failed to make blueprint active: {err}");
                });
            return;
        }

        // No default blueprint exists, create an empty one.
        let blueprint_id = StoreId::default_blueprint(app_id.clone());
        self.store_bundle.blueprint_entry(&blueprint_id);
        self.active_blueprint_by_app_id
            .insert(app_id.clone(), blueprint_id);
    }

    pub fn active_blueprint_id_for_app(&self, app_id: &ApplicationId) -> Option<&StoreId> {
        self.active_blueprint_by_app_id.get(app_id)
    }

    pub fn active_blueprint_for_app(&self, app_id: &ApplicationId) -> Option<&EntityDb> {
        let id = self.active_blueprint_id_for_app(app_id)?;
        self.store_bundle.get(id)
    }

    /// Like [`Self::active_blueprint_for_app`], but derives the app id from a [`Route`].
    pub fn active_blueprint_for_route(&self, route: &Route) -> Option<&EntityDb> {
        let app_id = route.app_id()?;
        self.active_blueprint_for_app(app_id)
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

    /// Clear the currently active blueprint for the given app.
    fn clear_active_blueprint_for_app_id(&mut self, app_id: &ApplicationId) {
        if let Some(blueprint_id) = self.active_blueprint_by_app_id.remove(app_id) {
            re_log::debug!("Clearing blueprint for {app_id}: {blueprint_id:?}");
            self.remove_store(&blueprint_id);
        }
    }

    /// Clear the currently active blueprint
    pub fn clear_active_blueprint(&mut self, route: &Route) {
        if let Some(app_id) = route.app_id() {
            self.clear_active_blueprint_for_app_id(app_id);
        }
    }

    /// Clear the currently active blueprint and enable the heuristics to generate a new one.
    ///
    /// These keeps the default blueprint as is, so the user may reset to the default blueprint
    /// afterward.
    pub fn clear_active_blueprint_and_generate(&mut self, route: &Route) {
        if let Some(app_id) = route.app_id() {
            self.clear_active_blueprint_for_app_id(app_id);

            // Simply clearing the default blueprint would trigger a reset to default. Instead, we must
            // actively set the blueprint to use the heuristics, so we store a flag so we do that early
            // next frame.
            self.should_enable_heuristics_by_app_id
                .insert(app_id.clone());
        }
    }

    /// Clear active blueprints (in-memory and on disk) for all `app_ids` that have
    /// recordings originating from the given server.
    pub fn clear_blueprints_for_origin(&mut self, origin: &re_uri::Origin) {
        let affected_app_ids: BTreeSet<ApplicationId> = self
            .store_bundle
            .recordings()
            .filter(|db| {
                matches!(
                    &db.data_source,
                    Some(LogSource::RedapGrpcStream { uri, .. }) if uri.origin == *origin
                )
            })
            .map(|db| db.application_id().clone())
            .collect();

        for app_id in &affected_app_ids {
            self.clear_active_blueprint_for_app_id(app_id);
        }

        if let Some(deleter) = &self.persistence.deleter {
            for app_id in &affected_app_ids {
                if let Err(err) = (deleter)(app_id) {
                    re_log::warn!("Failed to delete persisted blueprint for {app_id}: {err}");
                }
            }
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

    /// Call [`EntityDb::gc`] on every recording
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
        active_recording_id: Option<&StoreId>,
        time_cursor_for: &dyn Fn(&StoreId) -> Option<TimelinePoint>,
    ) -> u64 {
        re_tracing::profile_function!();

        #[expect(clippy::iter_over_hash_type)]
        for cache in self.store_caches.values_mut() {
            cache.purge_memory();
        }

        // Currently active recording.
        let mut active_recordings = Vec::new();
        // Opened in the recording panel, but not currently active.
        let mut opened_inactive_recordings = Vec::new();
        // Used as a preview.
        let mut preview_recordings = Vec::new();
        // Not used anywhere.
        let mut background_recordings = Vec::new();

        for recording in self.store_bundle.recordings() {
            let id = recording.store_id().clone();
            let Some(usage) = self.store_usages.get(recording.store_id()) else {
                background_recordings.push(id);
                continue;
            };

            if active_recording_id == Some(recording.store_id()) {
                active_recordings.push(id);
            } else if usage.was_preview() {
                preview_recordings.push(id);
            } else if usage.opened {
                opened_inactive_recordings.push(id);
            } else {
                background_recordings.push(id);
            }
        }

        let mut num_bytes_freed = 0;

        // Just close unused recordings if we purge memory.
        for store_id in &background_recordings {
            let size = self
                .store_bundle
                .get(store_id)
                .map(|db| db.total_size_bytes())
                .unwrap_or(0);

            num_bytes_freed += size;
            self.remove_store(store_id);
        }

        let inactive_target = GarbageCollectionTarget::Everything;

        for store_id in &opened_inactive_recordings {
            let time_cursor = time_cursor_for(store_id);
            num_bytes_freed += self.gc_store(inactive_target, store_id, time_cursor);
        }

        let target = GarbageCollectionTarget::DropAtLeastFraction(fraction_to_purge as _);

        for store_id in preview_recordings.iter().chain(&active_recordings) {
            let time_cursor = time_cursor_for(store_id);
            num_bytes_freed += self.gc_store(target, store_id, time_cursor);
        }

        // Didn't free memory from the active recording, or background recordings,
        // so resort to closing background recordings.
        if num_bytes_freed == 0 {
            let mut closed_count = 0_usize;
            for store_id in &opened_inactive_recordings {
                let Some(recording) = self.store_bundle.get(store_id) else {
                    continue;
                };

                // Don't close recordings with protected chunks — we'd just need to re-download them.
                if recording.has_protected_chunks() {
                    continue;
                }

                num_bytes_freed += recording.total_size_bytes();

                self.remove_store(store_id);

                closed_count += 1;
            }

            if closed_count > 0 {
                re_log::warn!(
                    "Closed {} to stay within memory limit",
                    re_format::format_plural_s(closed_count, "background recording")
                );
            }
        }

        num_bytes_freed
    }

    /// Call [`EntityDb::gc`] on every recording
    ///
    /// `time_cursor` can be used for more intelligent targeting of what is to be removed.
    //
    // NOTE: If you touch any of this, make sure to play around with our GC stress test scripts
    // available under `$WORKSPACE_ROOT/tests/python/gc_stress`.
    fn gc_store(
        &mut self,
        target: GarbageCollectionTarget,
        store_id: &StoreId,
        time_cursor: Option<TimelinePoint>,
    ) -> u64 {
        re_tracing::profile_function!();

        let store_bundle = &mut self.store_bundle;

        let Some(entity_db) = store_bundle.get_mut(store_id) else {
            if cfg!(debug_assertions) {
                unreachable!();
            }
            return 0; // unreachable
        };

        let store_size_before = entity_db.total_size_bytes();
        let store_events = entity_db.gc_with_target(target, time_cursor);
        let store_size_after = entity_db.total_size_bytes();

        if let Some(cache) = self.store_caches.get_mut(store_id) {
            cache.on_store_events(&store_events, entity_db);
        }

        store_size_before.saturating_sub(store_size_after)
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
                re_log_channel::LogSource::HttpStream { url, .. } => url != uri,

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
                    protected_chunks: HashSet::default(),
                    furthest_from: None,
                    // There is no point in keeping old virtual indices for blueprint data.
                    perform_deep_deletions: true,
                });
                if !store_events.is_empty() {
                    re_log::debug!("Garbage-collected blueprint store");
                    if let Some(cache) = self.store_caches.get_mut(blueprint_id) {
                        cache.on_store_events(&store_events, blueprint);
                    }
                }

                self.blueprint_last_gc
                    .insert(blueprint_id.clone(), blueprint.generation());
            }
        }
    }

    /// See [`crate::StoreCache::begin_frame`].
    pub fn begin_frame_caches(&mut self, active_recording: Option<&StoreId>) {
        // Sync usage entries: remove entries for stores that no longer exist,
        // and ensure entries exist for all current stores.
        self.store_usages
            .retain(|id, _| self.store_bundle.contains(id));
        for db in self.store_bundle.entity_dbs() {
            self.store_usages
                .entry(db.store_id().clone())
                .or_insert_with(EntityDbUsages::new);
        }

        // Rotate: move new → prev for the previous frame's usage.
        // Order doesn't matter — each entry is independent.
        #[expect(clippy::iter_over_hash_type)]
        for usages in self.store_usages.values_mut() {
            usages.update();
        }

        // Pre-compute which stores were rendered last frame to avoid borrow conflicts in retain.
        // Always include the active recording so its cache is fresh on the first frame
        // after switching.
        let mut used_last_frame: HashSet<StoreId> = self
            .store_usages
            .iter()
            .filter(|(_, u)| u.was_preview())
            .map(|(id, _)| id.clone())
            .collect();
        if let Some(active) = active_recording {
            used_last_frame.insert(active.clone());
        }

        self.store_caches.retain(|store_id, cache| {
            if self.store_bundle.contains(store_id) {
                // Only refresh the cache for stores that were actually rendered last frame.
                if used_last_frame.contains(store_id) {
                    cache.begin_frame();
                }
                true // keep cache for existing recordings
            } else {
                false // remove cache for recordings that no longer exist
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
            if app_id == Self::welcome_screen_app_id() {
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
            default_blueprint_by_app_id: _,
            active_blueprint_by_app_id: _,
            store_bundle,
            table_stores,
            data_source_order: _,
            should_enable_heuristics_by_app_id: _,

            store_caches,
            blueprint_last_save: _,
            blueprint_last_gc: _,
            store_usages: _,
        } = self;

        let mut store_stats = BTreeMap::new();

        for store in store_bundle.entity_dbs() {
            let store_id = store.store_id();
            let engine = store.storage_engine();
            let cache_vram_usage = store_caches
                .get(store_id)
                .map(|cache| cache.vram_usage())
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
            store_caches,

            // Small stuff:
            persistence: _,
            default_blueprint_by_app_id: _,
            active_blueprint_by_app_id: _,
            data_source_order: _,
            should_enable_heuristics_by_app_id: _,

            blueprint_last_save: _,
            blueprint_last_gc: _,
            store_usages: _,
        } = self;

        let mut node = MemUsageNode::new();

        // Collect all store IDs from both store_bundle and store_caches
        let mut all_store_ids = std::collections::BTreeSet::new();
        for entity_db in store_bundle.entity_dbs() {
            all_store_ids.insert(entity_db.store_id().clone());
        }
        for store_id in store_caches.keys() {
            all_store_ids.insert(store_id.clone());
        }

        // Group stores by recording ID, combining EntityDb and StoreCache
        let mut stores_node = MemUsageNode::new();
        for store_id in all_store_ids {
            let recording_id = format!("{store_id:?}");
            let mut recording_node = MemUsageNode::new();

            // Add EntityDb if it exists
            if let Some(entity_db) = store_bundle.get(&store_id) {
                recording_node.add("EntityDb", entity_db.capture_mem_usage_tree());
            }

            // Add StoreCache if it exists for this store
            if let Some(cache) = store_caches.get(&store_id) {
                recording_node.add("StoreCache", cache.capture_mem_usage_tree());
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
