//! Saving blueprints to persisted storage and loading them back.

use anyhow::Context as _;
use re_chunk::RowId;
use re_entity_db::{EntityDb, StoreBundle};
use re_log_types::{ApplicationId, StoreId, StoreKind};
use re_types_core::SegmentId;

use super::StoreHub;
use crate::TableReference;

/// One blueprint of an application that the viewer keeps an edited copy of.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CachedBlueprintSource {
    /// The blueprint the heuristics generate, or any other blueprint that isn't a copy of a
    /// dataset blueprint, like one from the SDK or from a file.
    ///
    /// An app has one cache entry for all of these, which holds the one that was active last.
    Local,

    /// A blueprint of a catalog dataset, by its segment id in the dataset's blueprint dataset.
    Dataset(SegmentId),
}

/// A copy of a dataset blueprint. Once edited, the copy is saved to the cache entry of the
/// dataset blueprint.
pub struct DatasetBlueprintCopy {
    pub blueprint_id: SegmentId,

    /// The latest row id of the copy when it was cloned from the dataset blueprint.
    ///
    /// A different latest row id means the copy has been edited. A copy loaded from its cache
    /// entry has `None`, so it counts as edited.
    pub latest_row_id_at_clone: Option<RowId>,
}

/// Identifies a persisted application or table blueprint.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BlueprintPersistenceKey {
    /// The edited copy of one blueprint of an application.
    Recording {
        application_id: ApplicationId,
        source: CachedBlueprintSource,
    },

    Table(Box<TableReference>),
}

impl BlueprintPersistenceKey {
    pub fn recording(application_id: ApplicationId, source: CachedBlueprintSource) -> Self {
        Self::Recording {
            application_id,
            source,
        }
    }

    /// The application of a [`Self::Recording`] key.
    pub fn application_id(&self) -> Option<&ApplicationId> {
        match self {
            Self::Recording { application_id, .. } => Some(application_id),
            Self::Table(_) => None,
        }
    }
}

/// Delete all persisted blueprints of an application.
pub type AppBlueprintClearer = dyn Fn(&ApplicationId) -> anyhow::Result<()> + Send + Sync;

/// Load the blueprint of an application from its legacy path, where earlier versions of the
/// viewer persisted one blueprint per application.
///
/// The file has the same format as any other persisted blueprint. It stays at the legacy path
/// until [`LegacyPathBlueprintDeleter`] deletes it.
///
/// Returns `Ok(None)` if there is none, or if the application already has blueprints persisted
/// by this version.
pub type LegacyPathBlueprintLoader =
    dyn Fn(&ApplicationId) -> anyhow::Result<Option<StoreBundle>> + Send + Sync;

/// Delete the blueprint of an application at its legacy path, if it exists.
///
/// Called when the blueprint fails to load, and by [`StoreHub::clear_blueprints_for_origin`].
/// Also called by [`StoreHub::clear_active_blueprint`] and
/// [`StoreHub::clear_active_blueprint_and_generate`] once the blueprint is loaded.
pub type LegacyPathBlueprintDeleter = dyn Fn(&ApplicationId) -> anyhow::Result<()> + Send + Sync;

/// Load a blueprint from persisted storage, e.g. disk.
/// Returns `Ok(None)` if no blueprint is found.
pub type BlueprintLoader =
    dyn Fn(&BlueprintPersistenceKey) -> anyhow::Result<Option<StoreBundle>> + Send + Sync;

/// Save a blueprint to persisted storage, e.g. disk.
pub type BlueprintSaver =
    dyn Fn(&BlueprintPersistenceKey, &EntityDb) -> anyhow::Result<()> + Send + Sync;

/// Validate a blueprint against the current blueprint schema requirements.
pub type BlueprintValidator = dyn Fn(&EntityDb) -> bool + Send + Sync;

/// Delete a persisted blueprint from storage, e.g. disk.
pub type BlueprintDeleter = dyn Fn(&BlueprintPersistenceKey) -> anyhow::Result<()> + Send + Sync;

/// Delete all persisted table blueprints from storage.
pub type TableBlueprintClearer = dyn Fn() -> anyhow::Result<()> + Send + Sync;

/// How to save and load blueprints.
#[derive(Default)]
pub struct BlueprintPersistence {
    pub loader: Option<Box<BlueprintLoader>>,
    pub saver: Option<Box<BlueprintSaver>>,
    pub validator: Option<Box<BlueprintValidator>>,
    pub deleter: Option<Box<BlueprintDeleter>>,
    pub table_blueprint_clearer: Option<Box<TableBlueprintClearer>>,
    pub app_blueprint_clearer: Option<Box<AppBlueprintClearer>>,
    pub legacy_path_blueprint_loader: Option<Box<LegacyPathBlueprintLoader>>,
    pub legacy_path_blueprint_deleter: Option<Box<LegacyPathBlueprintDeleter>>,
}

impl StoreHub {
    /// Load the blueprint at the legacy path of the given [`ApplicationId`], if it exists and the
    /// app has no active blueprint. Earlier versions of the viewer persisted it there.
    ///
    /// It loads once per app. Blueprints cached by this version of the viewer are loaded when the
    /// app gets an active blueprint.
    pub fn load_legacy_path_blueprint_for_app(&mut self, app_id: &ApplicationId) {
        if self.blueprints.active_blueprint_id(app_id).is_none()
            && let Err(err) = self.try_load_legacy_path_blueprint(app_id)
        {
            re_log::warn!("Failed to load blueprint at legacy path: {err}");
        }
    }

    /// Removes `blueprint_id` from the store bundle if persisted storage has its current
    /// generation.
    pub(super) fn remove_store_if_saved(&mut self, blueprint_id: &StoreId) {
        let is_saved = self
            .store_bundle
            .get(blueprint_id)
            .is_some_and(|blueprint| {
                self.blueprints
                    .is_saved(blueprint_id, &blueprint.generation())
            });
        if is_saved {
            self.remove_store(blueprint_id);
        }
    }

