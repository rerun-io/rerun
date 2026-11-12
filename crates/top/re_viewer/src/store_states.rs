//! The viewer's ephemeral state for each recording and blueprint.

use ahash::HashMap;
use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::StoreId;
use re_sdk_types::blueprint::components::PlayState;
use re_viewer_context::{AppOptions, BlueprintContext, BlueprintUndoState, StoreHub, TimeControl};

/// The viewer's ephemeral state for each recording and blueprint.
///
/// The per-store maps are filled lazily on first use with a given store.
/// [`Self::cleanup`] drops the entries of stores that are no longer loaded.
#[derive(Default)]
pub struct StoreStates {
    /// The time control for each recording (found in [`EntityDb`]).
    pub time_controls: HashMap<StoreId, TimeControl>,

    /// The time control of the blueprint timeline, used when inspecting it.
    pub blueprint_time_control: TimeControl,

    /// Maps blueprint id to the current undo state for it.
    pub blueprint_undo_state: HashMap<StoreId, BlueprintUndoState>,

    /// The selection panel's back/forward history for each recording.
    ///
    /// Kept per recording so that stepping back never jumps you to a different recording.
    pub selection_histories: HashMap<StoreId, re_selection_panel::SelectionHistory>,
}

impl StoreStates {
    /// The current time cursor for a recording, if any.
    pub fn time_cursor_for(&self, store_id: &StoreId) -> Option<re_entity_db::PrefetchTimeCursor> {
        self.time_controls.get(store_id)?.time_cursor()
    }

    /// The time control of a recording, if it has one yet.
    pub fn time_control(&self, rec_id: &StoreId) -> Option<&TimeControl> {
        self.time_controls.get(rec_id)
    }

    /// The time control of a recording, created if it has none yet.
    pub fn time_control_mut<'a>(
        &'a mut self,
        entity_db: &'a EntityDb,
        blueprint_ctx: &impl BlueprintContext,
    ) -> &'a mut TimeControl {
        create_time_control_for(&mut self.time_controls, entity_db, blueprint_ctx)
    }

    /// Removes the state of stores that are no longer loaded.
    pub fn cleanup(&mut self, store_hub: &StoreHub) {
        re_tracing::profile_function!();

        self.time_controls
            .retain(|store_id, _| store_hub.store_bundle().contains(store_id));

        self.blueprint_undo_state
            .retain(|store_id, _| store_hub.store_bundle().contains(store_id));

        self.selection_histories
            .retain(|store_id, _| store_hub.store_bundle().contains(store_id));
    }

    /// Returns the blueprint query that should be used for generating the current
    /// layout of the viewer.
    ///
    /// If `inspect_blueprint_timeline` is enabled, we use the time selection from the
    /// blueprint `time_ctrl`. Otherwise, we use a latest query from the blueprint timeline.
    ///
    /// If no blueprint is available (e.g. no active store context), falls back to
    /// a latest query on the blueprint timeline.
    pub fn blueprint_query_for_viewer(
        &mut self,
        app_options: &AppOptions,
        blueprint: Option<&EntityDb>,
    ) -> LatestAtQuery {
        if app_options.inspect_blueprint_timeline {
            if self.blueprint_time_control.play_state() == PlayState::Following {
                // Special-case just to make sure we include stuff added in this frame
                LatestAtQuery::latest(re_viewer_context::blueprint_timeline())
            } else {
                self.blueprint_time_control.current_query().clone()
            }
        } else if let Some(blueprint) = blueprint {
            let undo_state = self
                .blueprint_undo_state
                .entry(blueprint.store_id().clone())
                .or_default();
            undo_state.blueprint_query()
        } else {
            LatestAtQuery::latest(re_viewer_context::blueprint_timeline())
        }
    }

    /// Returns the blueprint query that should be used for generating the current
    /// layout of the viewer.
    ///
    /// If `inspect_blueprint_timeline` is enabled, we use the time selection from the
    /// blueprint `time_ctrl`. Otherwise, we use a latest query from the blueprint timeline.
    pub fn get_blueprint_query_for_viewer(
        &self,
        app_options: &AppOptions,
        blueprint: &EntityDb,
    ) -> Option<LatestAtQuery> {
        if app_options.inspect_blueprint_timeline {
            if self.blueprint_time_control.play_state() == PlayState::Following {
                // Special-case just to make sure we include stuff added in this frame
                Some(LatestAtQuery::latest(
                    re_viewer_context::blueprint_timeline(),
                ))
            } else {
                Some(self.blueprint_time_control.current_query().clone())
            }
        } else {
            self.blueprint_undo_state
                .get(blueprint.store_id())
                .map(|undo_state| undo_state.blueprint_query())
        }
    }
}

impl re_byte_size::MemUsageTreeCapture for StoreStates {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        use re_byte_size::SizeBytes as _;

        let mut tree = re_byte_size::MemUsageNode::default();
        tree.add(
            "blueprint_undo_state",
            self.blueprint_undo_state.total_size_bytes(),
        );
        tree.into_tree()
    }
}

fn create_time_control_for<'cfgs>(
    configs: &'cfgs mut HashMap<StoreId, TimeControl>,
    entity_db: &'_ EntityDb,
    blueprint_ctx: &'_ impl BlueprintContext,
) -> &'cfgs mut TimeControl {
    fn new_time_control(
        entity_db: &'_ EntityDb,
        blueprint_ctx: &'_ impl BlueprintContext,
    ) -> TimeControl {
        let sources = entity_db.data_sources();
        let follow = sources.is_empty() || sources.iter().any(|source| source.may_be_live());

        let play_state = if follow {
            PlayState::Following
        } else {
            // Play files from the start by default - it feels nice and alive.
            PlayState::Playing
        };

        // Apply the data-source-derived default only if the blueprint did not
        // already specify a `play_state`. Otherwise we'd clobber the user's
        // setting (and write the clobbered value back to the blueprint).
        TimeControl::from_blueprint_with_fallback_play_state(
            blueprint_ctx,
            Some(entity_db),
            play_state,
        )
    }

    configs
        .entry(entity_db.store_id().clone())
        .or_insert_with(|| new_time_control(entity_db, blueprint_ctx))
}
