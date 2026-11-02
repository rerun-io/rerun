//! The default and active blueprint of each app, and clearing and garbage collecting blueprints.

use std::collections::BTreeSet;

use ahash::{HashMap, HashSet};
use anyhow::Context as _;
use nohash_hasher::IntMap;
use re_chunk_store::{GarbageCollectionOptions, GarbageCollectionTarget};
use re_entity_db::EntityDb;
use re_log_types::{AbsoluteTimeRange, ApplicationId, StoreId, StoreKind};

use super::{BlueprintPersistenceKey, StoreHub};
use crate::{BlueprintUndoState, Route};

impl StoreHub {
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
        let app_id = blueprint_id.application_id().clone();
        self.default_blueprint_by_app_id
            .insert(app_id.clone(), blueprint_id.clone());

        // If the active blueprint for this app was auto-created as empty (i.e. no
        // chunks have ever been written to it), the user hasn't touched it yet,
        // so it's safe to replace it with a clone of the newly-registered default.
        // This handles the race where the user opens a recording (which creates
        // an empty active blueprint via `ensure_active_blueprint_for_app`) before
        // the dataset's default blueprint has finished streaming.
        if let Some(active_id) = self.active_blueprint_by_app_id.get(&app_id)
            && let Some(active_blueprint) = self.store_bundle.get(active_id)
            && active_blueprint.latest_row_id().is_none()
        {
            let blueprint_id = blueprint_id.clone();
            if let Err(err) = self.set_cloned_blueprint_active_for_app(&blueprint_id) {
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
    /// Any previously-active blueprint for this app is dropped from the store bundle
    /// so it doesn't linger as an orphan.
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

        let previous_active_id = self.active_blueprint_by_app_id.get(&app_id).cloned();

        self.store_bundle.insert(new_blueprint);

        self.active_blueprint_by_app_id.insert(app_id, new_id);

        // Drop the previous active store now that nothing references it. Skip if it
        // happens to be the source we just cloned from (i.e. caller passed the
        // already-active id), since `remove_store` would otherwise discard it.
        if let Some(prev_id) = previous_active_id
            && &prev_id != blueprint_id
        {
            self.remove_store(&prev_id);
        }

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
            .recordings_for_origin(origin)
            .map(|db| db.application_id().clone())
            .collect();

        for app_id in &affected_app_ids {
            self.clear_active_blueprint_for_app_id(app_id);
        }

        for app_id in &affected_app_ids {
            if let Err(err) =
                self.delete_persisted_blueprint(&BlueprintPersistenceKey::Recording(app_id.clone()))
            {
                re_log::warn!("Failed to delete persisted blueprint for {app_id}: {err}");
            }
        }
    }

    /// Cloned blueprints are the ones the user has edited,
    /// i.e. NOT sent from the SDK.
    pub fn clear_all_cloned_blueprints(&mut self) {
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

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    use re_chunk::Chunk;
    use re_log_types::TimePoint;
    use re_sdk_types::archetypes::Points2D;

    fn dummy_chunk() -> Arc<Chunk> {
        Arc::new(
            Chunk::builder("foo")
                .with_archetype(
                    re_chunk::RowId::new(),
                    TimePoint::STATIC,
                    &Points2D::new([(0.0_f32, 0.0_f32)]),
                )
                .build()
                .expect("chunk should build"),
        )
    }

    /// When the active blueprint for an app is auto-created empty and then a
    /// default blueprint is registered, the active should be replaced by a clone
    /// of the default. (Regression for the OSS server / `segment_table` flow,
    /// rerun#12773.)
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
        assert!(
            hub.store_bundle.get(&original_active_id).is_none(),
            "previous empty active blueprint should be dropped from the bundle"
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
        assert!(
            hub.store_bundle
                .get(&active_id)
                .unwrap()
                .latest_row_id()
                .is_some()
        );

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
