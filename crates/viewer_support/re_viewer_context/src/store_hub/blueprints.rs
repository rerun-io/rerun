//! The default and active blueprint of each app, and clearing and garbage collecting blueprints.

use std::collections::BTreeSet;

use ahash::{HashMap, HashSet};
use anyhow::Context as _;
use nohash_hasher::IntMap;
use re_chunk_store::{GarbageCollectionOptions, GarbageCollectionTarget};
use re_entity_db::EntityDb;
use re_log_channel::LogSource;
use re_log_types::{AbsoluteTimeRange, ApplicationId, StoreId, StoreKind};
use re_types_core::SegmentId;

use super::{BlueprintPersistenceKey, CachedBlueprintSource, DatasetBlueprintCopy, StoreHub};
use crate::{BlueprintUndoState, Route};

impl StoreHub {
    pub fn default_blueprint_id_for_app(&self, app_id: &ApplicationId) -> Option<&StoreId> {
        self.blueprints.default_blueprint_id(app_id)
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
        if let Some(validator) = &self.blueprints.persistence().validator
            && !(validator)(blueprint)
        {
            anyhow::bail!("Blueprint failed validation");
        }

        re_log::trace!(
            "Switching default blueprint for '{:?}' to '{:?}'",
            blueprint_id.application_id(),
            blueprint_id
        );
        let app_id = blueprint_id.application_id().clone();
        self.blueprints
            .set_default_blueprint(app_id.clone(), blueprint_id.clone());

        // A copy of the new default replaces the active blueprint of this app if the active one
        // isn't edited, see `StoreHubBlueprints::is_edited`. That covers an empty blueprint from
        // `ensure_active_blueprint_for_app` when a recording opens before the dataset's default
        // blueprint has finished streaming, and a copy of a dataset blueprint that nothing has
        // written to, which may be an earlier stream of `blueprint_id`.
        if let Some(active_id) = self.blueprints.active_blueprint_id(&app_id)
            && let Some(active_blueprint) = self.store_bundle.get(active_id)
            && !self.blueprints.is_edited(active_blueprint)
        {
            let blueprint_id = blueprint_id.clone();
            if let Err(err) = self.activate_blueprint(&blueprint_id) {
                re_log::warn!(
                    "Failed to promote new default blueprint to active for '{app_id}': {err}"
                );
            }
        }

        Ok(())
    }

    /// Ensure there is an active blueprint for the given app.
    ///
    /// First cleans up stale references, then:
    /// - If there is already an active blueprint, does nothing.
    /// - If heuristics are about to be enabled, creates a new empty blueprint.
    /// - If the default blueprint is a dataset blueprint, makes its cached copy active if there
    ///   is one, and otherwise clones it and makes the clone active.
    /// - Otherwise, makes the blueprint with the [`CachedBlueprintSource::Local`] cache entry
    ///   active if there is one. A later default blueprint from the SDK or from a file doesn't
    ///   replace it, since the cached blueprint is edited.
    /// - Otherwise, clones the default blueprint if there is one and makes the clone active, and
    ///   otherwise creates an empty blueprint and registers it as active.
    pub fn ensure_active_blueprint_for_app(&mut self, app_id: &ApplicationId) {
        // Clean up stale references in case our book-keeping is broken.
        let store_bundle = &self.store_bundle;
        self.blueprints
            .remove_missing_blueprints(app_id, |blueprint_id| store_bundle.contains(blueprint_id));

        if self.blueprints.active_blueprint_id(app_id).is_some() {
            return;
        }

        // If heuristics are about to run, create a new empty blueprint to write into.
        if self.blueprints.should_enable_heuristics(app_id) {
            let blueprint_id = StoreId::heuristic_blueprint(app_id.clone());
            self.store_bundle.blueprint_entry(&blueprint_id);
            self.replace_active_blueprint(&blueprint_id);
            return;
        }

        let default_id = self.blueprints.default_blueprint_id(app_id).cloned();
        let default_is_dataset_blueprint = default_id
            .as_ref()
            .is_some_and(|default_id| self.dataset_blueprint_segment(default_id).is_some());
        if !default_is_dataset_blueprint
            && self.try_activate_cached_blueprint(app_id, &CachedBlueprintSource::Local)
        {
            return;
        }

        // Try to clone the default blueprint.
        if let Some(blueprint_id) = default_id {
            self.activate_blueprint(&blueprint_id)
                .unwrap_or_else(|err| {
                    re_log::warn!("Failed to make blueprint active: {err}");
                });
            return;
        }

        let blueprint_id = StoreId::heuristic_blueprint(app_id.clone());
        self.store_bundle.blueprint_entry(&blueprint_id);
        self.replace_active_blueprint(&blueprint_id);
    }

