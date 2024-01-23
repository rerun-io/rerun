use ahash::{HashMap, HashMapExt};
use itertools::Itertools;

use re_data_store::{DataStoreConfig, DataStoreStats};
use re_entity_db::EntityDb;
use re_log_encoding::decoder::VersionPolicy;
use re_log_types::{ApplicationId, StoreId, StoreKind};
use re_viewer_context::{AppOptions, StoreContext};

use re_data_store::StoreGeneration;

#[cfg(not(target_arch = "wasm32"))]
use crate::{
    loading::load_blueprint_file,
    saving::{default_blueprint_path, save_database_to_file},
};

/// Interface for accessing all blueprints and recordings
///
/// The [`StoreHub`] provides access to the [`EntityDb`] instances that are used
/// to store both blueprints and recordings.
///
/// Internally, the [`StoreHub`] tracks which [`ApplicationId`] and `recording
/// id` ([`StoreId`]) are currently selected in the viewer. These can be configured
/// using [`StoreHub::set_recording_id`] and [`StoreHub::set_app_id`] respectively.
pub struct StoreHub {
    selected_rec_id: Option<StoreId>,
    selected_application_id: Option<ApplicationId>,
    blueprint_by_app_id: HashMap<ApplicationId, StoreId>,
    store_bundle: StoreBundle,

    /// Was a recording ever activated? Used by the heuristic controlling the welcome screen.
    was_recording_active: bool,

    // The [`StoreGeneration`] from when the [`EntityDb`] was last saved
    blueprint_last_save: HashMap<StoreId, StoreGeneration>,
}

/// Convenient information used for `MemoryPanel`
#[derive(Default)]
pub struct StoreHubStats {
    pub blueprint_stats: DataStoreStats,
    pub blueprint_config: DataStoreConfig,
    pub recording_stats: DataStoreStats,
    pub recording_config: DataStoreConfig,
}

impl StoreHub {
    /// App ID used as a marker to display the welcome screen.
    pub fn welcome_screen_app_id() -> ApplicationId {
        "<welcome screen>".into()
    }

    /// Create a new [`StoreHub`].
    ///
    /// The [`StoreHub`] will contain a single empty blueprint associated with the app ID returned
    /// by `[StoreHub::welcome_screen_app_id]`. It should be used as a marker to display the welcome
    /// screen.
    pub fn new() -> Self {
        re_tracing::profile_function!();
        let mut blueprint_by_app_id = HashMap::new();
        let mut store_bundle = StoreBundle::default();

        let welcome_screen_store_id = StoreId::from_string(
            StoreKind::Blueprint,
            Self::welcome_screen_app_id().to_string(),
        );
        blueprint_by_app_id.insert(
            Self::welcome_screen_app_id(),
            welcome_screen_store_id.clone(),
        );

        let welcome_screen_blueprint = store_bundle.blueprint_entry(&welcome_screen_store_id);
        crate::app_blueprint::setup_welcome_screen_blueprint(welcome_screen_blueprint);

        Self {
            selected_rec_id: None,
            selected_application_id: None,
            blueprint_by_app_id,
            store_bundle,

            was_recording_active: false,

            blueprint_last_save: Default::default(),
        }
    }

    /// Get a read-only [`StoreContext`] from the [`StoreHub`] if one is available.
    ///
    /// All of the returned references to blueprints and recordings will have a
    /// matching [`ApplicationId`].
    pub fn read_context(&mut self) -> Option<StoreContext<'_>> {
        // If we have an app-id, then use it to look up the blueprint.
        let blueprint_id = self.selected_application_id.as_ref().map(|app_id| {
            self.blueprint_by_app_id
                .entry(app_id.clone())
                .or_insert_with(|| StoreId::from_string(StoreKind::Blueprint, app_id.clone().0))
        });

        // As long as we have a blueprint-id, create the blueprint.
        blueprint_id
            .as_ref()
            .map(|id| self.store_bundle.blueprint_entry(id));

