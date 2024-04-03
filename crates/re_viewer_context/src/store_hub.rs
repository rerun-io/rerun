use ahash::{HashMap, HashMapExt};

use anyhow::Context as _;

use re_data_store::StoreGeneration;
use re_data_store::{DataStoreConfig, DataStoreStats};
use re_entity_db::{EntityDb, StoreBundle};
use re_log_types::{ApplicationId, StoreId, StoreKind};
use re_query_cache::CachesStats;

use crate::{AppOptions, StoreContext};

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
pub struct StoreHub {
    /// How we load and save blueprints.
    persistence: BlueprintPersistence,

    active_rec_id: Option<StoreId>,
    active_application_id: Option<ApplicationId>,
    default_blueprint_by_app_id: HashMap<ApplicationId, StoreId>,
    active_blueprint_by_app_id: HashMap<ApplicationId, StoreId>,
    store_bundle: StoreBundle,

    /// Was a recording ever activated? Used by the heuristic controlling the welcome screen.
    was_recording_active: bool,

    // The [`StoreGeneration`] from when the [`EntityDb`] was last saved
    blueprint_last_save: HashMap<StoreId, StoreGeneration>,
}

/// Load a blueprint from persisted storage, e.g. disk.
/// Returns `Ok(None)` if no blueprint is found.
pub type BlueprintLoader =
    dyn Fn(&ApplicationId) -> anyhow::Result<Option<StoreBundle>> + Send + Sync;

/// Save a blueprint to persisted storage, e.g. disk.
pub type BlueprintSaver = dyn Fn(&ApplicationId, &EntityDb) -> anyhow::Result<()> + Send + Sync;

/// How to save and load blueprints
pub struct BlueprintPersistence {
    pub loader: Option<Box<BlueprintLoader>>,
    pub saver: Option<Box<BlueprintSaver>>,
}

/// Convenient information used for `MemoryPanel`
#[derive(Default)]
pub struct StoreHubStats {
    pub blueprint_stats: DataStoreStats,
    pub blueprint_config: DataStoreConfig,

    pub recording_stats: DataStoreStats,
    pub recording_cached_stats: CachesStats,
    pub recording_config: DataStoreConfig,
}

impl StoreHub {
    /// App ID used as a marker to display the welcome screen.
    pub fn welcome_screen_app_id() -> ApplicationId {
        "<welcome screen>".into()
    }

