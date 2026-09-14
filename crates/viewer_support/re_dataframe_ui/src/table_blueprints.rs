use std::sync::LazyLock;

use ahash::HashMap;
use re_entity_db::{EntityDb, StoreBundle};
use re_log_types::{StoreId, StoreKind};
use re_viewer_context::store_hub::{BlueprintPersistenceKey, StoreHub};
use re_viewer_context::{AppBlueprintCtx, AppContext, TableReference};

#[derive(Debug, thiserror::Error)]
pub enum TableBlueprintError {
    #[error("missing table blueprint store: {0:?}")]
    MissingStore(StoreId),

    #[error("table blueprint store is not of kind blueprint")]
    NotBlueprint,

    #[error(transparent)]
    EntityDb(#[from] re_entity_db::Error),
}

/// Associations between table identities and their registered and editable blueprints.
///
/// The registered blueprint is never used as the target of a table UI edit.
/// An active blueprint is cloned from it before it is exposed to the table widget.
#[derive(Default)]
pub struct TableBlueprints {
    default_by_table_ref: HashMap<TableReference, StoreId>,
    active_by_table_ref: HashMap<TableReference, StoreId>,

    /// Latest row ID of each active blueprint when it was cloned from its default.
    ///
    /// A different latest row ID means the active blueprint has been edited and must not be
    /// replaced when a new default arrives.
    active_baseline: HashMap<TableReference, Option<re_chunk_store::external::re_chunk::RowId>>,
}

impl TableBlueprints {
    /// Register a table's default blueprint and update its active clone when it has not changed.
    pub fn set_default_blueprint(
        &mut self,
        table_ref: &TableReference,
        store_id: &StoreId,
        store_hub: &mut StoreHub,
    ) -> Result<(), TableBlueprintError> {
        self.ensure_active_blueprint(table_ref, store_hub)?;

        let stores = store_hub.store_bundle_mut();
        let store = stores
            .get(store_id)
            .ok_or_else(|| TableBlueprintError::MissingStore(store_id.clone()))?;
        if store.store_kind() != StoreKind::Blueprint {
            return Err(TableBlueprintError::NotBlueprint);
        }

        // Set the new default and remove the old one if it is different.
        if let Some(old_default) = self
            .default_by_table_ref
            .insert(table_ref.clone(), store_id.clone())
            && old_default != *store_id
        {
            self.remove_if_orphaned(stores, &old_default);
        }

        // Replace the currently active blueprint unless we already have edits.
        let replace_active = match self.active_by_table_ref.get(table_ref) {
            Some(active_id) => !self.active_is_modified(table_ref, active_id, stores),
            None => true,
        };
        if replace_active {
            self.clone_default_as_active(table_ref, stores)?;
        }

        Ok(())
    }

    pub fn default_id(&self, table_ref: &TableReference) -> Option<&StoreId> {
        self.default_by_table_ref.get(table_ref)
    }

    pub fn active_id(&self, table_ref: &TableReference) -> Option<&StoreId> {
        self.active_by_table_ref.get(table_ref)
    }

    /// Ensures that a table has an active blueprint.
    pub fn ensure_active_blueprint(
        &mut self,
        table_ref: &TableReference,
        store_hub: &mut StoreHub,
    ) -> Result<(), TableBlueprintError> {
        let stores = store_hub.store_bundle();

        // Remove any active or default blueprints that are no longer present in the store bundle.
        if let Some(default_id) = self.default_id(table_ref)
            && !stores.contains(default_id)
        {
            self.default_by_table_ref.remove(table_ref);
        }
        if let Some(active_id) = self.active_id(table_ref)
            && !stores.contains(active_id)
        {
            self.active_by_table_ref.remove(table_ref);
            self.active_baseline.remove(table_ref);
        }

        if self.active_id(table_ref).is_some() {
            return Ok(());
        }

        self.try_to_load_persisted_blueprint(table_ref, store_hub);
        if self.active_id(table_ref).is_some() {
            return Ok(());
        }

        let stores = store_hub.store_bundle_mut();
        if self.default_id(table_ref).is_some() {
            return self.clone_default_as_active(table_ref, stores);
        }

        let active_id = StoreId::random(StoreKind::Blueprint, "table-blueprint");
        let latest_row_id = stores.blueprint_entry(&active_id).latest_row_id();
        self.active_by_table_ref
            .insert(table_ref.clone(), active_id);
        self.active_baseline
            .insert(table_ref.clone(), latest_row_id);

        Ok(())
    }

