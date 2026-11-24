//! The store ids of the default and active blueprint of each app, and the state for loading and
//! saving blueprints.

use ahash::{HashMap, HashSet};
use re_chunk_store::ChunkStoreGeneration;
use re_entity_db::EntityDb;
use re_log_types::{ApplicationId, StoreId};
use re_types_core::SegmentId;

use super::blueprint_persistence::{
    BlueprintPersistence, BlueprintPersistenceKey, CachedBlueprintSource, DatasetBlueprintCopy,
};

/// The state of one blueprint store in the store bundle of [`super::StoreHub`].
#[derive(Default)]
struct BlueprintState {
    /// Set if the blueprint is a copy of a dataset blueprint.
    ///
    /// See [`StoreHubBlueprints::cached_blueprint_source`].
    dataset_copy: Option<DatasetBlueprintCopy>,

    /// The [`ChunkStoreGeneration`] that persisted storage has for the blueprint.
    ///
    /// Set when the blueprint is saved, or loaded from persisted storage.
    saved_generation: Option<ChunkStoreGeneration>,

    /// The [`ChunkStoreGeneration`] from when the blueprint was last garbage collected.
    gc_generation: Option<ChunkStoreGeneration>,
}

/// The default and active blueprint of each app, and the state for loading and saving blueprints.
///
/// The blueprint stores are in the store bundle of [`super::StoreHub`], including the inactive
/// blueprints that have a cache entry and aren't saved yet.
pub struct StoreHubBlueprints {
    /// How we load and save blueprints.
    persistence: BlueprintPersistence,

    default_blueprint_by_app_id: HashMap<ApplicationId, StoreId>,
    active_blueprint_by_app_id: HashMap<ApplicationId, StoreId>,

    /// The state of each blueprint store.
    states: HashMap<StoreId, BlueprintState>,

    /// The blueprint of each app that has the [`CachedBlueprintSource::Local`] cache entry.
    local_entry_blueprint_by_app_id: HashMap<ApplicationId, StoreId>,

    /// Apps whose blueprint from [`BlueprintPersistence::legacy_path_blueprint_loader`] is loaded.
    loaded_legacy_path_blueprint_apps: HashSet<ApplicationId>,

    /// These applications should enable the heuristics early next frame.
    should_enable_heuristics_by_app_id: HashSet<ApplicationId>,
}

impl StoreHubBlueprints {
    pub fn new(persistence: BlueprintPersistence) -> Self {
        Self {
            persistence,
            default_blueprint_by_app_id: Default::default(),
            active_blueprint_by_app_id: Default::default(),
            states: Default::default(),
            local_entry_blueprint_by_app_id: Default::default(),
            loaded_legacy_path_blueprint_apps: Default::default(),
            should_enable_heuristics_by_app_id: Default::default(),
        }
    }

    /// How we load and save blueprints.
    pub fn persistence(&self) -> &BlueprintPersistence {
        &self.persistence
    }

    // ---------------------
    // Default and active blueprints

    pub fn default_blueprint_id(&self, app_id: &ApplicationId) -> Option<&StoreId> {
        self.default_blueprint_by_app_id.get(app_id)
    }

    /// Makes `blueprint_id` the default blueprint of `app_id`.
    pub fn set_default_blueprint(&mut self, app_id: ApplicationId, blueprint_id: StoreId) {
        self.default_blueprint_by_app_id
            .insert(app_id, blueprint_id);
    }

    pub fn active_blueprint_id(&self, app_id: &ApplicationId) -> Option<&StoreId> {
        self.active_blueprint_by_app_id.get(app_id)
    }

    /// Makes `blueprint_id` the active blueprint of `app_id`, and returns the previous one.
    pub fn set_active_blueprint(
        &mut self,
        app_id: ApplicationId,
        blueprint_id: StoreId,
    ) -> Option<StoreId> {
        self.active_blueprint_by_app_id.insert(app_id, blueprint_id)
    }

    /// Removes the active blueprint of `app_id`, and returns it.
    pub fn take_active_blueprint(&mut self, app_id: &ApplicationId) -> Option<StoreId> {
        self.active_blueprint_by_app_id.remove(app_id)
    }

    /// Whether `blueprint_id` is the active blueprint of any app.
    pub fn is_active_blueprint(&self, blueprint_id: &StoreId) -> bool {
        self.active_blueprint_by_app_id
            .values()
            .any(|id| id == blueprint_id)
    }