    /// The store ids of the blueprints in the store bundle that are saved to their cache entry.
    ///
    /// That is each blueprint with a cache entry, except for an inactive one that is the default
    /// blueprint of its app or that another blueprint was cloned from.
    pub(super) fn cached_blueprint_ids(&self) -> Vec<StoreId> {
        let is_source = |blueprint_id: &StoreId| {
            self.blueprints
                .default_blueprint_id(blueprint_id.application_id())
                == Some(blueprint_id)
                || self
                    .store_bundle
                    .entity_dbs()
                    .any(|db| db.cloned_from() == Some(blueprint_id))
        };

        self.store_bundle
            .entity_dbs()
            .map(|db| db.store_id())
            .filter(|store_id| {
                store_id.is_blueprint()
                    && self.blueprints.cached_blueprint_source(store_id).is_some()
                    && (self.blueprints.is_active_blueprint(store_id) || !is_source(store_id))
            })
            .cloned()
            .collect()
    }

    /// The blueprint in the store bundle that has the cache entry of `source`, if there is one.
    fn cached_blueprint_id(
        &self,
        app_id: &ApplicationId,
        source: &CachedBlueprintSource,
    ) -> Option<StoreId> {
        match source {
            CachedBlueprintSource::Local => self
                .blueprints
                .local_entry_blueprint(app_id)
                .filter(|blueprint_id| self.store_bundle.contains(blueprint_id))
                .cloned(),
            CachedBlueprintSource::Dataset(blueprint_id) => self
                .blueprints
                .dataset_blueprint_copies(app_id, blueprint_id)
                .find(|copy_id| self.store_bundle.contains(copy_id))
                .cloned(),
        }
    }

    /// Deletes the cache entry of `blueprint_id` from persisted storage, if it has one.
    pub(super) fn delete_cache_entry(&self, blueprint_id: &StoreId) {
        if let Some(key) = self.blueprints.cache_key(blueprint_id)
            && let Err(err) = self.delete_persisted_blueprint(&key)
        {
            re_log::warn!("Failed to delete cached blueprint: {err}");
        }
    }

    /// Whether `blueprint_id` is in the store bundle and has data that the blueprint it was
    /// copied from doesn't have.
    fn is_edited(&self, blueprint_id: &StoreId) -> bool {
        self.store_bundle
            .get(blueprint_id)
            .is_some_and(|blueprint| self.blueprints.is_edited(blueprint))
    }

    /// Saves `blueprint_id` to its cache entry if it is edited.
    ///
    /// Does nothing if `blueprint_id` has no cache entry. If `blueprint_id` isn't edited, its
    /// cache entry is deleted, so the next copy of its source is a new clone.
    fn save_cached_blueprint(&mut self, blueprint_id: &StoreId) -> anyhow::Result<()> {
        let Some(key) = self.blueprints.cache_key(blueprint_id) else {
            return Ok(());
        };
        let Some(generation) = self
            .store_bundle
            .get(blueprint_id)
            .map(|blueprint| blueprint.generation())
        else {
            return Ok(());
        };

        if self.is_edited(blueprint_id) {
            return self.save_persisted_blueprint_if_changed(&key, blueprint_id);
        }

        if self.blueprints.is_saved(blueprint_id, &generation) {
            return Ok(());
        }
        self.delete_persisted_blueprint(&key)?;
        self.blueprints.mark_saved(blueprint_id.clone(), generation);
        Ok(())
    }

    /// Makes the cached copy of `source` the active blueprint of `app_id`.
    ///
    /// Returns false if there is no cached copy, or if it fails to load.
    pub(super) fn try_activate_cached_blueprint(
        &mut self,
        app_id: &ApplicationId,
        source: &CachedBlueprintSource,
    ) -> bool {
        self.activate_cached_blueprint(app_id, source)
            .unwrap_or_else(|err| {
                re_log::warn!("Failed to load cached blueprint: {err}");
                false
            })
    }

    /// Makes the cached copy of `source` the active blueprint of `app_id`.
    ///
    /// The cached copy is the blueprint in the store bundle with the cache entry of `source`, or
    /// otherwise the one in persisted storage. A blueprint in the store bundle that isn't edited
    /// is removed, and doesn't count as a cached copy.
    ///
    /// Returns `Ok(false)` if there is no cached copy.
    fn activate_cached_blueprint(
        &mut self,
        app_id: &ApplicationId,
        source: &CachedBlueprintSource,
    ) -> anyhow::Result<bool> {
        if let Some(blueprint_id) = self.cached_blueprint_id(app_id, source) {
            if !self.is_edited(&blueprint_id) {
                self.remove_store(&blueprint_id);
                return Ok(false);
            }
            self.replace_active_blueprint(&blueprint_id);
            return Ok(true);
        }

        let key = BlueprintPersistenceKey::recording(app_id.clone(), source.clone());
        let Some(blueprint) = self.load_persisted_blueprint(&key)? else {
            return Ok(false);
        };

        let blueprint_id = blueprint.store_id().clone();
        if blueprint_id.application_id() != app_id {
            anyhow::bail!(
                "Found app_id {} in a cached blueprint, expected {app_id}",
                blueprint_id.application_id()
            );
        }
        if blueprint_id.is_heuristic_blueprint()
            && matches!(source, CachedBlueprintSource::Dataset(_))
        {
            anyhow::bail!("Cached blueprint has an unexpected store id: {blueprint_id:?}");
        }

        self.store_bundle.insert(blueprint);
        self.mark_blueprint_persisted(&blueprint_id);
        if let CachedBlueprintSource::Dataset(segment) = source {
            self.blueprints.insert_dataset_blueprint_copy(
                blueprint_id.clone(),
                DatasetBlueprintCopy {
                    blueprint_id: segment.clone(),
                    latest_row_id_at_clone: None,
                },
            );
        }
        self.replace_active_blueprint(&blueprint_id);

        Ok(true)
    }

    /// Saves each blueprint in the store bundle that has a cache entry to that entry, and removes
    /// the inactive ones from the store bundle once they are saved. An inactive blueprint that is
    /// the default blueprint of its app, or that another blueprint was cloned from, is skipped.
    ///
    /// Without a [`BlueprintPersistence::saver`] the edited inactive blueprints stay in the store
    /// bundle.
    ///
    /// Returns the first error, after trying to save every blueprint.
    pub fn save_app_blueprints(&mut self) -> anyhow::Result<()> {
        re_tracing::profile_function!();

        let mut result = Ok(());
        for blueprint_id in self.cached_blueprint_ids() {
            if blueprint_id.application_id() == Self::welcome_screen_app_id() {
                continue;
            }

            if let Err(err) = self.save_cached_blueprint(&blueprint_id) {
                if result.is_ok() {
                    result = Err(err);
                }
                continue;
            }

            if !self.blueprints.is_active_blueprint(&blueprint_id) {
                self.remove_store_if_saved(&blueprint_id);
            }
        }

        result
    }