    pub fn active_blueprint_id_for_app(&self, app_id: &ApplicationId) -> Option<&StoreId> {
        self.blueprints.active_blueprint_id(app_id)
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
    /// Clones `blueprint_id` and makes the clone active, so edits never change `blueprint_id`.
    ///
    /// If `blueprint_id` is a dataset blueprint, the clone uses the cache entry of `blueprint_id`.
    /// Otherwise the clone takes the [`CachedBlueprintSource::Local`] cache entry of the app.
    /// Once the clone is edited, it is saved to its cache entry.
    ///
    /// If the previous active blueprint of the app still has a cache entry, it stays in the store
    /// bundle until [`Self::save_app_blueprints`] saves it. Otherwise it is removed from the store
    /// bundle right away, even with unsaved edits.
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
        if let Some(validator) = &self.blueprints.persistence().validator
            && !(validator)(blueprint)
        {
            anyhow::bail!("Blueprint failed validation");
        }

        let new_blueprint = blueprint.clone_with_new_id(new_id.clone())?;

        if let Some(segment) = self.dataset_blueprint_segment(blueprint_id) {
            self.blueprints.insert_dataset_blueprint_copy(
                new_id.clone(),
                DatasetBlueprintCopy {
                    blueprint_id: segment,
                    latest_row_id_at_clone: new_blueprint.latest_row_id(),
                },
            );
        }
        self.store_bundle.insert(new_blueprint);

        self.replace_active_blueprint(&new_id);

        Ok(())
    }

    /// Makes `blueprint_id` the active blueprint of its app.
    ///
    /// Expects `blueprint_id` to be in the store bundle.
    ///
    /// If `blueprint_id` is a copy of a dataset blueprint, any other copy of the same dataset
    /// blueprint is removed from the store bundle. Otherwise `blueprint_id` takes the
    /// [`CachedBlueprintSource::Local`] cache entry of the app, and the blueprint that had it
    /// is removed from the store bundle. In both cases the default blueprint of the app and the
    /// blueprint that `blueprint_id` is a clone of are not removed.
    ///
    /// The previous active blueprint stays in the store bundle if `blueprint_id` is a clone of it,
    /// or if it still has a cache entry and isn't saved yet. Otherwise it is removed right away,
    /// even with unsaved edits.
    pub(super) fn replace_active_blueprint(&mut self, blueprint_id: &StoreId) {
        let app_id = blueprint_id.application_id().clone();
        let cloned_from = self
            .store_bundle
            .get(blueprint_id)
            .and_then(|blueprint| blueprint.cloned_from())
            .cloned();

        let replaced_ids: Vec<StoreId> = match self.blueprints.cached_blueprint_source(blueprint_id)
        {
            Some(CachedBlueprintSource::Dataset(segment)) => self
                .blueprints
                .dataset_blueprint_copies(&app_id, &segment)
                .cloned()
                .collect(),
            Some(CachedBlueprintSource::Local) | None => self
                .blueprints
                .set_local_entry_blueprint(app_id.clone(), blueprint_id.clone())
                .into_iter()
                .collect(),
        };
        for replaced_id in replaced_ids {
            let is_source = Some(&replaced_id) == cloned_from.as_ref()
                || self.blueprints.default_blueprint_id(&app_id) == Some(&replaced_id);
            if replaced_id != *blueprint_id && !is_source {
                self.remove_store(&replaced_id);
            }
        }

        let Some(previous_id) = self
            .blueprints
            .set_active_blueprint(app_id, blueprint_id.clone())
        else {
            return;
        };
        if previous_id == *blueprint_id || Some(&previous_id) == cloned_from.as_ref() {
            return;
        }

        if self
            .blueprints
            .cached_blueprint_source(&previous_id)
            .is_some()
        {
            self.remove_store_if_saved(&previous_id);
        } else {
            self.remove_store(&previous_id);
        }
    }