    /// Removes the default and active blueprint of `app_id` if `is_loaded` returns false for them.
    pub fn remove_missing_blueprints(
        &mut self,
        app_id: &ApplicationId,
        is_loaded: impl Fn(&StoreId) -> bool,
    ) {
        for blueprint_by_app_id in [
            &mut self.default_blueprint_by_app_id,
            &mut self.active_blueprint_by_app_id,
        ] {
            if blueprint_by_app_id
                .get(app_id)
                .is_some_and(|blueprint_id| !is_loaded(blueprint_id))
            {
                blueprint_by_app_id.remove(app_id);
            }
        }
    }

    // ---------------------
    // Heuristics

    /// Makes `app_id` enable the heuristics early next frame.
    pub fn set_should_enable_heuristics(&mut self, app_id: ApplicationId) {
        self.should_enable_heuristics_by_app_id.insert(app_id);
    }

    /// Whether `app_id` enables the heuristics early next frame.
    pub fn should_enable_heuristics(&self, app_id: &ApplicationId) -> bool {
        self.should_enable_heuristics_by_app_id.contains(app_id)
    }

    /// Whether `app_id` enables the heuristics this frame. Returns true only once per request.
    pub fn take_should_enable_heuristics(&mut self, app_id: &ApplicationId) -> bool {
        self.should_enable_heuristics_by_app_id.remove(app_id)
    }

    // ---------------------
    // Removing stores and apps

    /// Removes the blueprint `store_id` as default and active blueprint, and drops its state.
    pub fn remove_store(&mut self, store_id: &StoreId) {
        self.active_blueprint_by_app_id
            .retain(|_, id| id != store_id);
        self.default_blueprint_by_app_id
            .retain(|_, id| id != store_id);
        self.local_entry_blueprint_by_app_id
            .retain(|_, id| id != store_id);
        self.states.remove(store_id);
    }

    /// Removes the default and active blueprint of `app_id`, and drops the state of its
    /// blueprints.
    ///
    /// Also forgets that the blueprint at the legacy path of `app_id` is loaded, so it loads again.
    pub fn remove_app(&mut self, app_id: &ApplicationId) {
        self.default_blueprint_by_app_id.remove(app_id);
        self.active_blueprint_by_app_id.remove(app_id);
        self.loaded_legacy_path_blueprint_apps.remove(app_id);
        self.retain_stores(|store_id| store_id.application_id() != app_id);
    }

    /// Drops the state of each blueprint where `should_retain` returns false.
    pub fn retain_stores(&mut self, should_retain: impl Fn(&StoreId) -> bool) {
        self.states.retain(|store_id, _| should_retain(store_id));
        self.local_entry_blueprint_by_app_id
            .retain(|_, store_id| should_retain(store_id));
    }

    // ---------------------
    // Cache entries

    /// Records that `blueprint_id` is a copy of a dataset blueprint.
    pub fn insert_dataset_blueprint_copy(
        &mut self,
        blueprint_id: StoreId,
        copy: DatasetBlueprintCopy,
    ) {
        self.states.entry(blueprint_id).or_default().dataset_copy = Some(copy);
    }

    /// The copies of the dataset blueprint `blueprint_id` of `app_id`.
    ///
    /// When a copy becomes active, [`super::StoreHub::replace_active_blueprint`] removes the
    /// other copies from the store bundle.
    pub fn dataset_blueprint_copies(
        &self,
        app_id: &ApplicationId,
        blueprint_id: &SegmentId,
    ) -> impl Iterator<Item = &StoreId> {
        self.states.iter().filter_map(move |(copy_id, state)| {
            let copy = state.dataset_copy.as_ref()?;
            (copy_id.application_id() == app_id && copy.blueprint_id == *blueprint_id)
                .then_some(copy_id)
        })
    }

    /// The blueprint of `app_id` that has the [`CachedBlueprintSource::Local`] cache entry.
    pub fn local_entry_blueprint(&self, app_id: &ApplicationId) -> Option<&StoreId> {
        self.local_entry_blueprint_by_app_id.get(app_id)
    }

    /// Gives the [`CachedBlueprintSource::Local`] cache entry of `app_id` to `blueprint_id`,
    /// and returns the blueprint that had it before.
    pub fn set_local_entry_blueprint(
        &mut self,
        app_id: ApplicationId,
        blueprint_id: StoreId,
    ) -> Option<StoreId> {
        self.local_entry_blueprint_by_app_id
            .insert(app_id, blueprint_id)
    }