    /// Load one persisted blueprint without activating it.
    pub fn load_persisted_blueprint(
        &self,
        key: &BlueprintPersistenceKey,
    ) -> anyhow::Result<Option<EntityDb>> {
        let Some(loader) = &self.blueprints.persistence().loader else {
            return Ok(None);
        };
        let Some(mut bundle) = (loader)(key)? else {
            return Ok(None);
        };

        let mut stores = bundle.drain_entity_dbs();
        let blueprint = match (stores.next(), stores.next()) {
            (Some(blueprint), None) if blueprint.store_kind() != StoreKind::Blueprint => Err(
                anyhow::anyhow!("Found a recording in a persisted blueprint file"),
            ),
            (Some(blueprint), None)
                if self
                    .blueprints
                    .persistence()
                    .validator
                    .as_ref()
                    .is_some_and(|validator| !(validator)(&blueprint)) =>
            {
                Err(anyhow::anyhow!("Persisted blueprint failed validation"))
            }
            (Some(blueprint), None) => Ok(blueprint),
            _ => Err(anyhow::anyhow!(
                "Persisted blueprint file must contain exactly one store"
            )),
        };

        match blueprint {
            Ok(blueprint) => Ok(Some(blueprint)),
            Err(err) => {
                if let Err(delete_err) = self.delete_persisted_blueprint(key) {
                    re_log::warn!("Failed to delete invalid persisted blueprint: {delete_err}");
                }
                Err(err)
            }
        }
    }

    /// Mark a loaded blueprint as already persisted.
    pub fn mark_blueprint_persisted(&mut self, store_id: &StoreId) {
        if let Some(blueprint) = self.store_bundle.get(store_id) {
            self.blueprints
                .mark_saved(store_id.clone(), blueprint.generation());
        }
    }

    /// Save a blueprint when its generation has changed since the last save.
    pub fn save_persisted_blueprint_if_changed(
        &mut self,
        key: &BlueprintPersistenceKey,
        store_id: &StoreId,
    ) -> anyhow::Result<()> {
        let Some(saver) = &self.blueprints.persistence().saver else {
            return Ok(());
        };
        let blueprint = self
            .store_bundle
            .get(store_id)
            .with_context(|| format!("Missing blueprint store: {store_id:?}"))?;
        if blueprint.store_kind() != StoreKind::Blueprint {
            anyhow::bail!("Store is not a blueprint: {store_id:?}");
        }
        if self.blueprints.is_saved(store_id, &blueprint.generation()) {
            return Ok(());
        }

        (saver)(key, blueprint)?;
        self.blueprints
            .mark_saved(store_id.clone(), blueprint.generation());
        Ok(())
    }

    /// Delete one persisted blueprint.
    pub fn delete_persisted_blueprint(&self, key: &BlueprintPersistenceKey) -> anyhow::Result<()> {
        let Some(deleter) = &self.blueprints.persistence().deleter else {
            return Ok(());
        };
        (deleter)(key)
    }

    /// Delete all persisted table blueprints, including blueprints for unopened tables.
    pub fn clear_persisted_table_blueprints(&self) -> anyhow::Result<()> {
        let Some(clearer) = &self.blueprints.persistence().table_blueprint_clearer else {
            return Ok(());
        };
        (clearer)()
    }

    /// Makes the blueprint from [`BlueprintPersistence::legacy_path_blueprint_loader`] the active
    /// blueprint of `app_id`, if there is one and it isn't loaded already.
    ///
    /// The blueprint gets the [`CachedBlueprintSource::Local`] cache entry, so the next save
    /// writes it there. A blueprint at a legacy path that fails to load or validate is deleted.
    fn try_load_legacy_path_blueprint(&mut self, app_id: &ApplicationId) -> anyhow::Result<()> {
        if self.blueprints.has_loaded_legacy_path_blueprint(app_id) {
            return Ok(());
        }
        let Some(loader) = &self.blueprints.persistence().legacy_path_blueprint_loader else {
            return Ok(());
        };
        let Some(bundle) = (loader)(app_id)? else {
            return Ok(());
        };
        self.blueprints
            .insert_loaded_legacy_path_blueprint(app_id.clone());

        let result = self.load_legacy_path_blueprint_bundle(bundle, app_id);
        if result.is_err()
            && let Err(err) = self.delete_legacy_path_blueprint(app_id)
        {
            re_log::warn!("Failed to delete invalid blueprint at legacy path: {err}");
        }
        result
    }

    /// Makes the blueprints in `legacy_bundle` active for `app_id`.
    ///
    /// A blueprint whose id passes [`StoreId::is_legacy_heuristic_blueprint`] is loaded with the
    /// id [`StoreId::heuristic_blueprint`].
    fn load_legacy_path_blueprint_bundle(
        &mut self,
        mut legacy_bundle: StoreBundle,
        app_id: &ApplicationId,
    ) -> anyhow::Result<()> {
        let mut bundle = StoreBundle::default();
        for blueprint in legacy_bundle.drain_entity_dbs() {
            let blueprint = if blueprint.store_id().is_legacy_heuristic_blueprint() {
                let heuristic_id = StoreId::heuristic_blueprint(blueprint.application_id().clone());
                blueprint.clone_with_new_id(heuristic_id)?
            } else {
                blueprint
            };
            bundle.insert(blueprint);
        }

        if let Some(validator) = &self.blueprints.persistence().validator
            && bundle.entity_dbs().any(|blueprint| !(validator)(blueprint))
        {
            anyhow::bail!("Blueprint at legacy path failed validation");
        }
        let blueprint_ids: Vec<StoreId> = bundle
            .entity_dbs()
            .map(|blueprint| blueprint.store_id().clone())
            .collect();
        self.load_blueprint_store(bundle, app_id)?;

        // The next save writes the blueprint to its cache entry, if it has one.
        for blueprint_id in &blueprint_ids {
            self.blueprints.clear_saved(blueprint_id);
        }
        Ok(())
    }

