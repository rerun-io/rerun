use ahash::{HashMap, HashMapExt};

use anyhow::Context as _;
use itertools::Itertools as _;

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

    /// Once added, we never remove a key from this map.
    /// Instead we clear the contents of [`AppBlueprints`].
    app_blueprints: HashMap<ApplicationId, AppBlueprints>,

    /// The contents of all the stores.
    store_bundle: StoreBundle,

    /// The [`StoreGeneration`] from when the [`EntityDb`] was last saved
    blueprint_last_save: HashMap<StoreId, StoreGeneration>,

    /// The [`StoreGeneration`] from when the [`EntityDb`] was last garbage collected
    blueprint_last_gc: HashMap<StoreId, StoreGeneration>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct AppBlueprints {
    /// The active blueprint is what the user sees and edits (if this is the active app).
    ///
    /// If there is no active blueprint, the default will be cloned and made active,
    /// using a new [`StoreId`].
    pub active: Option<StoreId>,

    /// The default blueprint is usually the blueprint set by the SDK.
    ///
    /// Storing this lets users reset the active blueprint to the one sent by the SDK.
    pub default: Option<StoreId>,
}

impl AppBlueprints {
    fn iter(&self) -> impl Iterator<Item = &StoreId> {
        let Self { active, default } = self;
        active.iter().chain(default.iter())
    }

    fn retains(&mut self, mut keep: impl FnMut(&StoreId) -> bool) {
        let Self { active, default } = self;
        if let Some(active) = active {
            if !keep(active) {
                self.active = None;
            }
        }
        if let Some(default) = default {
            if !keep(default) {
                self.default = None;
            }
        }
    }
}

/// Load a blueprint from persisted storage, e.g. disk.
/// Returns `Ok(None)` if no blueprint is found.
pub type BlueprintLoader =
    dyn Fn(&ApplicationId) -> anyhow::Result<Option<StoreBundle>> + Send + Sync;