    /// Removes the [`CachedBlueprintSource::Local`] cache entry of `app_id` from its
    /// blueprint, and returns that blueprint.
    pub fn take_local_entry_blueprint(&mut self, app_id: &ApplicationId) -> Option<StoreId> {
        self.local_entry_blueprint_by_app_id.remove(app_id)
    }

    /// The cache entry that `blueprint_id` is saved to, if it has one.
    ///
    /// Each copy of a dataset blueprint has the cache entry of that dataset blueprint. Of the
    /// other blueprints of an app, like the heuristic blueprint and the ones from the SDK or from
    /// a file, the one that was active last has the [`CachedBlueprintSource::Local`] cache entry.
    pub fn cached_blueprint_source(&self, blueprint_id: &StoreId) -> Option<CachedBlueprintSource> {
        if let Some(copy) = self
            .states
            .get(blueprint_id)
            .and_then(|state| state.dataset_copy.as_ref())
        {
            Some(CachedBlueprintSource::Dataset(copy.blueprint_id.clone()))
        } else if self.local_entry_blueprint(blueprint_id.application_id()) == Some(blueprint_id) {
            Some(CachedBlueprintSource::Local)
        } else {
            None
        }
    }

    /// The persistence key of the cache entry that `blueprint_id` is saved to, if it has one.
    pub fn cache_key(&self, blueprint_id: &StoreId) -> Option<BlueprintPersistenceKey> {
        self.cached_blueprint_source(blueprint_id).map(|source| {
            BlueprintPersistenceKey::recording(blueprint_id.application_id().clone(), source)
        })
    }

    /// Whether `blueprint` has data that the blueprint it was copied from doesn't have.
    ///
    /// A copy of a dataset blueprint is edited once its latest row id differs from the one it
    /// was cloned with. Any other blueprint is edited once it has data.
    pub fn is_edited(&self, blueprint: &EntityDb) -> bool {
        match self
            .states
            .get(blueprint.store_id())
            .and_then(|state| state.dataset_copy.as_ref())
        {
            Some(copy) => blueprint.latest_row_id() != copy.latest_row_id_at_clone,
            None => blueprint.latest_row_id().is_some(),
        }
    }

    // ---------------------
    // Blueprints at legacy paths

    /// Whether the blueprint at the legacy path of `app_id` is loaded.
    pub fn has_loaded_legacy_path_blueprint(&self, app_id: &ApplicationId) -> bool {
        self.loaded_legacy_path_blueprint_apps.contains(app_id)
    }

    /// Records that the blueprint at the legacy path of `app_id` is loaded.
    pub fn insert_loaded_legacy_path_blueprint(&mut self, app_id: ApplicationId) {
        self.loaded_legacy_path_blueprint_apps.insert(app_id);
    }

    // ---------------------
    // Saving and garbage collection

    /// Whether `blueprint_id` was last saved at `generation`.
    pub fn is_saved(&self, blueprint_id: &StoreId, generation: &ChunkStoreGeneration) -> bool {
        self.states
            .get(blueprint_id)
            .is_some_and(|state| state.saved_generation.as_ref() == Some(generation))
    }

    /// Records that `blueprint_id` was saved at `generation`.
    pub fn mark_saved(&mut self, blueprint_id: StoreId, generation: ChunkStoreGeneration) {
        self.states
            .entry(blueprint_id)
            .or_default()
            .saved_generation = Some(generation);
    }

    /// Drops the generation `blueprint_id` was last saved at, so the next save writes it.
    pub fn clear_saved(&mut self, blueprint_id: &StoreId) {
        if let Some(state) = self.states.get_mut(blueprint_id) {
            state.saved_generation = None;
        }
    }

    /// Whether `blueprint_id` was last garbage collected at `generation`.
    pub fn is_garbage_collected(
        &self,
        blueprint_id: &StoreId,
        generation: &ChunkStoreGeneration,
    ) -> bool {
        self.states
            .get(blueprint_id)
            .is_some_and(|state| state.gc_generation.as_ref() == Some(generation))
    }

    /// Records that `blueprint_id` was garbage collected at `generation`.
    pub fn mark_garbage_collected(
        &mut self,
        blueprint_id: StoreId,
        generation: ChunkStoreGeneration,
    ) {
        self.states.entry(blueprint_id).or_default().gc_generation = Some(generation);
    }
}
