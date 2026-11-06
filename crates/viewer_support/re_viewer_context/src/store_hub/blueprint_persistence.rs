//! Saving blueprints to persisted storage and loading them back.

use anyhow::Context as _;
use re_entity_db::{EntityDb, StoreBundle};
use re_log_types::{ApplicationId, StoreId, StoreKind};

use super::StoreHub;
use crate::TableReference;

/// Identifies a persisted application or table blueprint.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BlueprintPersistenceKey {
    Recording(ApplicationId),
    Table(Box<TableReference>),
}

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
}

impl StoreHub {
    /// Load persisted blueprints for the given [`ApplicationId`], if they exist and aren't there already.
    pub fn load_persisted_blueprints_for_app(&mut self, app_id: &ApplicationId) {
        if !self.active_blueprint_by_app_id.contains_key(app_id)
            && let Err(err) = self.try_to_load_persisted_blueprint(app_id)
        {
            re_log::warn!("Failed to load persisted blueprint: {err}");
        }
    }

    /// Persist any in-use blueprints to durable storage.
    pub fn save_app_blueprints(&mut self) -> anyhow::Result<()> {
        re_tracing::profile_function!();

        // Save only blueprints referenced by the application association map, even though the hub
        // can contain other blueprint stores.
        let blueprints: Vec<_> = self
            .active_blueprint_by_app_id
            .iter()
            .filter(|(app_id, _)| *app_id != Self::welcome_screen_app_id())
            .map(|(app_id, store_id)| (app_id.clone(), store_id.clone()))
            .collect();

        for (app_id, store_id) in blueprints {
            self.save_persisted_blueprint_if_changed(
                &BlueprintPersistenceKey::Recording(app_id),
                &store_id,
            )?;
        }

        Ok(())
    }

    /// Load one persisted blueprint without activating it.
    pub fn load_persisted_blueprint(
        &self,
        key: &BlueprintPersistenceKey,
    ) -> anyhow::Result<Option<EntityDb>> {
        let Some(loader) = &self.persistence.loader else {
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
                    .persistence
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
            self.blueprint_last_save
                .insert(store_id.clone(), blueprint.generation());
        }
    }

    /// Save a blueprint when its generation has changed since the last save.
    pub fn save_persisted_blueprint_if_changed(
        &mut self,
        key: &BlueprintPersistenceKey,
        store_id: &StoreId,
    ) -> anyhow::Result<()> {
        let Some(saver) = &self.persistence.saver else {
            return Ok(());
        };
        let blueprint = self
            .store_bundle
            .get(store_id)
            .with_context(|| format!("Missing blueprint store: {store_id:?}"))?;
        if blueprint.store_kind() != StoreKind::Blueprint {
            anyhow::bail!("Store is not a blueprint: {store_id:?}");
        }
        if self.blueprint_last_save.get(store_id) == Some(&blueprint.generation()) {
            return Ok(());
        }

        (saver)(key, blueprint)?;
        self.blueprint_last_save
            .insert(store_id.clone(), blueprint.generation());
        Ok(())
    }

    /// Delete one persisted blueprint.
    pub fn delete_persisted_blueprint(&self, key: &BlueprintPersistenceKey) -> anyhow::Result<()> {
        let Some(deleter) = &self.persistence.deleter else {
            return Ok(());
        };
        (deleter)(key)
    }

    /// Delete all persisted table blueprints, including blueprints for unopened tables.
    pub fn clear_persisted_table_blueprints(&self) -> anyhow::Result<()> {
        let Some(clearer) = &self.persistence.table_blueprint_clearer else {
            return Ok(());
        };
        (clearer)()
    }

    /// Try to load the persisted blueprint for the given `ApplicationId`.
    /// Note: If no blueprint exists at the expected path, the result is still considered `Ok`.
    /// It is only an `Error` if a blueprint exists but fails to load.
    fn try_to_load_persisted_blueprint(&mut self, app_id: &ApplicationId) -> anyhow::Result<()> {
        re_tracing::profile_function!();

        let key = BlueprintPersistenceKey::Recording(app_id.clone());
        if let Some(blueprint) = self.load_persisted_blueprint(&key)? {
            let mut bundle = StoreBundle::default();
            bundle.insert(blueprint);
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
}