    /// Returns the blueprint context for the given table reference.
    pub fn blueprint_context_for<'a>(
        &self,
        ctx: &AppContext<'a>,
        table_ref: &TableReference,
    ) -> AppBlueprintCtx<'a> {
        static EMPTY_BLUEPRINT: LazyLock<EntityDb> = LazyLock::new(|| {
            EntityDb::new(StoreId::random(
                StoreKind::Blueprint,
                "missing-table-blueprint",
            ))
        });

        let current_blueprint = self
            .active_id(table_ref)
            .and_then(|active_id| ctx.storage_context.bundle.get(active_id))
            .unwrap_or_else(|| {
                re_log::debug_warn_once!(
                    "Table {table_ref:?} has no active blueprint. An active table blueprint should already have been set."
                );
                &EMPTY_BLUEPRINT
            });

        AppBlueprintCtx {
            command_sender: ctx.command_sender,
            current_blueprint,
            default_blueprint: self
                .default_id(table_ref)
                .and_then(|store_id| ctx.storage_context.bundle.get(store_id)),

            // TODO(andreas): We should establish undo/redo for table blueprint edits.
            blueprint_query: re_chunk_store::LatestAtQuery::latest(
                re_viewer_context::blueprint_timeline(),
            ),
        }
    }

    /// Reset an active table blueprint to the current default.
    pub fn reset(
        &mut self,
        table_ref: &TableReference,
        store_hub: &mut StoreHub,
    ) -> Result<(), TableBlueprintError> {
        let persistence_key = BlueprintPersistenceKey::Table(Box::new(table_ref.clone()));
        if let Err(err) = store_hub.delete_persisted_blueprint(&persistence_key) {
            re_log::warn!("Failed to delete reset table blueprint: {err}");
        }

        let stores = store_hub.store_bundle_mut();
        if self.default_id(table_ref).is_some() {
            self.clone_default_as_active(table_ref, stores)?;
        } else {
            if let Some(old_active) = self.active_by_table_ref.remove(table_ref) {
                self.remove_if_orphaned(stores, &old_active);
            }
            self.active_baseline.remove(table_ref);
        }

        Ok(())
    }

    /// Removes all associated blueprints for a given table.
    pub fn close_table(&mut self, table_ref: &TableReference, store_hub: &mut StoreHub) {
        if let Err(err) = self.save_persisted_blueprint(table_ref, store_hub) {
            re_log::warn!("Failed to persist table blueprint before closing it: {err}");
        }

        self.active_baseline.remove(table_ref);
        let old_ids = [
            self.default_by_table_ref.remove(table_ref),
            self.active_by_table_ref.remove(table_ref),
        ];
        for id in old_ids.into_iter().flatten() {
            self.remove_if_orphaned(store_hub.store_bundle_mut(), &id);
        }
    }

    /// Removes all associated blueprints for all tables.
    pub fn close_all_tables(&mut self, store_hub: &mut StoreHub) {
        if let Err(err) = self.save_persisted_blueprints(store_hub) {
            re_log::warn!("Failed to save table blueprints before closing them: {err}");
        }

        self.active_baseline.clear();
        let old_ids: Vec<_> = std::iter::chain(
            self.default_by_table_ref.drain().map(|(_, id)| id),
            self.active_by_table_ref.drain().map(|(_, id)| id),
        )
        .collect();

        for id in old_ids {
            store_hub.store_bundle_mut().remove(&id);
        }
    }

    /// Removes all active clones and replaces them with fresh clones of their defaults.
    pub fn clear_all_cloned_blueprints(&mut self, store_hub: &mut StoreHub) {
        if let Err(err) = store_hub.clear_persisted_table_blueprints() {
            re_log::warn!("Failed to clear persisted table blueprints: {err}");
        }

        let stores = store_hub.store_bundle_mut();
        let table_refs: Vec<_> = self.default_by_table_ref.keys().cloned().collect();
        let old_active_ids: Vec<_> = self.active_by_table_ref.drain().map(|(_, id)| id).collect();
        self.active_baseline.clear();

        for id in old_active_ids {
            self.remove_if_orphaned(stores, &id);
        }

        for table_ref in table_refs {
            if let Err(err) = self.clone_default_as_active(&table_ref, stores) {
                re_log::warn!("Failed to reset table blueprint: {err}");
            }
        }
    }

    fn references_store(&self, store_id: &StoreId) -> bool {
        self.default_by_table_ref.values().any(|id| id == store_id)
            || self.active_by_table_ref.values().any(|id| id == store_id)
    }

    /// Removes a store if it is not referenced by any table blueprint.
    fn remove_if_orphaned(&self, stores: &mut StoreBundle, store_id: &StoreId) {
        if !self.references_store(store_id) {
            stores.remove(store_id);
        }
    }

    fn active_is_modified(
        &self,
        table_ref: &TableReference,
        active_id: &StoreId,
        stores: &StoreBundle,
    ) -> bool {
        let Some(active) = stores.get(active_id) else {
            return false;
        };
        self.active_baseline
            .get(table_ref)
            .is_some_and(|row_id| active.latest_row_id() != *row_id)
    }

    /// Clones the default for a given table, sets it as the active and removes the old active if it exists.
    /// Does nothing if there is no default for the given table.
    fn clone_default_as_active(
        &mut self,
        table_ref: &TableReference,
        stores: &mut StoreBundle,
    ) -> Result<(), TableBlueprintError> {
        let Some(source_store_id) = self.default_id(table_ref).cloned() else {
            return Ok(());
        };

        let source = stores
            .get(&source_store_id)
            .ok_or_else(|| TableBlueprintError::MissingStore(source_store_id.clone()))?;

        let new_store_id = StoreId::random(StoreKind::Blueprint, "table-blueprint");
        let new_blueprint = source.clone_with_new_id(new_store_id.clone())?;
        let latest_row_id = new_blueprint.latest_row_id();
        let old_active = self
            .active_by_table_ref
            .insert(table_ref.clone(), new_store_id.clone());
        if let Some(old_active) = old_active {
            self.remove_if_orphaned(stores, &old_active);
        }

        self.active_baseline
            .insert(table_ref.clone(), latest_row_id);
        stores.insert(new_blueprint);

        Ok(())
    }

    /// Save all active table blueprints.
    pub fn save_persisted_blueprints(&self, store_hub: &mut StoreHub) -> anyhow::Result<()> {
        let table_refs: Vec<_> = self.active_by_table_ref.keys().cloned().collect();
        let mut save_result = Ok(());

        for table_ref in table_refs {
            if let Err(err) = self.save_persisted_blueprint(&table_ref, store_hub)
                && save_result.is_ok()
            {
                save_result = Err(err);
            }
        }

        save_result
    }

    fn save_persisted_blueprint(
        &self,
        table_ref: &TableReference,
        store_hub: &mut StoreHub,
    ) -> anyhow::Result<()> {
        let Some(active_id) = self.active_id(table_ref) else {
            return Ok(());
        };
        store_hub.save_persisted_blueprint_if_changed(
            &BlueprintPersistenceKey::Table(Box::new(table_ref.clone())),
            active_id,
        )
    }

    fn try_to_load_persisted_blueprint(
        &mut self,
        table_ref: &TableReference,
        store_hub: &mut StoreHub,
    ) {
        let persistence_key = BlueprintPersistenceKey::Table(Box::new(table_ref.clone()));
        let blueprint = match store_hub.load_persisted_blueprint(&persistence_key) {
            Ok(Some(blueprint)) => blueprint,
            Ok(None) => return,
            Err(err) => {
                re_log::warn!("Failed to load persisted table blueprint: {err}");
                return;
            }
        };

        let active_id = blueprint.store_id().clone();
        self.active_by_table_ref
            .insert(table_ref.clone(), active_id.clone());
        self.active_baseline.insert(table_ref.clone(), None);
        store_hub.store_bundle_mut().insert(blueprint);
        store_hub.mark_blueprint_persisted(&active_id);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_mutex::Mutex;

    use ahash::HashMap;
    use re_chunk_store::external::re_chunk::Chunk;
    use re_entity_db::{EntityDb, StoreBundle};
    use re_log_types::{ApplicationId, StoreId, StoreKind, TimePoint};
    use re_sdk_types::archetypes::Points2D;
    use re_viewer_context::TableReference;
    use re_viewer_context::store_hub::{BlueprintPersistence, BlueprintPersistenceKey, StoreHub};

    use super::TableBlueprints;

    type Persisted = Arc<Mutex<HashMap<BlueprintPersistenceKey, EntityDb>>>;

    fn dummy_chunk() -> Arc<Chunk> {
        Arc::new(
            Chunk::builder("foo")
                .with_archetype_auto_row(TimePoint::STATIC, &Points2D::new([(0.0_f32, 0.0_f32)]))
                .build()
                .unwrap(),
        )
    }

    fn test_hub() -> (StoreHub, Persisted) {
        let persisted = Persisted::default();
        let loader_state = persisted.clone();
        let saver_state = persisted.clone();
        let deleter_state = persisted.clone();
        let clearer_state = persisted.clone();
        let persistence = BlueprintPersistence {
            loader: Some(Box::new(move |key| {
                let persisted = loader_state.lock();
                let Some(store) = persisted.get(key) else {
                    return Ok(None);
                };
                let store = store.clone_with_new_id(store.store_id().clone())?;
                let mut bundle = StoreBundle::default();
                bundle.insert(store);
                Ok(Some(bundle))
            })),
            saver: Some(Box::new(move |key, store| {
                saver_state.lock().insert(
                    key.clone(),
                    store.clone_with_new_id(store.store_id().clone())?,
                );
                Ok(())
            })),
            validator: Some(Box::new(|_| true)),
            deleter: Some(Box::new(move |key| {
                deleter_state.lock().remove(key);
                Ok(())
            })),
            table_blueprint_clearer: Some(Box::new(move || {
                clearer_state
                    .lock()
                    .retain(|key, _| matches!(key, BlueprintPersistenceKey::Recording(_)));
                Ok(())
            })),
        };

        (StoreHub::new(persistence, &|_| {}), persisted)
    }

    fn default_store(hub: &mut StoreHub, id: &str) -> StoreId {
        let id = StoreId::random(
            StoreKind::Blueprint,
            ApplicationId::try_new(id.to_owned()).unwrap(),
        );
        let bundle = hub.store_bundle_mut();
        bundle.blueprint_entry(&id);
        bundle
            .get_mut(&id)
            .unwrap()
            .add_chunk(&dummy_chunk())
            .unwrap();
        id
    }

    fn table_ref() -> TableReference {
        TableReference::RedapEntry {
            origin: re_uri::Origin::test(),
            entry_id: re_log_types::EntryId::new(),
        }
    }

    fn register_default(
        hub: &mut StoreHub,
        blueprints: &mut TableBlueprints,
        table_ref: &TableReference,
    ) -> StoreId {
        let default_id = default_store(hub, "default");
        blueprints
            .set_default_blueprint(table_ref, &default_id, hub)
            .unwrap();
        default_id
    }

    fn edit_active(
        hub: &mut StoreHub,
        blueprints: &TableBlueprints,
        table_ref: &TableReference,
    ) -> StoreId {
        let active_id = blueprints.active_id(table_ref).unwrap().clone();
        hub.store_bundle_mut()
            .get_mut(&active_id)
            .unwrap()
            .add_chunk(&dummy_chunk())
            .unwrap();
        active_id
    }

    #[test]
    fn ensure_creates_empty_active_and_later_default_replaces_it() {
        let (mut hub, _) = test_hub();
        let table_ref = TableReference::local("test_table");
        let mut blueprints = TableBlueprints::default();

        blueprints
            .ensure_active_blueprint(&table_ref, &mut hub)
            .unwrap();
        let empty_active_id = blueprints.active_id(&table_ref).unwrap().clone();
        assert!(hub.store_bundle().get(&empty_active_id).is_some());

        let default_id = default_store(&mut hub, "default");
        blueprints
            .set_default_blueprint(&table_ref, &default_id, &mut hub)
            .unwrap();

        assert!(hub.store_bundle().get(&empty_active_id).is_none());
        assert_eq!(
            hub.store_bundle()
                .get(blueprints.active_id(&table_ref).unwrap())
                .unwrap()
                .cloned_from(),
            Some(&default_id)
        );
    }

    /// Active blueprints isolate edits from registered defaults, and reset discards those edits.
    #[test]
    fn registration_clones_default_and_reset_reclones() {
        let (mut hub, _) = test_hub();
        let default_id = default_store(&mut hub, "default");
        let table_ref = TableReference::local("test_table");
        let mut blueprints = TableBlueprints::default();

        blueprints
            .set_default_blueprint(&table_ref, &default_id, &mut hub)
            .unwrap();
        let active_id = blueprints.active_id(&table_ref).unwrap().clone();
        assert_ne!(active_id, default_id);
        assert_eq!(
            hub.store_bundle().get(&active_id).unwrap().cloned_from(),
            Some(&default_id)
        );

        hub.store_bundle_mut()
            .get_mut(&active_id)
            .unwrap()
            .add_chunk(&dummy_chunk())
            .unwrap();
        blueprints.reset(&table_ref, &mut hub).unwrap();
        assert!(hub.store_bundle().get(&active_id).is_none());
        assert_eq!(
            hub.store_bundle()
                .get(blueprints.active_id(&table_ref).unwrap())
                .unwrap()
                .cloned_from(),
            Some(&default_id)
        );
    }

    /// Clearing cloned blueprints discards edits for every table and recreates each clone from its default.
    #[test]
    fn clear_all_cloned_blueprints_reclones_defaults() {
        let (mut hub, _) = test_hub();
        let first_default = default_store(&mut hub, "first-default");
        let second_default = default_store(&mut hub, "second-default");
        let first_table = TableReference::local("first-table");
        let second_table = TableReference::local("second-table");
        let mut blueprints = TableBlueprints::default();
        blueprints
            .set_default_blueprint(&first_table, &first_default, &mut hub)
            .unwrap();
        blueprints
            .set_default_blueprint(&second_table, &second_default, &mut hub)
            .unwrap();
        let first_active = blueprints.active_id(&first_table).unwrap().clone();
        let second_active = blueprints.active_id(&second_table).unwrap().clone();
        hub.store_bundle_mut()
            .get_mut(&first_active)
            .unwrap()
            .add_chunk(&dummy_chunk())
            .unwrap();

        blueprints.clear_all_cloned_blueprints(&mut hub);

        let new_first_active = blueprints.active_id(&first_table).unwrap();
        let new_second_active = blueprints.active_id(&second_table).unwrap();
        assert_ne!(new_first_active, &first_active);
        assert_ne!(new_second_active, &second_active);
        assert!(hub.store_bundle().get(&first_active).is_none());
        assert!(hub.store_bundle().get(&second_active).is_none());
        assert_eq!(
            hub.store_bundle()
                .get(new_first_active)
                .unwrap()
                .cloned_from(),
            Some(&first_default)
        );
        assert_eq!(
            hub.store_bundle()
                .get(new_second_active)
                .unwrap()
                .cloned_from(),
            Some(&second_default)
        );
    }

    /// Replacing a default must not overwrite edits in its active blueprint.
    #[test]
    fn modified_active_survives_default_replacement() {
        let (mut hub, _) = test_hub();
        let first_default = default_store(&mut hub, "first");
        let second_default = default_store(&mut hub, "second");
        let table_ref = TableReference::local("test_table");
        let mut blueprints = TableBlueprints::default();
        blueprints
            .set_default_blueprint(&table_ref, &first_default, &mut hub)
            .unwrap();
        let active_id = blueprints.active_id(&table_ref).unwrap().clone();
        hub.store_bundle_mut()
            .get_mut(&active_id)
            .unwrap()
            .add_chunk(&dummy_chunk())
            .unwrap();
        blueprints
            .set_default_blueprint(&table_ref, &second_default, &mut hub)
            .unwrap();
        assert!(hub.store_bundle().get(&first_default).is_none());
        assert_eq!(blueprints.active_id(&table_ref), Some(&active_id));
    }

    /// Removing one table must not delete a default store that another table still references.
    #[test]
    fn shared_defaults_remain_referenced() {
        let (mut hub, _) = test_hub();
        let default_id = default_store(&mut hub, "shared");
        let mut blueprints = TableBlueprints::default();
        let first = TableReference::local("first");
        let second = TableReference::local("second");

        blueprints
            .set_default_blueprint(&first, &default_id, &mut hub)
            .unwrap();
        blueprints
            .set_default_blueprint(&second, &default_id, &mut hub)
            .unwrap();

        blueprints.close_table(&first, &mut hub);

        assert!(blueprints.references_store(&default_id));
        assert!(hub.store_bundle().get(&default_id).is_some());
    }

    #[test]
    fn modified_registered_blueprint_round_trips_after_close() {
        let (mut hub, persisted) = test_hub();
        let table_ref = table_ref();
        let mut blueprints = TableBlueprints::default();
        register_default(&mut hub, &mut blueprints, &table_ref);
        let active_id = edit_active(&mut hub, &blueprints, &table_ref);
        let saved_row_id = hub.store_bundle().get(&active_id).unwrap().latest_row_id();

        // Closing the table doesn't mean removing it.
        blueprints.close_table(&table_ref, &mut hub);
        assert_eq!(persisted.lock().len(), 1);

        // Can restore from persisted state afterwards.
        let mut restored = TableBlueprints::default();
        register_default(&mut hub, &mut restored, &table_ref);
        restored
            .ensure_active_blueprint(&table_ref, &mut hub)
            .unwrap();
        let restored = hub
            .store_bundle()
            .get(restored.active_id(&table_ref).unwrap())
            .unwrap();
        assert_eq!(restored.latest_row_id(), saved_row_id);
    }

    #[test]
    fn local_blueprints_are_saved() {
        let (mut hub, persisted) = test_hub();
        let table_ref = TableReference::local("local");
        let mut blueprints = TableBlueprints::default();
        register_default(&mut hub, &mut blueprints, &table_ref);

        blueprints.save_persisted_blueprints(&mut hub).unwrap();

        assert!(
            persisted
                .lock()
                .contains_key(&BlueprintPersistenceKey::Table(Box::new(table_ref)))
        );
    }

    #[test]
    fn reset_deletes_persisted_blueprint() {
        let (mut hub, persisted) = test_hub();
        let table_ref = table_ref();
        let mut blueprints = TableBlueprints::default();
        register_default(&mut hub, &mut blueprints, &table_ref);
        edit_active(&mut hub, &blueprints, &table_ref);
        blueprints.save_persisted_blueprints(&mut hub).unwrap();

        blueprints.reset(&table_ref, &mut hub).unwrap();
        assert!(persisted.lock().is_empty());
    }
}