        // If we have a blueprint, we can return the `StoreContext`. In most
        // cases it should have already existed or been created above.
        blueprint_id
            .and_then(|id| self.store_bundle.blueprint(id))
            .map(|blueprint| {
                let recording = self
                    .selected_rec_id
                    .as_ref()
                    .and_then(|id| self.store_bundle.recording(id));

                StoreContext {
                    blueprint,
                    recording,
                    all_recordings: self.store_bundle.recordings().collect_vec(),
                }
            })
    }

    /// Keeps track if a recording was ever activated.
    ///
    /// This is useful for the heuristic controlling the welcome screen.
    pub fn was_recording_active(&self) -> bool {
        self.was_recording_active
    }

    /// Change the selected/visible recording id.
    /// This will also change the application-id to match the newly selected recording.
    pub fn set_recording_id(&mut self, recording_id: StoreId) {
        // If this recording corresponds to an app that we know about, then update the app-id.
        if let Some(app_id) = self
            .store_bundle
            .recording(&recording_id)
            .as_ref()
            .and_then(|recording| recording.app_id())
        {
            self.set_app_id(app_id.clone());
        }

        self.selected_rec_id = Some(recording_id);
        self.was_recording_active = true;
    }

    pub fn remove_recording_id(&mut self, recording_id: &StoreId) {
        if self.selected_rec_id.as_ref() == Some(recording_id) {
            if let Some(new_selection) = self.store_bundle.find_closest_recording(recording_id) {
                self.set_recording_id(new_selection.clone());
            } else {
                self.selected_application_id = None;
                self.selected_rec_id = None;
            }
        }

        self.store_bundle.remove(recording_id);
    }

    /// Change the selected [`ApplicationId`]
    pub fn set_app_id(&mut self, app_id: ApplicationId) {
        // If we don't know of a blueprint for this `ApplicationId` yet,
        // try to load one from the persisted store
        // TODO(#2579): implement web-storage for blueprints as well
        #[cfg(not(target_arch = "wasm32"))]
        if !self.blueprint_by_app_id.contains_key(&app_id) {
            if let Err(err) = self.try_to_load_persisted_blueprint(&app_id) {
                re_log::warn!("Failed to load persisted blueprint: {err}");
            }
        }

        self.selected_application_id = Some(app_id);
    }

    pub fn selected_application_id(&self) -> Option<&ApplicationId> {
        self.selected_application_id.as_ref()
    }

    /// Change which blueprint is active for a given [`ApplicationId`]
    #[inline]
    pub fn set_blueprint_for_app_id(&mut self, blueprint_id: StoreId, app_id: ApplicationId) {
        re_log::debug!("Switching blueprint for {app_id:?} to {blueprint_id:?}");
        self.blueprint_by_app_id.insert(app_id, blueprint_id);
    }

    /// Clear the current blueprint
    pub fn clear_blueprint(&mut self) {
        if let Some(app_id) = &self.selected_application_id {
            if let Some(blueprint_id) = self.blueprint_by_app_id.remove(app_id) {
                self.store_bundle.remove(&blueprint_id);
            }
        }
    }

    /// Insert a new recording into the [`StoreHub`].
    ///
    /// Note that the recording is not automatically made active. Use [`StoreHub::set_recording_id`]
    /// if needed.
    pub fn insert_recording(&mut self, entity_db: EntityDb) {
        self.store_bundle.insert_recording(entity_db);
    }

    /// Mutable access to a [`EntityDb`] by id
    pub fn entity_db_mut(&mut self, store_id: &StoreId) -> &mut EntityDb {
        self.store_bundle.entity_db_entry(store_id)
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

        let entity_dbs = &mut self.store_bundle.entity_dbs;

        let Some(entity_db) = entity_dbs.get_mut(&store_id) else {
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
            self.remove_recording_id(&store_id);
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
        if store_size_before == store_size_after && entity_dbs.len() > 1 {
            self.remove_recording_id(&store_id);
        }

        // Either we've reached our target goal or we couldn't fetch memory stats, in which case
        // we still consider that we're done anyhow.

        // NOTE: It'd be tempting to loop through recordings here, as long as we haven't reached
        // our actual target goal.
        // We cannot do that though: there are other subsystems that need to release memory before
        // we can get an accurate reading of the current memory used and decide if we should go on.
    }

    /// Directly access the [`EntityDb`] for the selected recording
    pub fn current_recording(&self) -> Option<&EntityDb> {
        self.selected_rec_id
            .as_ref()
            .and_then(|id| self.store_bundle.recording(id))
    }

    /// Check whether the [`StoreHub`] contains the referenced recording
    pub fn contains_recording(&self, id: &StoreId) -> bool {
        self.store_bundle.contains_recording(id)
    }

    /// Remove any recordings with a network source pointing at this `uri`.
    #[cfg(target_arch = "wasm32")]
    pub fn remove_recording_by_uri(&mut self, uri: &str) {
        self.store_bundle.entity_dbs.retain(|_, db| {
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
            for blueprint_id in self.blueprint_by_app_id.values() {
                if let Some(blueprint) = self.store_bundle.blueprint_mut(blueprint_id) {
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

        for (app_id, blueprint_id) in &self.blueprint_by_app_id {
            if let Some(blueprint) = self.store_bundle.blueprint_mut(blueprint_id) {
                if self.blueprint_last_save.get(blueprint_id) != Some(&blueprint.generation()) {
                    if app_options.blueprint_gc {
                        blueprint.gc_everything_but_the_latest_row();
                    }

                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let blueprint_path = default_blueprint_path(app_id)?;
                        re_log::debug!("Saving blueprint for {app_id} to {blueprint_path:?}");
                        // TODO(jleibs): Should we push this into a background thread? Blueprints should generally
                        // be small & fast to save, but maybe not once we start adding big pieces of user data?
                        let file_saver = save_database_to_file(blueprint, blueprint_path, None)?;
                        file_saver()?;
                        self.blueprint_last_save
                            .insert(blueprint_id.clone(), blueprint.generation());
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        _ = app_id;
                    }
                }
            }
        }
        Ok(())
    }

    /// Try to load the persisted blueprint for the given `ApplicationId`.
    /// Note: If no blueprint exists at the expected path, the result is still considered `Ok`.
    /// It is only an `Error` if a blueprint exists but fails to load.
    // TODO(#2579): implement persistence for web
    #[cfg(not(target_arch = "wasm32"))]
    pub fn try_to_load_persisted_blueprint(
        &mut self,
        app_id: &ApplicationId,
    ) -> anyhow::Result<()> {
        use crate::blueprint::is_valid_blueprint;

        re_tracing::profile_function!();
        let blueprint_path = default_blueprint_path(app_id)?;
        if blueprint_path.exists() {
            re_log::debug!("Trying to load blueprint for {app_id} from {blueprint_path:?}",);

            let with_notifications = false;
            if let Some(mut bundle) = load_blueprint_file(&blueprint_path, with_notifications) {
                for store in bundle.drain_entity_dbs() {
                    if store.store_kind() == StoreKind::Blueprint && store.app_id() == Some(app_id)
                    {
                        if !is_valid_blueprint(&store) {
                            re_log::warn_once!("Blueprint for {app_id} appears invalid - restoring to default. This is expected if you have just upgraded Rerun versions.");
                            continue;
                        }
                        // We found the blueprint we were looking for; make it active.
                        // borrow-checker won't let us just call `self.set_blueprint_for_app_id`
                        re_log::debug!(
                            "Switching blueprint for {app_id:?} to {:?}",
                            store.store_id(),
                        );
                        self.blueprint_by_app_id
                            .insert(app_id.clone(), store.store_id().clone());
                        self.blueprint_last_save
                            .insert(store.store_id().clone(), store.generation());
                        self.store_bundle.insert_blueprint(store);
                    } else {
                        anyhow::bail!(
                            "Found unexpected store while loading blueprint: {:?}",
                            store.store_id()
                        );
                    }
                }
            }
        }
        Ok(())
    }

    /// Populate a [`StoreHubStats`] based on the selected app.
    // TODO(jleibs): We probably want stats for all recordings, not just
    // the currently selected recording.
    pub fn stats(&self) -> StoreHubStats {
        // If we have an app-id, then use it to look up the blueprint.
        let blueprint = self
            .selected_application_id
            .as_ref()
            .and_then(|app_id| self.blueprint_by_app_id.get(app_id))
            .and_then(|blueprint_id| self.store_bundle.blueprint(blueprint_id));

        let blueprint_stats = blueprint
            .map(|entity_db| DataStoreStats::from_store(entity_db.store()))
            .unwrap_or_default();

        let blueprint_config = blueprint
            .map(|entity_db| entity_db.store().config().clone())
            .unwrap_or_default();

        let recording = self
            .selected_rec_id
            .as_ref()
            .and_then(|rec_id| self.store_bundle.recording(rec_id));

        let recording_stats = recording
            .map(|entity_db| DataStoreStats::from_store(entity_db.store()))
            .unwrap_or_default();

        let recording_config = recording
            .map(|entity_db| entity_db.store().config().clone())
            .unwrap_or_default();

        StoreHubStats {
            blueprint_stats,
            blueprint_config,
            recording_stats,
            recording_config,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum StoreLoadError {
    #[error(transparent)]
    Decode(#[from] re_log_encoding::decoder::DecodeError),

    #[error(transparent)]
    DataStore(#[from] re_entity_db::Error),
}

/// Stores many [`EntityDb`]s of recordings and blueprints.
#[derive(Default)]
pub struct StoreBundle {
    // TODO(emilk): two separate maps per [`StoreKind`].
    entity_dbs: ahash::HashMap<StoreId, EntityDb>,
}

impl StoreBundle {
    /// Decode an rrd stream.
    /// It can theoretically contain multiple recordings, and blueprints.
    pub fn from_rrd(
        version_policy: VersionPolicy,
        read: impl std::io::Read,
    ) -> Result<Self, StoreLoadError> {
        re_tracing::profile_function!();

        let decoder = re_log_encoding::decoder::Decoder::new(version_policy, read)?;

        let mut slf = Self::default();

        for msg in decoder {
            let msg = msg?;
            slf.entity_db_entry(msg.store_id()).add(&msg)?;
        }
        Ok(slf)
    }

    /// Returns either a recording or blueprint [`EntityDb`].
    /// One is created if it doesn't already exist.
    pub fn entity_db_entry(&mut self, id: &StoreId) -> &mut EntityDb {
        self.entity_dbs
            .entry(id.clone())
            .or_insert_with(|| EntityDb::new(id.clone()))
    }

    /// All loaded [`EntityDb`], both recordings and blueprints, in arbitrary order.
    pub fn entity_dbs(&self) -> impl Iterator<Item = &EntityDb> {
        self.entity_dbs.values()
    }

    /// All loaded [`EntityDb`], both recordings and blueprints, in arbitrary order.
    pub fn entity_dbs_mut(&mut self) -> impl Iterator<Item = &mut EntityDb> {
        self.entity_dbs.values_mut()
    }

    pub fn append(&mut self, mut other: Self) {
        for (id, entity_db) in other.entity_dbs.drain() {
            self.entity_dbs.insert(id, entity_db);
        }
    }

    pub fn remove(&mut self, id: &StoreId) {
        self.entity_dbs.remove(id);
    }

    /// Returns the closest "neighbor" recording to the given id.
    ///
    /// The closest neighbor is the next recording when sorted by (app ID, time), if any, or the
    /// previous one otherwise. This is used to update the selected recording when the current one
    /// is deleted.
    pub fn find_closest_recording(&self, id: &StoreId) -> Option<&StoreId> {
        let mut recs = self.recordings().collect_vec();
        recs.sort_by_key(|entity_db| entity_db.sort_key());

        let cur_pos = recs.iter().position(|rec| rec.store_id() == id);

        if let Some(cur_pos) = cur_pos {
            if recs.len() > cur_pos + 1 {
                Some(recs[cur_pos + 1].store_id())
            } else if cur_pos > 0 {
                Some(recs[cur_pos - 1].store_id())
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns the [`StoreId`] of the oldest modified recording, according to [`EntityDb::last_modified_at`].
    pub fn find_oldest_modified_recording(&self) -> Option<&StoreId> {
        let mut entity_dbs = self
            .entity_dbs
            .values()
            .filter(|db| db.store_kind() == StoreKind::Recording)
            .collect_vec();

        entity_dbs.sort_by_key(|db| db.last_modified_at());

        entity_dbs.first().map(|db| db.store_id())
    }

    // --

    pub fn contains_recording(&self, id: &StoreId) -> bool {
        debug_assert_eq!(id.kind, StoreKind::Recording);
        self.entity_dbs.contains_key(id)
    }

    pub fn recording(&self, id: &StoreId) -> Option<&EntityDb> {
        debug_assert_eq!(id.kind, StoreKind::Recording);
        self.entity_dbs.get(id)
    }

    pub fn recording_mut(&mut self, id: &StoreId) -> Option<&mut EntityDb> {
        debug_assert_eq!(id.kind, StoreKind::Recording);
        self.entity_dbs.get_mut(id)
    }

    /// Creates one if it doesn't exist.
    pub fn recording_entry(&mut self, id: &StoreId) -> &mut EntityDb {
        debug_assert_eq!(id.kind, StoreKind::Recording);
        self.entity_dbs
            .entry(id.clone())
            .or_insert_with(|| EntityDb::new(id.clone()))
    }

    pub fn insert_recording(&mut self, entity_db: EntityDb) {
        debug_assert_eq!(entity_db.store_kind(), StoreKind::Recording);
        self.entity_dbs
            .insert(entity_db.store_id().clone(), entity_db);
    }

    pub fn insert_blueprint(&mut self, entity_db: EntityDb) {
        debug_assert_eq!(entity_db.store_kind(), StoreKind::Blueprint);
        self.entity_dbs
            .insert(entity_db.store_id().clone(), entity_db);
    }

    pub fn recordings(&self) -> impl Iterator<Item = &EntityDb> {
        self.entity_dbs
            .values()
            .filter(|log| log.store_kind() == StoreKind::Recording)
    }

    pub fn blueprints(&self) -> impl Iterator<Item = &EntityDb> {
        self.entity_dbs
            .values()
            .filter(|log| log.store_kind() == StoreKind::Blueprint)
    }

    // --

    pub fn contains_blueprint(&self, id: &StoreId) -> bool {
        debug_assert_eq!(id.kind, StoreKind::Blueprint);
        self.entity_dbs.contains_key(id)
    }

    pub fn blueprint(&self, id: &StoreId) -> Option<&EntityDb> {
        debug_assert_eq!(id.kind, StoreKind::Blueprint);
        self.entity_dbs.get(id)
    }

    pub fn blueprint_mut(&mut self, id: &StoreId) -> Option<&mut EntityDb> {
        debug_assert_eq!(id.kind, StoreKind::Blueprint);
        self.entity_dbs.get_mut(id)
    }

    /// Creates one if it doesn't exist.
    pub fn blueprint_entry(&mut self, id: &StoreId) -> &mut EntityDb {
        debug_assert_eq!(id.kind, StoreKind::Blueprint);

        self.entity_dbs.entry(id.clone()).or_insert_with(|| {
            // TODO(jleibs): If the blueprint doesn't exist this probably means we are
            // initializing a new default-blueprint for the application in question.
            // Make sure it's marked as a blueprint.

            let mut blueprint_db = EntityDb::new(id.clone());

            blueprint_db.set_store_info(re_log_types::SetStoreInfo {
                row_id: re_log_types::RowId::new(),
                info: re_log_types::StoreInfo {
                    application_id: id.as_str().into(),
                    store_id: id.clone(),
                    is_official_example: false,
                    started: re_log_types::Time::now(),
                    store_source: re_log_types::StoreSource::Other("viewer".to_owned()),
                    store_kind: StoreKind::Blueprint,
                },
            });

            blueprint_db
        })
    }

    // --

    pub fn purge_empty(&mut self) {
        self.entity_dbs.retain(|_, entity_db| !entity_db.is_empty());
    }

    pub fn drain_entity_dbs(&mut self) -> impl Iterator<Item = EntityDb> + '_ {
        self.entity_dbs.drain().map(|(_, store)| store)
    }
}