    /// Deletes the blueprint at the legacy path of `app_id`.
    pub(super) fn delete_legacy_path_blueprint(
        &self,
        app_id: &ApplicationId,
    ) -> anyhow::Result<()> {
        let Some(deleter) = &self.blueprints.persistence().legacy_path_blueprint_deleter else {
            return Ok(());
        };
        (deleter)(app_id)
    }

    /// Load a blueprint and make it active for the given `ApplicationId`.
    ///
    /// [`Self::save_app_blueprints`] saves the previous active blueprint to its cache entry, if it
    /// has one.
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
            let store_id = store.store_id().clone();
            self.blueprints
                .mark_saved(store_id.clone(), store.generation());
            self.store_bundle.insert(store);
            self.replace_active_blueprint(&store_id);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashSet;
    use std::sync::Arc;

    use ahash::HashMap;

    use crate::Route;
    use crate::store_hub::{add_dataset_blueprint, catalog_origin, catalog_source, dummy_chunk};

    fn recording_route(app_id: &ApplicationId) -> Route {
        Route::LocalRecording {
            recording_id: StoreId::random(StoreKind::Recording, app_id.clone()),
        }
    }

    /// Adds a blueprint with data to the hub, as a blueprint that didn't come from a catalog server.
    fn add_blueprint_with_data(hub: &mut StoreHub, app_id: &ApplicationId) -> StoreId {
        let blueprint_id = StoreId::random(StoreKind::Blueprint, app_id.clone());
        hub.store_bundle.blueprint_entry(&blueprint_id);
        hub.add_chunk_for_tests(&blueprint_id, &dummy_chunk())
            .unwrap();
        blueprint_id
    }

    /// Persisted blueprints of [`hub_with_persistence`], kept in memory.
    #[derive(Default)]
    struct PersistedBlueprints {
        blueprints: HashMap<BlueprintPersistenceKey, EntityDb>,

        /// Blueprints persisted by an earlier version of the viewer.
        legacy_path_blueprints: HashMap<ApplicationId, EntityDb>,
    }

    type Persisted = Arc<re_mutex::Mutex<PersistedBlueprints>>;

    /// The keys of the blueprints in `persisted`, without the ones at legacy paths.
    fn persisted_keys(persisted: &Persisted) -> HashSet<BlueprintPersistenceKey> {
        persisted.lock().blueprints.keys().cloned().collect()
    }

    /// Copies the store id and chunks of `blueprint`, the same as saving and loading it from disk.
    fn copy_blueprint(blueprint: &EntityDb) -> EntityDb {
        let mut copy = EntityDb::new(blueprint.store_id().clone());
        for chunk in blueprint.storage_engine().store().iter_physical_chunks() {
            copy.add_chunk(chunk).unwrap();
        }
        copy
    }

    fn bundle_of(blueprint: EntityDb) -> StoreBundle {
        let mut bundle = StoreBundle::default();
        bundle.insert(blueprint);
        bundle
    }

    /// A hub that persists its blueprints in `persisted`.
    ///
    /// Two hubs with the same `persisted` act like the viewer before and after a restart.
    fn hub_with_persistence(persisted: &Persisted) -> StoreHub {
        let loader_state = persisted.clone();
        let saver_state = persisted.clone();
        let deleter_state = persisted.clone();
        let clearer_state = persisted.clone();
        let legacy_path_loader_state = persisted.clone();
        let legacy_path_deleter_state = persisted.clone();

        let persistence = BlueprintPersistence {
            loader: Some(Box::new(move |key| {
                Ok(loader_state
                    .lock()
                    .blueprints
                    .get(key)
                    .map(|blueprint| bundle_of(copy_blueprint(blueprint))))
            })),
            saver: Some(Box::new(move |key, blueprint| {
                saver_state
                    .lock()
                    .blueprints
                    .insert(key.clone(), copy_blueprint(blueprint));
                Ok(())
            })),
            validator: None,
            deleter: Some(Box::new(move |key| {
                deleter_state.lock().blueprints.remove(key);
                Ok(())
            })),
            table_blueprint_clearer: None,
            app_blueprint_clearer: Some(Box::new(move |app_id| {
                clearer_state
                    .lock()
                    .blueprints
                    .retain(|key, _| key.application_id() != Some(app_id));
                Ok(())
            })),
            legacy_path_blueprint_loader: Some(Box::new(move |app_id| {
                let persisted = legacy_path_loader_state.lock();
                if persisted
                    .blueprints
                    .keys()
                    .any(|key| key.application_id() == Some(app_id))
                {
                    return Ok(None);
                }
                Ok(persisted
                    .legacy_path_blueprints
                    .get(app_id)
                    .map(|blueprint| bundle_of(copy_blueprint(blueprint))))
            })),
            legacy_path_blueprint_deleter: Some(Box::new(move |app_id| {
                legacy_path_deleter_state
                    .lock()
                    .legacy_path_blueprints
                    .remove(app_id);
                Ok(())
            })),
        };

        StoreHub::new(persistence, &|_| {})
    }

    /// Adds a chunk to the active blueprint of `app_id`, and returns its new latest row id.
    fn edit_active_blueprint(hub: &mut StoreHub, app_id: &ApplicationId) -> Option<RowId> {
        let active_id = hub.active_blueprint_id_for_app(app_id).unwrap().clone();
        hub.add_chunk_for_tests(&active_id, &dummy_chunk()).unwrap();
        hub.entity_db(&active_id).unwrap().latest_row_id()
    }

    fn active_latest_row_id(hub: &StoreHub, app_id: &ApplicationId) -> Option<RowId> {
        hub.active_blueprint_for_app(app_id)
            .unwrap()
            .latest_row_id()
    }

    fn dataset_key(app_id: &ApplicationId, blueprint_id: &str) -> BlueprintPersistenceKey {
        BlueprintPersistenceKey::recording(
            app_id.clone(),
            CachedBlueprintSource::Dataset(SegmentId::from(blueprint_id)),
        )
    }