    /// Used only for tests
    pub fn test_hub() -> Self {
        Self::new(
            BlueprintPersistence {
                loader: None,
                saver: None,
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
        let mut active_blueprint_by_app_id = HashMap::new();
        let mut store_bundle = StoreBundle::default();

        let welcome_screen_store_id = StoreId::from_string(
            StoreKind::Blueprint,
            Self::welcome_screen_app_id().to_string(),
        );
        active_blueprint_by_app_id.insert(
            Self::welcome_screen_app_id(),
            welcome_screen_store_id.clone(),
        );

        let welcome_screen_blueprint = store_bundle.blueprint_entry(&welcome_screen_store_id);
        (setup_welcome_screen_blueprint)(welcome_screen_blueprint);

        Self {
            persistence,

            active_rec_id: None,
            active_application_id: None,
            default_blueprint_by_app_id: Default::default(),
            active_blueprint_by_app_id,
            store_bundle,

            was_recording_active: false,

            blueprint_last_save: Default::default(),
        }
    }

    // ---------------------
    // Accessors

    /// All the loaded recordings and blueprints.
    #[inline]
    pub fn store_bundle(&self) -> &StoreBundle {
        &self.store_bundle
    }

    /// Get a read-only [`StoreContext`] from the [`StoreHub`] if one is available.
    ///
    /// All of the returned references to blueprints and recordings will have a
    /// matching [`ApplicationId`].
    pub fn read_context(&mut self) -> Option<StoreContext<'_>> {
        static EMPTY_ENTITY_DB: once_cell::sync::Lazy<EntityDb> =
            once_cell::sync::Lazy::new(|| EntityDb::new(re_log_types::StoreId::empty_recording()));

        // If we have an app-id, then use it to look up the blueprint.
        let app_id = self.active_application_id.clone()?;

        // If there's no active blueprint for this app, try to make the current default one active.
        if !self.active_blueprint_by_app_id.contains_key(&app_id) {
            if let Some(blueprint_id) = self.default_blueprint_by_app_id.get(&app_id).cloned() {
                self.set_cloned_blueprint_active_for_app(&app_id, &blueprint_id)
                    .unwrap_or_else(|err| {
                        re_log::warn!("Failed to make blueprint active: {err}");
                    });
            }
        }

        // Get the id is of whatever blueprint is now active, falling back on the "app blueprint" if needed.
        let blueprint_id = self
            .active_blueprint_by_app_id
            .entry(app_id.clone())
            .or_insert_with(|| StoreId::from_string(StoreKind::Blueprint, app_id.clone().0));

        // Get or create the blueprint:
        self.store_bundle.blueprint_entry(blueprint_id);
        let blueprint = self.store_bundle.get(blueprint_id)?;

        let recording = self
            .active_rec_id
            .as_ref()
            .and_then(|id| self.store_bundle.get(id));

        Some(StoreContext {
            app_id,
            blueprint,
            recording: recording.unwrap_or(&EMPTY_ENTITY_DB),
            bundle: &self.store_bundle,
            hub: self,
        })
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

    pub fn remove(&mut self, store_id: &StoreId) {
        if self.active_rec_id.as_ref() == Some(store_id) {
            if let Some(new_selection) = self.store_bundle.find_closest_recording(store_id) {
                self.set_active_recording_id(new_selection.clone());
            } else {
                self.active_application_id = None;
                self.active_rec_id = None;
            }
        }

        self.store_bundle.remove(store_id);
    }

    /// Remove all open recordings, and go to the welcome page.
    pub fn clear_recordings(&mut self) {
        self.store_bundle
            .retain(|db| db.store_kind() != StoreKind::Recording);
        self.active_rec_id = None;
        self.active_application_id = Some(Self::welcome_screen_app_id());
    }

    // ---------------------
    // Active app

    /// Change the active [`ApplicationId`]
    pub fn set_active_app(&mut self, app_id: ApplicationId) {
        // If we don't know of a blueprint for this `ApplicationId` yet,
        // try to load one from the persisted store
        if !self.active_blueprint_by_app_id.contains_key(&app_id) {
            if let Err(err) = self.try_to_load_persisted_blueprint(&app_id) {
                re_log::warn!("Failed to load persisted blueprint: {err}");
            }
        }

        self.active_application_id = Some(app_id);
    }

    #[inline]
    pub fn active_app(&self) -> Option<&ApplicationId> {
        self.active_application_id.as_ref()
    }

    // ---------------------
    // Active recording

    /// Keeps track if a recording was ever activated.
    ///
    /// This is useful for the heuristic controlling the welcome screen.
    #[inline]
    pub fn was_recording_active(&self) -> bool {
        self.was_recording_active
    }

    /// Directly access the [`EntityDb`] for the active recording.
    #[inline]
    pub fn active_recording_id(&self) -> Option<&StoreId> {
        self.active_rec_id.as_ref()
    }

    /// Directly access the [`EntityDb`] for the active recording.
    #[inline]
    pub fn active_recording(&self) -> Option<&EntityDb> {
        self.active_rec_id
            .as_ref()
            .and_then(|id| self.store_bundle.get(id))
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
        {
            self.set_active_app(app_id.clone());
        }

        self.active_rec_id = Some(recording_id);
        self.was_recording_active = true;
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

    // ---------------------
    // Default blueprint

    pub fn default_blueprint_id_for_app(&self, app_id: &ApplicationId) -> Option<&StoreId> {
        self.default_blueprint_by_app_id.get(app_id)
    }

    /// Change which blueprint is the default for a given [`ApplicationId`]
    #[inline]
    pub fn set_default_blueprint_for_app(
        &mut self,
        app_id: &ApplicationId,
        blueprint_id: &StoreId,
    ) {
        re_log::debug!("Switching default blueprint for {app_id} to {blueprint_id}");
        self.default_blueprint_by_app_id
            .insert(app_id.clone(), blueprint_id.clone());
    }

    /// Clear the current default blueprint
    pub fn clear_default_blueprint(&mut self) {
        if let Some(app_id) = &self.active_application_id {
            self.default_blueprint_by_app_id.remove(app_id);
        }
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

        re_log::debug!(
            "Cloning {blueprint_id} as {new_id} the active blueprint for {app_id} to {blueprint_id}"
        );

        let blueprint = self
            .store_bundle
            .get(blueprint_id)
            .context("missing blueprint")?;

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
                self.store_bundle.remove(&blueprint_id);
            }
        }
    }

    // ---------------------
    // Misc operations

    /// Forgets all blueprints
    pub fn clear_all_blueprints(&mut self) {
        for (_app_id, blueprint_id) in self
            .active_blueprint_by_app_id
            .drain()
            .chain(self.default_blueprint_by_app_id.drain())
        {
            self.store_bundle.remove(&blueprint_id);
        }
    }

    /// Remove any empty [`EntityDb`]s from the hub
    pub fn purge_empty(&mut self) {
        self.store_bundle.purge_empty();
    }

    /// Call [`EntityDb::purge_fraction_of_ram`] on every recording
    //
    // NOTE: If you touch any of this, make sure to play around with our GC stress test scripts
    // available under `$WORKSPACE_ROOT/tests/python/gc_stress`.
    pub fn purge_fraction_of_ram(&mut self, fraction_to_purge: f32) {
        re_tracing::profile_function!();

        let Some(store_id) = self.store_bundle.find_oldest_modified_recording().cloned() else {
            return;
        };

        let store_bundle = &mut self.store_bundle;

        let Some(entity_db) = store_bundle.get_mut(&store_id) else {
            if cfg!(debug_assertions) {
                unreachable!();
            }
            return; // unreachable
        };

        let store_size_before =
            entity_db.store().timeless_size_bytes() + entity_db.store().temporal_size_bytes();
        entity_db.purge_fraction_of_ram(fraction_to_purge);
        let store_size_after =
            entity_db.store().timeless_size_bytes() + entity_db.store().temporal_size_bytes();

        // No point keeping an empty recording around.
        if entity_db.is_empty() {
            self.remove(&store_id);
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
            self.remove(&store_id);
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
        self.store_bundle.retain(|db| {
            let Some(data_source) = &db.data_source else {
                // no data source, keep
                return true;
            };

            // retain only sources which:
            // - aren't network sources
            // - don't point at the given `uri`
            match data_source {
                re_smart_channel::SmartChannelSource::RrdHttpStream { url } => url != uri,
                re_smart_channel::SmartChannelSource::WsClient { ws_server_url } => {
                    ws_server_url != uri
                }
                _ => true,
            }
        });
    }

    pub fn gc_blueprints(&mut self, app_options: &AppOptions) {
        re_tracing::profile_function!();
        if app_options.blueprint_gc {
            for blueprint_id in self
                .active_blueprint_by_app_id
                .values()
                .chain(self.default_blueprint_by_app_id.values())
            {
                if let Some(blueprint) = self.store_bundle.get_mut(blueprint_id) {
                    // TODO(jleibs): Decide a better tuning for this. Would like to save a
                    // reasonable amount of history, or incremental snapshots.
                    blueprint.gc_everything_but_the_latest_row();
                }
            }
        }
    }

    /// Persist any in-use blueprints to durable storage.
    // TODO(#2579): implement persistence for web
    #[allow(clippy::unnecessary_wraps)]
    pub fn gc_and_persist_app_blueprints(
        &mut self,
        app_options: &AppOptions,
    ) -> anyhow::Result<()> {
        re_tracing::profile_function!();

        // Because we save blueprints based on their `ApplicationId`, we only
        // save the blueprints referenced by `blueprint_by_app_id`, even though
        // there may be other Blueprints in the Hub.

        for (app_id, blueprint_id) in &self.active_blueprint_by_app_id {
            let Some(blueprint) = self.store_bundle.get_mut(blueprint_id) else {
                re_log::debug!("Failed to find blueprint {blueprint_id}.");
                continue;
            };
            if self.blueprint_last_save.get(blueprint_id) == Some(&blueprint.generation()) {
                continue; // no change since last save
            }

            if app_options.blueprint_gc {
                blueprint.gc_everything_but_the_latest_row();
            }

            if let Some(saver) = &self.persistence.saver {
                (saver)(app_id, blueprint)?;
                self.blueprint_last_save
                    .insert(blueprint_id.clone(), blueprint.generation());
            }
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
                if store.store_kind() == StoreKind::Blueprint && store.app_id() == Some(app_id) {
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
                } else {
                    anyhow::bail!(
                        "Found unexpected store while loading blueprint: {:?}",
                        store.store_id()
                    );
                }
            }
        }

        Ok(())
    }

    /// Populate a [`StoreHubStats`] based on the active app.
    //
    // TODO(jleibs): We probably want stats for all recordings, not just
    // the active recording.
    pub fn stats(&self, detailed_cache_stats: bool) -> StoreHubStats {
        re_tracing::profile_function!();

        // If we have an app-id, then use it to look up the blueprint.
        let blueprint = self
            .active_application_id
            .as_ref()
            .and_then(|app_id| self.active_blueprint_by_app_id.get(app_id))
            .and_then(|blueprint_id| self.store_bundle.get(blueprint_id));

        let blueprint_stats = blueprint
            .map(|entity_db| DataStoreStats::from_store(entity_db.store()))
            .unwrap_or_default();

        let blueprint_config = blueprint
            .map(|entity_db| entity_db.store().config().clone())
            .unwrap_or_default();

        let recording = self
            .active_rec_id
            .as_ref()
            .and_then(|rec_id| self.store_bundle.get(rec_id));

        let recording_stats = recording
            .map(|entity_db| DataStoreStats::from_store(entity_db.store()))
            .unwrap_or_default();

        let recording_cached_stats = recording
            .map(|entity_db| entity_db.query_caches().stats(detailed_cache_stats))
            .unwrap_or_default();

        let recording_config = recording
            .map(|entity_db| entity_db.store().config().clone())
            .unwrap_or_default();

        StoreHubStats {
            blueprint_stats,
            blueprint_config,
            recording_stats,
            recording_cached_stats,
            recording_config,
        }
    }
}