    /// Makes a copy of `source_id` the active blueprint of its app.
    ///
    /// If `source_id` is a dataset blueprint with a cached copy, that cached copy becomes active.
    /// Otherwise a new clone of `source_id` becomes active.
    fn activate_blueprint(&mut self, source_id: &StoreId) -> anyhow::Result<()> {
        if let Some(segment) = self.dataset_blueprint_segment(source_id)
            && self.try_activate_cached_blueprint(
                source_id.application_id(),
                &CachedBlueprintSource::Dataset(segment),
            )
        {
            return Ok(());
        }

        self.set_cloned_blueprint_active_for_app(source_id)
    }

    /// The segment of `blueprint_id`, if it is a dataset blueprint.
    ///
    /// A dataset blueprint is a blueprint store streamed from a catalog server, and has its
    /// segment id as recording id.
    fn dataset_blueprint_segment(&self, blueprint_id: &StoreId) -> Option<SegmentId> {
        let blueprint = self.store_bundle.get(blueprint_id)?;
        let is_from_catalog = blueprint.store_kind() == StoreKind::Blueprint
            && blueprint
                .data_sources()
                .iter()
                .any(|source| matches!(source.as_ref(), LogSource::RedapGrpcStream { .. }));
        is_from_catalog.then(|| SegmentId::from(blueprint_id.recording_id()))
    }

    /// Is the given blueprint id the active blueprint for any app id?
    pub fn is_active_blueprint(&self, blueprint_id: &StoreId) -> bool {
        self.blueprints.is_active_blueprint(blueprint_id)
    }

    /// Clear the currently active blueprint for the given app, and delete its cache entry.
    ///
    /// If the blueprint at the legacy path of the app is loaded, it is deleted too.
    fn clear_active_blueprint_for_app_id(&mut self, app_id: &ApplicationId) {
        if let Some(blueprint_id) = self.blueprints.take_active_blueprint(app_id) {
            re_log::debug!("Clearing blueprint for {app_id}: {blueprint_id:?}");
            self.delete_cache_entry(&blueprint_id);
            self.remove_store(&blueprint_id);
        }
        if self.blueprints.has_loaded_legacy_path_blueprint(app_id)
            && let Err(err) = self.delete_legacy_path_blueprint(app_id)
        {
            re_log::warn!("Failed to delete blueprint at legacy path for {app_id}: {err}");
        }
    }

    /// Clear the currently active blueprint, and delete its cache entry.
    ///
    /// If the blueprint at the legacy path of the app is loaded, it is deleted too.
    pub fn clear_active_blueprint(&mut self, route: &Route) {
        if let Some(app_id) = route.app_id() {
            self.clear_active_blueprint_for_app_id(app_id);
        }
    }

    /// Clear the currently active blueprint and enable the heuristics to generate a new one.
    ///
    /// This keeps the default blueprint as is, so the user may reset to the default blueprint
    /// afterward. The cache entry of the active blueprint and the
    /// [`CachedBlueprintSource::Local`] cache entry are deleted. No blueprint has the
    /// local cache entry afterward, and the one that had it is removed unless it is the
    /// default blueprint. If the blueprint at the legacy path of the app is loaded, it is deleted
    /// too.
    pub fn clear_active_blueprint_and_generate(&mut self, route: &Route) {
        if let Some(app_id) = route.app_id() {
            self.clear_active_blueprint_for_app_id(app_id);
            if let Some(blueprint_id) = self.blueprints.take_local_entry_blueprint(app_id)
                && self.blueprints.default_blueprint_id(app_id) != Some(&blueprint_id)
            {
                self.remove_store(&blueprint_id);
            }
            let key =
                BlueprintPersistenceKey::recording(app_id.clone(), CachedBlueprintSource::Local);
            if let Err(err) = self.delete_persisted_blueprint(&key) {
                re_log::warn!("Failed to delete cached blueprint: {err}");
            }

            // Simply clearing the default blueprint would trigger a reset to default. Instead, we must
            // actively set the blueprint to use the heuristics, so we store a flag so we do that early
            // next frame.
            self.blueprints.set_should_enable_heuristics(app_id.clone());
        }
    }