    fn local_key(app_id: &ApplicationId) -> BlueprintPersistenceKey {
        BlueprintPersistenceKey::recording(app_id.clone(), CachedBlueprintSource::Local)
    }

    /// The user edits the copy of a dataset's default blueprint and restarts the viewer. Once the
    /// dataset blueprint is streamed again, its edited copy becomes active, and further edits are
    /// saved to the same cache entry.
    #[test]
    fn restart_loads_cached_copies() {
        let persisted = Persisted::default();
        let app_id = ApplicationId::from("test_app");

        let edited_row_id = {
            let mut hub = hub_with_persistence(&persisted);
            add_dataset_blueprint(&mut hub, &app_id, "a");
            hub.ensure_active_blueprint_for_app(&app_id);
            let edited_row_id = edit_active_blueprint(&mut hub, &app_id);
            hub.save_app_blueprints().unwrap();
            edited_row_id
        };

        let mut hub = hub_with_persistence(&persisted);
        hub.load_legacy_path_blueprint_for_app(&app_id);
        add_dataset_blueprint(&mut hub, &app_id, "a");
        hub.ensure_active_blueprint_for_app(&app_id);
        assert_eq!(
            active_latest_row_id(&hub, &app_id),
            edited_row_id,
            "the edited copy should be active"
        );

        let new_edit_row_id = edit_active_blueprint(&mut hub, &app_id);
        hub.save_app_blueprints().unwrap();
        assert_eq!(
            persisted_keys(&persisted),
            HashSet::from([dataset_key(&app_id, "a")]),
            "the save should write only the cache entry of the dataset blueprint"
        );
        assert_eq!(
            persisted.lock().blueprints[&dataset_key(&app_id, "a")].latest_row_id(),
            new_edit_row_id,
            "the cache entry should have the new edits"
        );
    }

    /// The user edits the copy of a dataset's default blueprint, and then the SDK makes another
    /// blueprint active. The edits are written to the dataset blueprint's cache entry on the next
    /// save, and come back when the user resets the SDK blueprint to the dataset's default.
    #[test]
    fn sdk_blueprint_keeps_dataset_edits_cached() {
        let persisted = Persisted::default();
        let mut hub = hub_with_persistence(&persisted);
        let app_id = ApplicationId::from("test_app");
        add_dataset_blueprint(&mut hub, &app_id, "a");
        let sdk_id = add_blueprint_with_data(&mut hub, &app_id);

        hub.ensure_active_blueprint_for_app(&app_id);
        let copy_id = hub.active_blueprint_id_for_app(&app_id).unwrap().clone();
        let edited_row_id = edit_active_blueprint(&mut hub, &app_id);

        hub.set_cloned_blueprint_active_for_app(&sdk_id).unwrap();
        hub.save_app_blueprints().unwrap();
        assert_eq!(
            persisted_keys(&persisted),
            HashSet::from([dataset_key(&app_id, "a"), local_key(&app_id)]),
            "the save should write the cache entry of the dataset blueprint and the local cache entry"
        );
        assert_eq!(
            persisted.lock().blueprints[&dataset_key(&app_id, "a")].latest_row_id(),
            edited_row_id,
            "the cache entry should have the edits"
        );
        assert!(
            hub.entity_db(&copy_id).is_none(),
            "the saved copy should be removed from the store bundle"
        );

        hub.clear_active_blueprint(&recording_route(&app_id));
        hub.ensure_active_blueprint_for_app(&app_id);
        assert_eq!(
            active_latest_row_id(&hub, &app_id),
            edited_row_id,
            "the reset should make the edited copy active"
        );
    }

    /// The user edits the copy of a dataset's default blueprint, and then resets it to the default.
    /// The cache entry is deleted on the next save, and the new active blueprint is a clone of the
    /// dataset blueprint without the edits.
    #[test]
    fn reset_drops_cache_entry() {
        let persisted = Persisted::default();
        let mut hub = hub_with_persistence(&persisted);
        let app_id = ApplicationId::from("test_app");
        let a_id = add_dataset_blueprint(&mut hub, &app_id, "a");

        hub.ensure_active_blueprint_for_app(&app_id);
        edit_active_blueprint(&mut hub, &app_id);
        hub.save_app_blueprints().unwrap();
        assert_eq!(
            persisted_keys(&persisted),
            HashSet::from([dataset_key(&app_id, "a")]),
            "the save should write the edited copy to its cache entry"
        );

        hub.clear_active_blueprint(&recording_route(&app_id));
        hub.ensure_active_blueprint_for_app(&app_id);
        hub.save_app_blueprints().unwrap();
        assert_eq!(
            persisted_keys(&persisted),
            HashSet::new(),
            "the save after the reset should delete the cache entry"
        );

        let active = hub.active_blueprint_for_app(&app_id).unwrap();
        assert_eq!(
            active.cloned_from(),
            Some(&a_id),
            "the active blueprint should be a clone of the dataset blueprint"
        );
        assert_eq!(
            active.latest_row_id(),
            hub.entity_db(&a_id).unwrap().latest_row_id(),
            "the clone should not have the edits"
        );
    }

    /// The user resets an app without a default blueprint to the heuristics and edits the
    /// heuristic blueprint. After a restart the edited heuristic blueprint is active, and a reset
    /// to the heuristics drops its cache entry.
    #[test]
    fn heuristic_blueprint_round_trip() {
        let persisted = Persisted::default();
        let app_id = ApplicationId::from("test_app");
        let route = recording_route(&app_id);
        let heuristic_id = StoreId::heuristic_blueprint(app_id.clone());

        let edited_row_id = {
            let mut hub = hub_with_persistence(&persisted);
            hub.clear_active_blueprint_and_generate(&route);
            hub.ensure_active_blueprint_for_app(&app_id);
            // `read_context` takes this flag on the next frame.
            hub.blueprints.take_should_enable_heuristics(&app_id);
            assert_eq!(
                hub.active_blueprint_id_for_app(&app_id),
                Some(&heuristic_id),
                "the reset should make the heuristic blueprint active"
            );
            let edited_row_id = edit_active_blueprint(&mut hub, &app_id);
            hub.save_app_blueprints().unwrap();
            assert_eq!(
                persisted_keys(&persisted),
                HashSet::from([local_key(&app_id)]),
                "the save should write only the local cache entry"
            );
            edited_row_id
        };

        let mut hub = hub_with_persistence(&persisted);
        hub.ensure_active_blueprint_for_app(&app_id);
        assert_eq!(
            hub.active_blueprint_id_for_app(&app_id),
            Some(&heuristic_id),
            "the heuristic blueprint should be active after the restart"
        );
        assert_eq!(
            active_latest_row_id(&hub, &app_id),
            edited_row_id,
            "the heuristic blueprint should have the edits from its cache entry"
        );

        hub.clear_active_blueprint_and_generate(&route);
        hub.save_app_blueprints().unwrap();
        assert_eq!(
            persisted_keys(&persisted),
            HashSet::new(),
            "the save should delete the local cache entry"
        );
    }

