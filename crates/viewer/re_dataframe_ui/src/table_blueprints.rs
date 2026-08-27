use ahash::HashMap;
use re_entity_db::StoreBundle;
use re_log_types::{StoreId, StoreKind};
use re_viewer_context::TableReference;

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
        stores: &mut StoreBundle,
    ) -> Result<(), TableBlueprintError> {
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

    /// Reset an active table blueprint to the current default.
    pub fn reset(
        &mut self,
        table_ref: &TableReference,
        stores: &mut StoreBundle,
    ) -> Result<(), TableBlueprintError> {
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
    pub fn remove_table(&mut self, table_ref: &TableReference, stores: &mut StoreBundle) {
        self.active_baseline.remove(table_ref);
        let old_ids = [
            self.default_by_table_ref.remove(table_ref),
            self.active_by_table_ref.remove(table_ref),
        ];
        for id in old_ids.into_iter().flatten() {
            self.remove_if_orphaned(stores, &id);
        }
    }

    /// Removes all associated blueprints for all tables.
    pub fn clear(&mut self, stores: &mut StoreBundle) {
        self.active_baseline.clear();
        let old_ids: Vec<_> = std::iter::chain(
            self.default_by_table_ref.drain().map(|(_, id)| id),
            self.active_by_table_ref.drain().map(|(_, id)| id),
        )
        .collect();

        for id in old_ids {
            stores.remove(&id);
        }
    }

    /// Removes all active clones and replaces them with fresh clones of their defaults.
    pub fn clear_all_cloned_blueprints(&mut self, stores: &mut StoreBundle) {
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
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_chunk_store::external::re_chunk::Chunk;
    use re_entity_db::StoreBundle;
    use re_log_types::{ApplicationId, StoreId, StoreKind, TimePoint};
    use re_sdk_types::archetypes::Points2D;
    use re_viewer_context::TableReference;

    use super::TableBlueprints;

    fn dummy_chunk() -> Arc<Chunk> {
        Arc::new(
            Chunk::builder("foo")
                .with_archetype_auto_row(TimePoint::STATIC, &Points2D::new([(0.0_f32, 0.0_f32)]))
                .build()
                .unwrap(),
        )
    }

    fn default_store(bundle: &mut StoreBundle, id: &str) -> StoreId {
        let id = StoreId::random(
            StoreKind::Blueprint,
            ApplicationId::try_new(id.to_owned()).unwrap(),
        );
        bundle.blueprint_entry(&id);
        bundle
            .get_mut(&id)
            .unwrap()
            .add_chunk(&dummy_chunk())
            .unwrap();
        id
    }

    /// Active blueprints isolate edits from registered defaults, and reset discards those edits.
    #[test]
    fn registration_clones_default_and_reset_reclones() {
        let mut stores = StoreBundle::default();
        let default_id = default_store(&mut stores, "default");
        let table_ref = TableReference::local("test_table");
        let mut blueprints = TableBlueprints::default();

        blueprints
            .set_default_blueprint(&table_ref, &default_id, &mut stores)
            .unwrap();
        let active_id = blueprints.active_id(&table_ref).unwrap().clone();
        assert_ne!(active_id, default_id);
        assert_eq!(
            stores.get(&active_id).unwrap().cloned_from(),
            Some(&default_id)
        );

        stores
            .get_mut(&active_id)
            .unwrap()
            .add_chunk(&dummy_chunk())
            .unwrap();
        blueprints.reset(&table_ref, &mut stores).unwrap();
        assert!(stores.get(&active_id).is_none());
        assert_eq!(
            stores
                .get(blueprints.active_id(&table_ref).unwrap())
                .unwrap()
                .cloned_from(),
            Some(&default_id)
        );
    }

    /// Clearing cloned blueprints discards edits for every table and recreates each clone from its default.
    #[test]
    fn clear_all_cloned_blueprints_reclones_defaults() {
        let mut stores = StoreBundle::default();
        let first_default = default_store(&mut stores, "first-default");
        let second_default = default_store(&mut stores, "second-default");
        let first_table = TableReference::local("first-table");
        let second_table = TableReference::local("second-table");
        let mut blueprints = TableBlueprints::default();
        blueprints
            .set_default_blueprint(&first_table, &first_default, &mut stores)
            .unwrap();
        blueprints
            .set_default_blueprint(&second_table, &second_default, &mut stores)
            .unwrap();
        let first_active = blueprints.active_id(&first_table).unwrap().clone();
        let second_active = blueprints.active_id(&second_table).unwrap().clone();
        stores
            .get_mut(&first_active)
            .unwrap()
            .add_chunk(&dummy_chunk())
            .unwrap();

        blueprints.clear_all_cloned_blueprints(&mut stores);

        let new_first_active = blueprints.active_id(&first_table).unwrap();
        let new_second_active = blueprints.active_id(&second_table).unwrap();
        assert_ne!(new_first_active, &first_active);
        assert_ne!(new_second_active, &second_active);
        assert!(stores.get(&first_active).is_none());
        assert!(stores.get(&second_active).is_none());
        assert_eq!(
            stores.get(new_first_active).unwrap().cloned_from(),
            Some(&first_default)
        );
        assert_eq!(
            stores.get(new_second_active).unwrap().cloned_from(),
            Some(&second_default)
        );
    }

    /// Replacing a default must not overwrite edits in its active blueprint.
    #[test]
    fn modified_active_survives_default_replacement() {
        let mut stores = StoreBundle::default();
        let first_default = default_store(&mut stores, "first");
        let second_default = default_store(&mut stores, "second");
        let table_ref = TableReference::local("test_table");
        let mut blueprints = TableBlueprints::default();
        blueprints
            .set_default_blueprint(&table_ref, &first_default, &mut stores)
            .unwrap();
        let active_id = blueprints.active_id(&table_ref).unwrap().clone();
        stores
            .get_mut(&active_id)
            .unwrap()
            .add_chunk(&dummy_chunk())
            .unwrap();
        blueprints
            .set_default_blueprint(&table_ref, &second_default, &mut stores)
            .unwrap();
        assert!(stores.get(&first_default).is_none());
        assert_eq!(blueprints.active_id(&table_ref), Some(&active_id));
    }

    /// Removing one table must not delete a default store that another table still references.
    #[test]
    fn shared_defaults_remain_referenced() {
        let mut stores = StoreBundle::default();
        let default_id = default_store(&mut stores, "shared");
        let mut blueprints = TableBlueprints::default();
        let first = TableReference::local("first");
        let second = TableReference::local("second");

        blueprints
            .set_default_blueprint(&first, &default_id, &mut stores)
            .unwrap();
        blueprints
            .set_default_blueprint(&second, &default_id, &mut stores)
            .unwrap();

        blueprints.remove_table(&first, &mut stores);

        assert!(blueprints.references_store(&default_id));
        assert!(stores.get(&default_id).is_some());
    }
}
