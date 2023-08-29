use ahash::{HashMap, HashMapExt};
use itertools::Itertools;

use re_arrow_store::{DataStoreConfig, DataStoreStats};
use re_data_store::StoreDb;
use re_log_types::{ApplicationId, StoreId, StoreKind};
use re_viewer_context::StoreContext;

#[cfg(not(target_arch = "wasm32"))]
use re_arrow_store::StoreGeneration;

#[cfg(not(target_arch = "wasm32"))]
use crate::{
    loading::load_file_path,
    saving::{default_blueprint_path, save_database_to_file},
};

/// Interface for accessing all blueprints and recordings
///
/// The [`StoreHub`] provides access to the [`StoreDb`] instances that are used
/// to store both blueprints and recordings.
///
/// Internally, the [`StoreHub`] tracks which [`ApplicationId`] and `recording
/// id` ([`StoreId`]) are currently selected in the viewer. These can be configured
/// using [`StoreHub::set_recording_id`] and [`StoreHub::set_app_id`] respectively.
pub struct StoreHub {
    selected_rec_id: Option<StoreId>,
    selected_application_id: Option<ApplicationId>,
    blueprint_by_app_id: HashMap<ApplicationId, StoreId>,
    store_dbs: StoreBundle,

    /// Was a recording ever activated? Used by the heuristic controlling the welcome screen.
    was_recording_active: bool,

    // The [`StoreGeneration`] from when the [`StoreDb`] was last saved
    #[cfg(not(target_arch = "wasm32"))]
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
        let mut blueprints = HashMap::new();
        blueprints.insert(
            Self::welcome_screen_app_id(),
            StoreId::from_string(
                StoreKind::Blueprint,
                Self::welcome_screen_app_id().to_string(),
            ),
        );

        Self {
            selected_rec_id: None,
            selected_application_id: None,
            blueprint_by_app_id: blueprints,
            store_dbs: Default::default(),

            was_recording_active: false,

            #[cfg(not(target_arch = "wasm32"))]
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
            .map(|id| self.store_dbs.blueprint_entry(id));

        // If we have a blueprint, we can return the `StoreContext`. In most
        // cases it should have already existed or been created above.
        blueprint_id
            .and_then(|id| self.store_dbs.blueprint(id))
            .map(|blueprint| {
                let recording = self
                    .selected_rec_id
                    .as_ref()
                    .and_then(|id| self.store_dbs.recording(id));

                StoreContext {
                    blueprint,
                    recording,
                    alternate_recordings: self.store_dbs.recordings().collect_vec(),
                }
            })
    }

    /// Keeps track if a recording was every activated.
    ///
    /// This useful for the heuristic controlling the welcome screen.
    pub fn was_recording_active(&self) -> bool {
        self.was_recording_active
    }

    /// Change the selected/visible recording id.
    /// This will also change the application-id to match the newly selected recording.
    pub fn set_recording_id(&mut self, recording_id: StoreId) {
        // If this recording corresponds to an app that we know about, then update the app-id.
        if let Some(app_id) = self
            .store_dbs
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
            if let Some(new_selection) = self.store_dbs.find_closest_recording(recording_id) {
                self.set_recording_id(new_selection.clone());
            } else {
                self.selected_application_id = None;
                self.selected_rec_id = None;
            }
        }