    /// The user edits the heuristic blueprint, and then the SDK makes another blueprint active.
    /// The SDK blueprint replaces the heuristic blueprint in the local cache entry, and is
    /// active after a restart. A reset to the heuristics drops the cache entry.
    #[test]
    fn sdk_blueprint_replaces_heuristic_blueprint() {
        let persisted = Persisted::default();
        let app_id = ApplicationId::from("test_app");
        let route = recording_route(&app_id);
        let heuristic_id = StoreId::heuristic_blueprint(app_id.clone());

        let (sdk_copy_id, sdk_row_id) = {
            let mut hub = hub_with_persistence(&persisted);
            hub.ensure_active_blueprint_for_app(&app_id);
            edit_active_blueprint(&mut hub, &app_id);

            let sdk_id = add_blueprint_with_data(&mut hub, &app_id);
            hub.set_cloned_blueprint_active_for_app(&sdk_id).unwrap();
            assert!(
                hub.entity_db(&heuristic_id).is_none(),
                "the heuristic blueprint should be removed from the store bundle"
            );
            let sdk_copy_id = hub.active_blueprint_id_for_app(&app_id).unwrap().clone();
            let sdk_row_id = active_latest_row_id(&hub, &app_id);

            hub.save_app_blueprints().unwrap();
            assert_eq!(
                persisted_keys(&persisted),
                HashSet::from([local_key(&app_id)]),
                "the save should write only the local cache entry"
            );
            assert_eq!(
                persisted.lock().blueprints[&local_key(&app_id)].latest_row_id(),
                sdk_row_id,
                "the local cache entry should have the SDK blueprint"
            );
            (sdk_copy_id, sdk_row_id)
        };

        let mut hub = hub_with_persistence(&persisted);
        hub.ensure_active_blueprint_for_app(&app_id);
        assert_eq!(
            hub.active_blueprint_id_for_app(&app_id),
            Some(&sdk_copy_id),
            "the SDK blueprint should be active after the restart"
        );
        assert_eq!(
            active_latest_row_id(&hub, &app_id),
            sdk_row_id,
            "the SDK blueprint should have its data from the cache entry"
        );

        hub.clear_active_blueprint_and_generate(&route);
        hub.save_app_blueprints().unwrap();
        assert_eq!(
            persisted_keys(&persisted),
            HashSet::new(),
            "the save should delete the local cache entry"
        );
    }

    /// The user edits a copy of the SDK's default blueprint and restarts the viewer. The edited
    /// copy is active after the restart, whether the SDK sends its default blueprint again before
    /// or after the viewer makes a blueprint active.
    #[test]
    fn sdk_default_does_not_replace_cached_blueprint() {
        let persisted = Persisted::default();
        let app_id = ApplicationId::from("test_app");

        let edited_row_id = {
            let mut hub = hub_with_persistence(&persisted);
            let sdk_id = add_blueprint_with_data(&mut hub, &app_id);
            hub.set_default_blueprint_for_app(&sdk_id).unwrap();
            hub.ensure_active_blueprint_for_app(&app_id);
            let edited_row_id = edit_active_blueprint(&mut hub, &app_id);
            hub.save_app_blueprints().unwrap();
            edited_row_id
        };

        for sdk_default_first in [true, false] {
            let mut hub = hub_with_persistence(&persisted);
            let sdk_id = add_blueprint_with_data(&mut hub, &app_id);
            if sdk_default_first {
                hub.set_default_blueprint_for_app(&sdk_id).unwrap();
                hub.ensure_active_blueprint_for_app(&app_id);
            } else {
                hub.ensure_active_blueprint_for_app(&app_id);
                hub.set_default_blueprint_for_app(&sdk_id).unwrap();
            }
            assert_eq!(
                active_latest_row_id(&hub, &app_id),
                edited_row_id,
                "the edited copy should be active, sdk_default_first: {sdk_default_first}"
            );
        }
    }

    /// The SDK makes a blueprint active and the user edits it. A save writes the edited blueprint
    /// to the local cache entry.
    #[test]
    fn sdk_blueprint_is_cached_as_local() {
        let persisted = Persisted::default();
        let mut hub = hub_with_persistence(&persisted);
        let app_id = ApplicationId::from("test_app");
        let sdk_id = add_blueprint_with_data(&mut hub, &app_id);

        hub.set_cloned_blueprint_active_for_app(&sdk_id).unwrap();
        let edited_row_id = edit_active_blueprint(&mut hub, &app_id);
        hub.save_app_blueprints().unwrap();
        assert_eq!(
            persisted_keys(&persisted),
            HashSet::from([local_key(&app_id)]),
            "the save should write only the local cache entry"
        );
        assert_eq!(
            persisted.lock().blueprints[&local_key(&app_id)].latest_row_id(),
            edited_row_id,
            "the local cache entry should have the edits"
        );
    }

    /// The user doesn't edit the copy of a dataset's default blueprint, and then the SDK makes
    /// another blueprint active. No save writes the unedited copy to its cache entry, and the save
    /// after the SDK change writes the SDK blueprint to the local cache entry.
    #[test]
    fn unedited_copy_is_not_cached() {
        let persisted = Persisted::default();
        let mut hub = hub_with_persistence(&persisted);
        let app_id = ApplicationId::from("test_app");
        add_dataset_blueprint(&mut hub, &app_id, "a");
        let sdk_id = add_blueprint_with_data(&mut hub, &app_id);

        hub.ensure_active_blueprint_for_app(&app_id);
        hub.save_app_blueprints().unwrap();
        assert_eq!(
            persisted_keys(&persisted),
            HashSet::new(),
            "the save should not persist the unedited copy"
        );

        hub.set_cloned_blueprint_active_for_app(&sdk_id).unwrap();
        hub.save_app_blueprints().unwrap();
        assert_eq!(
            persisted_keys(&persisted),
            HashSet::from([local_key(&app_id)]),
            "the save should write only the SDK blueprint to the local cache entry, not the unedited copy"
        );
    }