/// Save a blueprint to persisted storage, e.g. disk.
pub type BlueprintSaver =
    dyn Fn(&ApplicationId, &[re_log_types::LogMsg]) -> anyhow::Result<()> + Send + Sync;

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
        "Welcome screen".into()
    }

    /// Blueprint ID used for the default welcome screen blueprint
    fn welcome_screen_blueprint_id() -> StoreId {
        StoreId::from_string(
            StoreKind::Blueprint,
            Self::welcome_screen_app_id().to_string(),
        )
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
        let mut blueprints_by_app_id = HashMap::new();
        let mut store_bundle = StoreBundle::default();

        blueprints_by_app_id.insert(
            Self::welcome_screen_app_id(),
            AppBlueprints {
                active: None,
                default: Some(Self::welcome_screen_blueprint_id()),
            },
        );

        let welcome_screen_blueprint =
            store_bundle.blueprint_entry(&Self::welcome_screen_blueprint_id());
        (setup_welcome_screen_blueprint)(welcome_screen_blueprint);

        Self {
            persistence,

            active_rec_id: None,
            active_application_id: None,
            app_blueprints: blueprints_by_app_id,
            store_bundle,

            blueprint_last_save: Default::default(),
            blueprint_last_gc: Default::default(),
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

        // Defensive coding: Check that default and active blueprints exists,
        // in case some of our book-keeping is broken.
        for app_blueprints in self.app_blueprints.values_mut() {
            if let Some(blueprint_id) = &app_blueprints.active {
                if !self.store_bundle.contains(blueprint_id) {
                    app_blueprints.active = None;
                }
            }
            if let Some(blueprint_id) = &app_blueprints.default {
                if !self.store_bundle.contains(blueprint_id) {
                    app_blueprints.default = None;
                }
            }
        }

        {
            // If there's no active blueprint for this app, try to make the current default one active.
            let app_blueprints = self.app_blueprints.entry(app_id.clone()).or_default();
            if app_blueprints.active.is_none() {
                if let Some(blueprint_id) = app_blueprints.default.clone() {
                    self.set_cloned_blueprint_active_for_app(&app_id, &blueprint_id)
                        .unwrap_or_else(|err| {
                            re_log::warn!("Failed to make blueprint active: {err}");
                        });
                }
            }
        }

        let app_blueprints = self.app_blueprints.entry(app_id.clone()).or_default();

        // Get the id is of whatever blueprint is now active, falling back on the "app blueprint" if needed.
        let active_blueprint_id = app_blueprints
            .active
            .get_or_insert_with(|| StoreId::from_string(StoreKind::Blueprint, app_id.clone().0));

        // Get or create the blueprint:
        self.store_bundle.blueprint_entry(active_blueprint_id);
        let blueprint = self.store_bundle.get(active_blueprint_id)?;

        let recording = self
            .active_rec_id
            .as_ref()
            .and_then(|id| self.store_bundle.get(id));

        if recording.is_none() {
            self.active_rec_id = None;
        }

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
        let removed_store = self.store_bundle.remove(store_id);

        let Some(removed_store) = removed_store else {
            return;
        };

        match removed_store.store_kind() {
            StoreKind::Recording => {
                if let Some(app_id) = removed_store.app_id().cloned() {
                    let any_other_recordings_for_this_app = self
                        .store_bundle
                        .recordings()
                        .any(|rec| rec.app_id() == Some(&app_id));

                    if !any_other_recordings_for_this_app {
                        re_log::trace!("Removed last recording of {app_id}. Closing app.");
                        self.close_app(&app_id);
                    }
                }
            }
            StoreKind::Blueprint => {
                for app_blueprints in self.app_blueprints.values_mut() {
                    app_blueprints.retains(|id| id != store_id);
                }
            }
        }

        if self.active_rec_id.as_ref() == Some(store_id) {
            if let Some(new_selection) = self.store_bundle.find_closest_recording(store_id) {
                self.set_active_recording_id(new_selection.clone());
            } else {
                self.active_application_id = None;
                self.active_rec_id = None;
            }
        }
    }

    pub fn retain(&mut self, mut should_retain: impl FnMut(&EntityDb) -> bool) {
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
            self.remove(&store);
        }
    }

    /// Remove all open recordings and applications, and go to the welcome page.
    pub fn clear_recordings(&mut self) {
        // Keep only the welcome screen:
        self.store_bundle
            .retain(|db| db.app_id() == Some(&Self::welcome_screen_app_id()));
        self.active_rec_id = None;
        self.active_application_id = Some(Self::welcome_screen_app_id());
    }

    // ---------------------
    // Active app

    /// Change the active [`ApplicationId`]
    #[allow(clippy::needless_pass_by_value)]
    pub fn set_active_app(&mut self, app_id: ApplicationId) {
        // If we don't know of this app id yet, try to load its persisted blueprints.
        if !self.app_blueprints.contains_key(&app_id) {
            if let Err(err) = self.try_to_load_persisted_blueprint(&app_id) {
                re_log::warn!("Failed to load persisted blueprint: {err}");
            }
        }

        if self.active_application_id.as_ref() == Some(&app_id) {
            return;
        }

        self.active_application_id = Some(app_id.clone());
        self.active_rec_id = None;

        // Find any matching recording and activate it
        for rec in self
            .store_bundle
            .recordings()
            .sorted_by_key(|entity_db| entity_db.store_info().map(|info| info.started))
        {
            if rec.app_id() == Some(&app_id) {
                self.active_rec_id = Some(rec.store_id().clone());
                return;
            }
        }
    }

    /// Close this application and all its recordings.
    pub fn close_app(&mut self, app_id: &ApplicationId) {
        self.store_bundle.retain(|db| db.app_id() != Some(app_id));

        if self.active_application_id.as_ref() == Some(app_id) {
            self.active_application_id = None;
            self.active_rec_id = None;
        }

        let app_blueprints = self.app_blueprints.entry(app_id.clone()).or_default();
        *app_blueprints = Default::default();
    }

    #[inline]
    pub fn active_app(&self) -> Option<&ApplicationId> {
        self.active_application_id.as_ref()
    }

    // ---------------------
    // Active recording

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
            .cloned()
        {
            self.set_active_app(app_id);
        }

        self.active_rec_id = Some(recording_id);
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
        self.app_blueprints
            .get(app_id)
            .and_then(|app_blueprints| app_blueprints.default.as_ref())
    }

    pub fn default_blueprint_for_app(&self, app_id: &ApplicationId) -> Option<&EntityDb> {
        self.default_blueprint_id_for_app(app_id)
            .and_then(|id| self.store_bundle.get(id))
    }

    /// Change which blueprint is the default for a given [`ApplicationId`]
    #[inline]
    pub fn set_default_blueprint_for_app(
        &mut self,
        app_id: &ApplicationId,
        blueprint_id: &StoreId,
    ) {
        re_log::debug!("Switching default blueprint for {app_id} to {blueprint_id}");
        let app_blueprints = self.app_blueprints.entry(app_id.clone()).or_default();
        app_blueprints.default = Some(blueprint_id.clone());
    }

    /// Clear the current default blueprint
    pub fn clear_default_blueprint(&mut self) {
        if let Some(app_id) = &self.active_application_id {
            if let Some(app_blueprints) = self.app_blueprints.get_mut(app_id) {
                if let Some(blueprint_id) = app_blueprints.default.take() {
                    self.remove(&blueprint_id);
                }
            }
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
        self.app_blueprints
            .get(app_id)
            .and_then(|app_blueprints| app_blueprints.active.as_ref())
    }

    pub fn active_blueprint_for_app(&self, app_id: &ApplicationId) -> Option<&EntityDb> {
        self.active_blueprint_id_for_app(app_id)
            .and_then(|id| self.store_bundle.get(id))
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

        let app_blueprints = self.app_blueprints.entry(app_id.clone()).or_default();
        app_blueprints.active = Some(new_id);

        Ok(())
    }

    /// Is the given blueprint id the active blueprint for any app id?
    pub fn is_active_blueprint_for_any_app(&self, blueprint_id: &StoreId) -> bool {
        self.app_blueprints
            .values()
            .any(|app_blueprints| app_blueprints.active.as_ref() == Some(blueprint_id))
    }

    /// Clear the currently active blueprint
    pub fn clear_active_blueprint(&mut self) {
        if let Some(app_id) = &self.active_application_id {
            if let Some(app_blueprints) = self.app_blueprints.get_mut(app_id) {
                if let Some(blueprint_id) = app_blueprints.active.take() {
                    self.remove(&blueprint_id);
                }
            }
        }
    }

    // ---------------------
    // Misc operations

    /// Cloned blueprints are the ones the user has edited,
    /// i.e. NOT sent from the SDK.
    pub fn clear_all_cloned_blueprints(&mut self) {
        self.retain(|db| match db.store_kind() {
            StoreKind::Recording => true,
            StoreKind::Blueprint => db.cloned_from().is_none(),
        });
    }

    /// Remove any empty [`EntityDb`]s from the hub
    pub fn purge_empty(&mut self) {
        self.retain(|entity_db| !entity_db.is_empty());
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
        self.retain(|db| {
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
                .app_blueprints
                .values()
                .flat_map(|app_blueprints| app_blueprints.iter())
            {
                if let Some(blueprint) = self.store_bundle.get_mut(blueprint_id) {
                    if self.blueprint_last_gc.get(blueprint_id) == Some(&blueprint.generation()) {
                        continue; // no change since last gc
                    }

                    // TODO(jleibs): Decide a better tuning for this. Would like to save a
                    // reasonable amount of history, or incremental snapshots.
                    blueprint.gc_everything_but_the_latest_row();

                    self.blueprint_last_gc
                        .insert(blueprint_id.clone(), blueprint.generation());
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

        for (app_id, app_blueprints) in &self.app_blueprints {
            let Some(blueprint_id) = &app_blueprints.active else {
                continue;
            };

            let Some(blueprint) = self.store_bundle.get_mut(blueprint_id) else {
                re_log::debug!("Failed to find blueprint {blueprint_id}.");
                continue;
            };
            if self.blueprint_last_save.get(blueprint_id) == Some(&blueprint.generation()) {
                continue; // no change since last save
            }

            if app_options.blueprint_gc {
                blueprint.gc_everything_but_the_latest_row();
                self.blueprint_last_gc
                    .insert(blueprint_id.clone(), blueprint.generation());
            }

            if blueprint.app_id() == Some(&Self::welcome_screen_app_id()) {
                // Don't save changes to the welcome screen
            } else if let Some(saver) = &self.persistence.saver {
                let messages = blueprint.to_messages(None)?;
                (saver)(app_id, &messages)?;
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
                match store.store_kind() {
                    StoreKind::Recording => {
                        anyhow::bail!(
                            "Found a recording in a blueprint file: {:?}",
                            store.store_id()
                        );
                    }
                    StoreKind::Blueprint => {}
                }

                if store.app_id() != Some(app_id) {
                    if let Some(store_app_id) = store.app_id() {
                        anyhow::bail!("Found app_id {store_app_id}; expected {app_id}");
                    } else {
                        anyhow::bail!("Found store without an app_id");
                    }
                }

                // We found the blueprint we were looking for; make it active.
                // borrow-checker won't let us just call `self.set_blueprint_for_app_id`
                re_log::debug!(
                    "Activating new blueprint {} for {app_id}; loaded from disk",
                    store.store_id(),
                );
                self.app_blueprints
                    .entry(app_id.clone())
                    .or_default()
                    .active = Some(store.store_id().clone());
                self.blueprint_last_save
                    .insert(store.store_id().clone(), store.generation());
                self.store_bundle.insert(store);
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

        let active_blueprint = self
            .active_blueprint_id()
            .and_then(|blueprint_id| self.store_bundle.get(blueprint_id));

        let blueprint_stats = active_blueprint
            .map(|entity_db| DataStoreStats::from_store(entity_db.store()))
            .unwrap_or_default();

        let blueprint_config = active_blueprint
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