        self.store_dbs.remove(recording_id);
    }

    /// Change the selected [`ApplicationId`]
    pub fn set_app_id(&mut self, app_id: ApplicationId) {
        // If we don't know of a blueprint for this `ApplicationId` yet,
        // try to load one from the persisted store
        // TODO(2579): implement web-storage for blueprints as well
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
                self.store_dbs.remove(&blueprint_id);
            }
        }
    }

    /// Mutable access to a [`StoreDb`] by id
    pub fn store_db_mut(&mut self, store_id: &StoreId) -> &mut StoreDb {
        self.store_dbs.store_db_entry(store_id)
    }

    /// Remove any empty [`StoreDb`]s from the hub
    pub fn purge_empty(&mut self) {
        self.store_dbs.purge_empty();
    }

    /// Call [`StoreDb::purge_fraction_of_ram`] on every recording
    pub fn purge_fraction_of_ram(&mut self, fraction_to_purge: f32) {
        self.store_dbs.purge_fraction_of_ram(fraction_to_purge);
    }

    /// Directly access the [`StoreDb`] for the selected recording
    pub fn current_recording(&self) -> Option<&StoreDb> {
        self.selected_rec_id
            .as_ref()
            .and_then(|id| self.store_dbs.recording(id))
    }

    /// Check whether the [`StoreHub`] contains the referenced recording
    pub fn contains_recording(&self, id: &StoreId) -> bool {
        self.store_dbs.contains_recording(id)
    }

    /// Persist any in-use blueprints to durable storage.
    // TODO(#2579): implement persistence for web
    #[cfg(not(target_arch = "wasm32"))]
    pub fn persist_app_blueprints(&mut self) -> anyhow::Result<()> {
        // Because we save blueprints based on their `ApplicationId`, we only
        // save the blueprints referenced by `blueprint_by_app_id`, even though
        // there may be other Blueprints in the Hub.

        use re_arrow_store::GarbageCollectionOptions;
        for (app_id, blueprint_id) in &self.blueprint_by_app_id {
            let blueprint_path = default_blueprint_path(app_id)?;
            re_log::debug!("Saving blueprint for {app_id} to {blueprint_path:?}");

            if let Some(blueprint) = self.store_dbs.blueprint_mut(blueprint_id) {
                if self.blueprint_last_save.get(blueprint_id) != Some(&blueprint.generation()) {
                    let gc_rows = blueprint.store_mut().gc(GarbageCollectionOptions {
                        target: re_arrow_store::GarbageCollectionTarget::DropAtLeastFraction(1.0),
                        gc_timeless: true,
                        protect_latest: 1,
                    });
                    re_log::debug!("Cleaned up blueprint: {:?}", gc_rows);
                    // TODO(jleibs): Should we push this into a background thread? Blueprints should generally
                    // be small & fast to save, but maybe not once we start adding big pieces of user data?
                    let file_saver = save_database_to_file(blueprint, blueprint_path, None)?;
                    file_saver()?;
                    self.blueprint_last_save
                        .insert(blueprint_id.clone(), blueprint.generation());
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
        re_tracing::profile_function!();
        let blueprint_path = default_blueprint_path(app_id)?;
        if blueprint_path.exists() {
            re_log::debug!("Trying to load blueprint for {app_id} from {blueprint_path:?}",);
            let with_notification = false;
            if let Some(mut bundle) = load_file_path(&blueprint_path, with_notification) {
                for store in bundle.drain_store_dbs() {
                    if store.store_kind() == StoreKind::Blueprint && store.app_id() == Some(app_id)
                    {
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
                        self.store_dbs.insert_blueprint(store);
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
            .and_then(|blueprint_id| self.store_dbs.blueprint(blueprint_id));

        let blueprint_stats = blueprint
            .map(|store_db| DataStoreStats::from_store(&store_db.entity_db.data_store))
            .unwrap_or_default();

        let blueprint_config = blueprint
            .map(|store_db| store_db.entity_db.data_store.config().clone())
            .unwrap_or_default();

        let recording = self
            .selected_rec_id
            .as_ref()
            .and_then(|rec_id| self.store_dbs.recording(rec_id));

        let recording_stats = recording
            .map(|store_db| DataStoreStats::from_store(&store_db.entity_db.data_store))
            .unwrap_or_default();

        let recording_config = recording
            .map(|store_db| store_db.entity_db.data_store.config().clone())
            .unwrap_or_default();

        StoreHubStats {
            blueprint_stats,
            blueprint_config,
            recording_stats,
            recording_config,
        }
    }
}

/// Stores many [`StoreDb`]s of recordings and blueprints.
#[derive(Default)]
pub struct StoreBundle {
    // TODO(emilk): two separate maps per [`StoreKind`].
    store_dbs: ahash::HashMap<StoreId, StoreDb>,
}

impl StoreBundle {
    /// Decode an rrd stream.
    /// It can theoretically contain multiple recordings, and blueprints.
    pub fn from_rrd(read: impl std::io::Read) -> anyhow::Result<Self> {
        re_tracing::profile_function!();

        let decoder = re_log_encoding::decoder::Decoder::new(read)?;

        let mut slf = Self::default();

        for msg in decoder {
            let msg = msg?;
            slf.store_db_entry(msg.store_id()).add(&msg)?;
        }
        Ok(slf)
    }

    /// Returns either a recording or blueprint [`StoreDb`].
    /// One is created if it doesn't already exist.
    pub fn store_db_entry(&mut self, id: &StoreId) -> &mut StoreDb {
        self.store_dbs
            .entry(id.clone())
            .or_insert_with(|| StoreDb::new(id.clone()))
    }

    /// All loaded [`StoreDb`], both recordings and blueprints, in arbitrary order.
    pub fn store_dbs(&self) -> impl Iterator<Item = &StoreDb> {
        self.store_dbs.values()
    }

    /// All loaded [`StoreDb`], both recordings and blueprints, in arbitrary order.
    pub fn store_dbs_mut(&mut self) -> impl Iterator<Item = &mut StoreDb> {
        self.store_dbs.values_mut()
    }

    pub fn append(&mut self, mut other: Self) {
        for (id, store_db) in other.store_dbs.drain() {
            self.store_dbs.insert(id, store_db);
        }
    }

    pub fn remove(&mut self, id: &StoreId) {
        self.store_dbs.remove(id);
    }

    /// Returns the closest "neighbor" recording to the given id.
    ///
    /// The closest neighbor is the next recording when sorted by (app ID, time), if any, or the
    /// previous one otherwise. This is used to update the selected recording when the current one
    /// is deleted.
    pub fn find_closest_recording(&self, id: &StoreId) -> Option<&StoreId> {
        let mut recs = self.recordings().collect_vec();
        recs.sort_by_key(|store_db| store_db.sort_key());

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

    // --

    pub fn contains_recording(&self, id: &StoreId) -> bool {
        debug_assert_eq!(id.kind, StoreKind::Recording);
        self.store_dbs.contains_key(id)
    }

    pub fn recording(&self, id: &StoreId) -> Option<&StoreDb> {
        debug_assert_eq!(id.kind, StoreKind::Recording);
        self.store_dbs.get(id)
    }

    pub fn recording_mut(&mut self, id: &StoreId) -> Option<&mut StoreDb> {
        debug_assert_eq!(id.kind, StoreKind::Recording);
        self.store_dbs.get_mut(id)
    }

    /// Creates one if it doesn't exist.
    pub fn recording_entry(&mut self, id: &StoreId) -> &mut StoreDb {
        debug_assert_eq!(id.kind, StoreKind::Recording);
        self.store_dbs
            .entry(id.clone())
            .or_insert_with(|| StoreDb::new(id.clone()))
    }

    pub fn insert_recording(&mut self, store_db: StoreDb) {
        debug_assert_eq!(store_db.store_kind(), StoreKind::Recording);
        self.store_dbs.insert(store_db.store_id().clone(), store_db);
    }

    pub fn insert_blueprint(&mut self, store_db: StoreDb) {
        debug_assert_eq!(store_db.store_kind(), StoreKind::Blueprint);
        self.store_dbs.insert(store_db.store_id().clone(), store_db);
    }

    pub fn recordings(&self) -> impl Iterator<Item = &StoreDb> {
        self.store_dbs
            .values()
            .filter(|log| log.store_kind() == StoreKind::Recording)
    }

    pub fn blueprints(&self) -> impl Iterator<Item = &StoreDb> {
        self.store_dbs
            .values()
            .filter(|log| log.store_kind() == StoreKind::Blueprint)
    }

    // --

    pub fn contains_blueprint(&self, id: &StoreId) -> bool {
        debug_assert_eq!(id.kind, StoreKind::Blueprint);
        self.store_dbs.contains_key(id)
    }

    pub fn blueprint(&self, id: &StoreId) -> Option<&StoreDb> {
        debug_assert_eq!(id.kind, StoreKind::Blueprint);
        self.store_dbs.get(id)
    }

    pub fn blueprint_mut(&mut self, id: &StoreId) -> Option<&mut StoreDb> {
        debug_assert_eq!(id.kind, StoreKind::Blueprint);
        self.store_dbs.get_mut(id)
    }

    /// Creates one if it doesn't exist.
    pub fn blueprint_entry(&mut self, id: &StoreId) -> &mut StoreDb {
        debug_assert_eq!(id.kind, StoreKind::Blueprint);

        self.store_dbs.entry(id.clone()).or_insert_with(|| {
            // TODO(jleibs): If the blueprint doesn't exist this probably means we are
            // initializing a new default-blueprint for the application in question.
            // Make sure it's marked as a blueprint.

            let mut blueprint_db = StoreDb::new(id.clone());

            blueprint_db.add_begin_recording_msg(&re_log_types::SetStoreInfo {
                row_id: re_log_types::RowId::random(),
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
        self.store_dbs.retain(|_, store_db| !store_db.is_empty());
    }

    pub fn purge_fraction_of_ram(&mut self, fraction_to_purge: f32) {
        re_tracing::profile_function!();

        for store_db in self.store_dbs.values_mut() {
            store_db.purge_fraction_of_ram(fraction_to_purge);
        }
    }

    pub fn drain_store_dbs(&mut self) -> impl Iterator<Item = StoreDb> + '_ {
        self.store_dbs.drain().map(|(_, store)| store)
    }
}