    fn blueprint_with_data(blueprint_id: StoreId) -> EntityDb {
        let mut blueprint = EntityDb::new(blueprint_id);
        blueprint.add_chunk(&dummy_chunk()).unwrap();
        blueprint
    }

    /// Persisted blueprints with a blueprint that an earlier version of the viewer persisted for
    /// `app_id`.
    fn persisted_with_legacy_path_blueprint(
        app_id: &ApplicationId,
        legacy_path_blueprint_id: &StoreId,
    ) -> Persisted {
        let persisted = Persisted::default();
        persisted.lock().legacy_path_blueprints.insert(
            app_id.clone(),
            blueprint_with_data(legacy_path_blueprint_id.clone()),
        );
        persisted
    }

    /// A blueprint at a legacy path loads once. After a reset it doesn't load again.
    #[test]
    fn legacy_path_blueprint_loads_once() {
        let app_id = ApplicationId::from("test_app");
        let legacy_path_blueprint_id = StoreId::random(StoreKind::Blueprint, app_id.clone());
        let persisted = persisted_with_legacy_path_blueprint(&app_id, &legacy_path_blueprint_id);

        let mut hub = hub_with_persistence(&persisted);
        hub.load_legacy_path_blueprint_for_app(&app_id);
        assert_eq!(
            hub.active_blueprint_id_for_app(&app_id),
            Some(&legacy_path_blueprint_id),
            "the blueprint at the legacy path should be active"
        );

        hub.clear_active_blueprint(&recording_route(&app_id));
        hub.load_legacy_path_blueprint_for_app(&app_id);
        assert_eq!(
            hub.active_blueprint_id_for_app(&app_id),
            None,
            "the blueprint at the legacy path should load only once"
        );
    }

    /// The user edits a blueprint with a random id from a legacy path, and then resets it to a
    /// dataset's default or to the heuristics. The new active blueprint isn't cached, and the
    /// blueprint at the legacy path doesn't load after a restart.
    #[test]
    fn reset_deletes_legacy_path_blueprint() {
        let app_id = ApplicationId::from("test_app");
        let route = recording_route(&app_id);
        let resets: [fn(&mut StoreHub, &Route); 2] = [
            StoreHub::clear_active_blueprint,
            StoreHub::clear_active_blueprint_and_generate,
        ];

        for reset in resets {
            let legacy_path_blueprint_id = StoreId::random(StoreKind::Blueprint, app_id.clone());
            let persisted =
                persisted_with_legacy_path_blueprint(&app_id, &legacy_path_blueprint_id);

            {
                let mut hub = hub_with_persistence(&persisted);
                add_dataset_blueprint(&mut hub, &app_id, "a");
                hub.load_legacy_path_blueprint_for_app(&app_id);
                assert_eq!(
                    hub.active_blueprint_id_for_app(&app_id),
                    Some(&legacy_path_blueprint_id),
                    "the blueprint at the legacy path should be active"
                );
                edit_active_blueprint(&mut hub, &app_id);

                reset(&mut hub, &route);
                hub.ensure_active_blueprint_for_app(&app_id);
                hub.save_app_blueprints().unwrap();
                assert_eq!(
                    persisted_keys(&persisted),
                    HashSet::new(),
                    "the save should not cache the new active blueprint"
                );
            }

            let mut hub = hub_with_persistence(&persisted);
            hub.load_legacy_path_blueprint_for_app(&app_id);
            assert_eq!(
                hub.active_blueprint_id_for_app(&app_id),
                None,
                "the blueprint at the legacy path should not load after the restart"
            );
        }
    }

    /// The SDK makes a blueprint active before the blueprint at the legacy path loads, and the
    /// user resets it. The blueprint at the legacy path loads after a restart.
    #[test]
    fn reset_keeps_unloaded_legacy_path_blueprint() {
        let app_id = ApplicationId::from("test_app");
        let legacy_path_blueprint_id = StoreId::random(StoreKind::Blueprint, app_id.clone());
        let persisted = persisted_with_legacy_path_blueprint(&app_id, &legacy_path_blueprint_id);

        {
            let mut hub = hub_with_persistence(&persisted);
            let sdk_id = add_blueprint_with_data(&mut hub, &app_id);
            hub.set_cloned_blueprint_active_for_app(&sdk_id).unwrap();
            hub.load_legacy_path_blueprint_for_app(&app_id);
            hub.clear_active_blueprint(&recording_route(&app_id));
        }

        let mut hub = hub_with_persistence(&persisted);
        hub.load_legacy_path_blueprint_for_app(&app_id);
        assert_eq!(
            hub.active_blueprint_id_for_app(&app_id),
            Some(&legacy_path_blueprint_id),
            "the blueprint at the legacy path should load after the restart"
        );
    }