    /// Clear active blueprints (in-memory and on disk) for all `app_ids` that have
    /// recordings originating from the given server.
    pub fn clear_blueprints_for_origin(&mut self, origin: &re_uri::Origin) {
        let affected_app_ids: BTreeSet<ApplicationId> = self
            .store_bundle
            .recordings_for_origin(origin)
            .map(|db| db.application_id().clone())
            .collect();

        for app_id in &affected_app_ids {
            self.clear_active_blueprint_for_app_id(app_id);
        }
        for blueprint_id in self.cached_blueprint_ids() {
            if affected_app_ids.contains(blueprint_id.application_id()) {
                self.remove_store(&blueprint_id);
            }
        }

        for app_id in &affected_app_ids {
            if let Some(clearer) = &self.blueprints.persistence().app_blueprint_clearer
                && let Err(err) = (clearer)(app_id)
            {
                re_log::warn!("Failed to delete persisted blueprints for {app_id}: {err}");
            }
            if let Err(err) = self.delete_legacy_path_blueprint(app_id) {
                re_log::warn!("Failed to delete blueprint at legacy path for {app_id}: {err}");
            }
        }
    }

    /// Cloned blueprints are the ones the user has edited,
    /// i.e. NOT sent from the SDK.
    ///
    /// Also removes the inactive blueprints that have a cache entry.
    pub fn clear_all_cloned_blueprints(&mut self) {
        for blueprint_id in self.cached_blueprint_ids() {
            if !self.blueprints.is_active_blueprint(&blueprint_id) {
                self.remove_store(&blueprint_id);
            }
        }
        self.retain_recordings(|db| match db.store_kind() {
            StoreKind::Recording => true,
            StoreKind::Blueprint => db.cloned_from().is_none(),
        });
    }