    /// The user edits a blueprint from a legacy path, and a save writes it to the local cache
    /// entry. After a restart the cache entry is active, and the blueprint at the legacy path is
    /// unchanged. A blueprint with the heuristic blueprint id of earlier versions loads with the
    /// current heuristic blueprint id, and any other blueprint keeps its id.
    #[test]
    fn legacy_path_blueprint_moves_to_cache_entry() {
        let app_id = ApplicationId::from("test_app");
        let random_id = StoreId::random(StoreKind::Blueprint, app_id.clone());
        let legacy_heuristic_id =
            StoreId::new(StoreKind::Blueprint, app_id.clone(), app_id.as_str());

        for (legacy_path_blueprint_id, loaded_id) in [
            (random_id.clone(), random_id),
            (
                legacy_heuristic_id,
                StoreId::heuristic_blueprint(app_id.clone()),
            ),
        ] {
            let persisted =
                persisted_with_legacy_path_blueprint(&app_id, &legacy_path_blueprint_id);
            let legacy_path_row_id =
                persisted.lock().legacy_path_blueprints[&app_id].latest_row_id();

            let edited_row_id = {
                let mut hub = hub_with_persistence(&persisted);
                hub.load_legacy_path_blueprint_for_app(&app_id);
                assert_eq!(
                    hub.active_blueprint_id_for_app(&app_id),
                    Some(&loaded_id),
                    "the blueprint from the legacy path should be active"
                );
                let edited_row_id = edit_active_blueprint(&mut hub, &app_id);

                hub.save_app_blueprints().unwrap();
                assert_eq!(
                    persisted_keys(&persisted),
                    HashSet::from([local_key(&app_id)]),
                    "the save should write only the local cache entry"
                );
                edited_row_id
            };
            assert_eq!(
                persisted.lock().legacy_path_blueprints[&app_id].latest_row_id(),
                legacy_path_row_id,
                "the blueprint at the legacy path should be unchanged"
            );

            let mut hub = hub_with_persistence(&persisted);
            hub.load_legacy_path_blueprint_for_app(&app_id);
            hub.ensure_active_blueprint_for_app(&app_id);
            assert_eq!(
                hub.active_blueprint_id_for_app(&app_id),
                Some(&loaded_id),
                "the local cache entry should be active after the restart"
            );
            assert_eq!(
                active_latest_row_id(&hub, &app_id),
                edited_row_id,
                "the local cache entry should have the edits"
            );
        }
    }

    /// A dataset blueprint has the app id as segment id. After a reset to the heuristics, the
    /// active blueprint is a new heuristic blueprint, and edits to it don't change the dataset
    /// blueprint.
    #[test]
    fn dataset_blueprint_named_like_its_app() {
        let persisted = Persisted::default();
        let mut hub = hub_with_persistence(&persisted);
        let app_id = ApplicationId::from("test_app");
        let route = recording_route(&app_id);
        let dataset_id = add_dataset_blueprint(&mut hub, &app_id, "test_app");
        let dataset_row_id = hub.entity_db(&dataset_id).unwrap().latest_row_id();
        hub.ensure_active_blueprint_for_app(&app_id);

        hub.clear_active_blueprint_and_generate(&route);
        hub.ensure_active_blueprint_for_app(&app_id);
        hub.blueprints.take_should_enable_heuristics(&app_id);
        assert_eq!(
            hub.active_blueprint_id_for_app(&app_id),
            Some(&StoreId::heuristic_blueprint(app_id.clone())),
            "the reset should make the heuristic blueprint active"
        );

        edit_active_blueprint(&mut hub, &app_id);
        assert_eq!(
            hub.entity_db(&dataset_id).unwrap().latest_row_id(),
            dataset_row_id,
            "the edit should not change the dataset blueprint"
        );
        assert_eq!(
            hub.default_blueprint_id_for_app(&app_id),
            Some(&dataset_id),
            "the dataset blueprint should stay the default"
        );
    }

    /// A file at a legacy path that holds a recording fails to load and is deleted.
    #[test]
    fn invalid_legacy_path_blueprint_is_deleted() {
        let app_id = ApplicationId::from("test_app");
        let recording_id = StoreId::random(StoreKind::Recording, app_id.clone());
        let persisted = persisted_with_legacy_path_blueprint(&app_id, &recording_id);

        let mut hub = hub_with_persistence(&persisted);
        hub.load_legacy_path_blueprint_for_app(&app_id);
        assert_eq!(
            hub.active_blueprint_id_for_app(&app_id),
            None,
            "the invalid blueprint at the legacy path should not be active"
        );
        assert!(
            !persisted
                .lock()
                .legacy_path_blueprints
                .contains_key(&app_id),
            "loading should delete the invalid blueprint at the legacy path"
        );
    }

    /// Removing the catalog server of an app's recording deletes the blueprint at the app's legacy
    /// path.
    #[test]
    fn removing_server_deletes_legacy_path_blueprint() {
        let app_id = ApplicationId::from("test_app");
        let legacy_path_blueprint_id = StoreId::random(StoreKind::Blueprint, app_id.clone());
        let persisted = persisted_with_legacy_path_blueprint(&app_id, &legacy_path_blueprint_id);

        let mut hub = hub_with_persistence(&persisted);
        let recording_id = StoreId::random(StoreKind::Recording, app_id.clone());
        hub.store_bundle
            .entry(&recording_id)
            .add_data_source(&catalog_source());

        hub.clear_blueprints_for_origin(&catalog_origin());
        assert!(
            !persisted
                .lock()
                .legacy_path_blueprints
                .contains_key(&app_id),
            "removing the server should delete the blueprint at the legacy path"
        );
    }

    /// Without persisted storage, the user edits the copy of a dataset's default blueprint, and
    /// then switches to an SDK blueprint and resets back to the dataset's default twice. The
    /// edited copy stays in the store bundle through each save, and each reset makes it active.
    #[test]
    fn edits_stay_in_store_bundle_without_saver() {
        let mut hub = StoreHub::test_hub();
        let app_id = ApplicationId::from("test_app");
        add_dataset_blueprint(&mut hub, &app_id, "a");
        let sdk_id = add_blueprint_with_data(&mut hub, &app_id);

        hub.ensure_active_blueprint_for_app(&app_id);
        let copy_id = hub.active_blueprint_id_for_app(&app_id).unwrap().clone();
        let edited_row_id = edit_active_blueprint(&mut hub, &app_id);

        for _ in 0..2 {
            hub.set_cloned_blueprint_active_for_app(&sdk_id).unwrap();
            hub.save_app_blueprints().unwrap();
            assert!(
                hub.entity_db(&copy_id).is_some(),
                "the edited copy should stay in the store bundle without a saver"
            );

            hub.clear_active_blueprint(&recording_route(&app_id));
            hub.ensure_active_blueprint_for_app(&app_id);
            assert_eq!(
                hub.active_blueprint_id_for_app(&app_id),
                Some(&copy_id),
                "the reset should make the edited copy active"
            );
            assert_eq!(
                active_latest_row_id(&hub, &app_id),
                edited_row_id,
                "the edited copy should have the edits"
            );
        }
    }
}