    /// Drops all but the latest data for every blueprint.
    /// Preserves usable undo points in the process.
    pub fn gc_blueprints(&mut self, undo_state: &HashMap<StoreId, BlueprintUndoState>) {
        re_tracing::profile_function!();

        for blueprint in self.store_bundle.blueprints_mut() {
            let blueprint_id = blueprint.store_id();
            if self
                .blueprints
                .is_garbage_collected(blueprint_id, &blueprint.generation())
            {
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

            self.blueprints
                .mark_garbage_collected(blueprint_id.clone(), blueprint.generation());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::store_hub::{add_dataset_blueprint, dummy_chunk};

    /// When the active blueprint for an app is auto-created empty and then a
    /// default blueprint is registered, the active should be replaced by a clone
    /// of the default. The empty blueprint is dropped on the next save.
    /// (Regression for the OSS server / `segment_table` flow, rerun#12773.)
    #[test]
    fn registering_default_replaces_empty_active_blueprint() {
        let mut hub = StoreHub::test_hub();
        let app_id = ApplicationId::from("test_app");

        // Simulate the race: the user opens the route before the dataset
        // blueprint has landed, so an empty active blueprint is created first.
        hub.ensure_active_blueprint_for_app(&app_id);
        let original_active_id = hub
            .active_blueprint_id_for_app(&app_id)
            .expect("active blueprint should exist")
            .clone();
        assert!(
            hub.store_bundle
                .get(&original_active_id)
                .unwrap()
                .latest_row_id()
                .is_none(),
            "active blueprint should be empty"
        );

        // Now the dataset's default blueprint finishes streaming and registers.
        let default_id = StoreId::random(StoreKind::Blueprint, app_id.clone());
        hub.store_bundle.blueprint_entry(&default_id);
        hub.add_chunk_for_tests(&default_id, &dummy_chunk())
            .unwrap();
        hub.set_default_blueprint_for_app(&default_id).unwrap();

        let new_active_id = hub
            .active_blueprint_id_for_app(&app_id)
            .expect("active blueprint should still exist")
            .clone();
        assert_ne!(
            new_active_id, original_active_id,
            "empty active blueprint should have been replaced"
        );
        assert_eq!(
            hub.store_bundle.get(&new_active_id).unwrap().cloned_from(),
            Some(&default_id),
            "new active should be a clone of the newly-registered default"
        );

        hub.save_app_blueprints().unwrap();
        assert!(
            hub.store_bundle.get(&original_active_id).is_none(),
            "previous empty active blueprint should be dropped from the bundle on the next save"
        );
    }

    /// The active blueprint is an unedited copy of a dataset's default blueprint, and then
    /// another default blueprint of the dataset is registered. A copy of the registered default
    /// becomes active, and the next save removes the replaced copy from the store bundle.
    #[test]
    fn registering_default_replaces_unedited_dataset_copy() {
        let mut hub = StoreHub::test_hub();
        let app_id = ApplicationId::from("test_app");
        add_dataset_blueprint(&mut hub, &app_id, "a");
        hub.ensure_active_blueprint_for_app(&app_id);
        let replaced_copy_id = hub.active_blueprint_id_for_app(&app_id).unwrap().clone();

        let b_id = add_dataset_blueprint(&mut hub, &app_id, "b");
        let active = hub.active_blueprint_for_app(&app_id).unwrap();
        assert_eq!(
            active.cloned_from(),
            Some(&b_id),
            "the active blueprint should be a copy of the new default"
        );

        hub.save_app_blueprints().unwrap();
        assert!(
            hub.entity_db(&replaced_copy_id).is_none(),
            "the save should remove the replaced copy from the store bundle"
        );
        assert_eq!(
            hub.blueprints
                .dataset_blueprint_copies(&app_id, &SegmentId::from("a"))
                .count(),
            0,
            "no copy of the first default should be left"
        );
    }

    /// The active blueprint is an unedited copy of a dataset's default blueprint, and then the
    /// same default blueprint is streamed again with new data. A copy with the new data becomes
    /// active, and it is the only copy of that default.
    #[test]
    fn streaming_default_again_replaces_unedited_dataset_copy() {
        let mut hub = StoreHub::test_hub();
        let app_id = ApplicationId::from("test_app");
        let a_id = add_dataset_blueprint(&mut hub, &app_id, "a");
        hub.ensure_active_blueprint_for_app(&app_id);
        let replaced_copy_id = hub.active_blueprint_id_for_app(&app_id).unwrap().clone();

        hub.clear_store_data(&a_id);
        add_dataset_blueprint(&mut hub, &app_id, "a");
        let new_copy_id = hub.active_blueprint_id_for_app(&app_id).unwrap().clone();
        assert_eq!(
            hub.entity_db(&new_copy_id).unwrap().latest_row_id(),
            hub.entity_db(&a_id).unwrap().latest_row_id(),
            "the active blueprint should have the new data of the default"
        );
        assert!(
            hub.entity_db(&replaced_copy_id).is_none(),
            "the replaced copy should be removed from the store bundle"
        );
        assert_eq!(
            hub.blueprints
                .dataset_blueprint_copies(&app_id, &SegmentId::from("a"))
                .collect::<Vec<_>>(),
            vec![&new_copy_id],
            "the active blueprint should be the only copy of the default"
        );
    }

    /// The user edits the copy of a dataset's default blueprint, and then another default
    /// blueprint of the dataset is registered. The edited copy stays active.
    #[test]
    fn registering_default_preserves_edited_dataset_copy() {
        let mut hub = StoreHub::test_hub();
        let app_id = ApplicationId::from("test_app");
        add_dataset_blueprint(&mut hub, &app_id, "a");
        hub.ensure_active_blueprint_for_app(&app_id);
        let copy_id = hub.active_blueprint_id_for_app(&app_id).unwrap().clone();
        hub.add_chunk_for_tests(&copy_id, &dummy_chunk()).unwrap();

        add_dataset_blueprint(&mut hub, &app_id, "b");
        assert_eq!(
            hub.active_blueprint_id_for_app(&app_id),
            Some(&copy_id),
            "the edited copy should stay active"
        );
    }

    /// If the active blueprint has any user-written content, registering a new
    /// default must not clobber it.
    #[test]
    fn registering_default_preserves_modified_active_blueprint() {
        let mut hub = StoreHub::test_hub();
        let app_id = ApplicationId::from("test_app");

        hub.ensure_active_blueprint_for_app(&app_id);
        let active_id = hub
            .active_blueprint_id_for_app(&app_id)
            .expect("active blueprint should exist")
            .clone();

        // User has touched the active blueprint.
        hub.add_chunk_for_tests(&active_id, &dummy_chunk()).unwrap();

        // A default blueprint arrives.
        let default_id = StoreId::random(StoreKind::Blueprint, app_id.clone());
        hub.store_bundle.blueprint_entry(&default_id);
        hub.add_chunk_for_tests(&default_id, &dummy_chunk())
            .unwrap();
        hub.set_default_blueprint_for_app(&default_id).unwrap();

        assert_eq!(
            hub.active_blueprint_id_for_app(&app_id),
            Some(&active_id),
            "modified active blueprint should NOT be replaced"
        );
    }
}
