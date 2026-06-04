//! Storage for the state of each `View`.
//!
//! The `Viewer` has ownership of this state and pass it around to users (mainly viewport and
//! selection panel).

use ahash::HashMap;

use re_byte_size::SizeBytes as _;
use re_log_types::StoreId;

use crate::view::system_execution_output::VisualizerViewReport;
use crate::{
    AppBlueprintCtx, NeedsRepaint, SystemExecutionOutput, TimeControl, TimeControlUpdateParams,
    ViewClass, ViewId, ViewState, VisualizerTypeReport,
};

/// Combined key of recording store id and view id.
///
/// The same view may be shown for different recordings, and we don't want to share
/// view state between them since it may contain recording-specific data.
type ViewStateKey = (StoreId, ViewId);

#[derive(re_byte_size::SizeBytes)]
struct ActivePreview {
    time_control: TimeControl,
}

impl Default for ActivePreview {
    fn default() -> Self {
        Self {
            time_control: TimeControl::preview_time_control(),
        }
    }
}

/// Shared playback state for all preview recordings shown in grid or table cards.
///
/// All active previews have their own [`TimeControl`].
#[derive(Default, re_byte_size::SizeBytes)]
pub struct PreviewState {
    /// The previews that are currently active.
    active_previews: ahash::HashMap<StoreId, ActivePreview>,

    /// URIs that have already been requested.
    pub requested_uris: ahash::HashSet<re_uri::DatasetSegmentUri>,
}

impl PreviewState {
    /// Register a recording as an active preview clip.
    ///
    /// Called each frame by the view renderer when a preview is shown.
    pub fn register_recording(
        &mut self,
        store_id: &StoreId,
        store_bundle: &re_entity_db::StoreBundle,
    ) {
        self.active_previews.entry(store_id.clone()).or_default();

        if let Some(db) = store_bundle.get(store_id)
            && let Some(re_entity_db::LogSource::RedapGrpcStream { uri, .. }) = &db.data_source
        {
            // If we've successfully loaded a uri, we could possibly want to request
            // it again later if it gets GC'ed.
            self.requested_uris.remove(uri);
        }
    }

    /// Remove registrations for recordings that are no longer loaded.
    pub fn cleanup_recordings(&mut self, is_loaded: impl Fn(&StoreId) -> bool) {
        self.active_previews.retain(|id, _| is_loaded(id));
    }

    pub fn tick<'db>(
        &mut self,
        resolve: impl Fn(&StoreId) -> Option<&'db re_entity_db::EntityDb>,
        stable_dt: f32,
    ) -> NeedsRepaint {
        let mut needs_repaint = NeedsRepaint::No;

        #[expect(clippy::iter_over_hash_type)] // Fine here, we're updating each one individually.
        for (id, active_preview) in &mut self.active_previews {
            let Some(db) = resolve(id) else {
                continue;
            };

            let res = active_preview.time_control.update(
                db,
                &TimeControlUpdateParams {
                    stable_dt,
                    more_data_is_streaming_in: false,
                    is_buffering: db.is_buffering(),
                    should_diff_state: false,
                },
                None::<&AppBlueprintCtx<'_>>,
            );

            needs_repaint = needs_repaint.or(res.needs_repaint);
        }

        needs_repaint
    }

    pub fn recording_time_control(&self, store_id: &StoreId) -> Option<&TimeControl> {
        self.active_previews.get(store_id).map(|p| &p.time_control)
    }

    pub fn recording_time_control_mut(&mut self, store_id: &StoreId) -> Option<&mut TimeControl> {
        self.active_previews
            .get_mut(store_id)
            .map(|p| &mut p.time_control)
    }
}

/// State for the `View`s that persists across frames but otherwise
/// is not saved.
#[derive(Default, re_byte_size::SizeBytes)]
pub struct ViewStates {
    states: HashMap<ViewStateKey, Box<dyn ViewState>>,

    /// List of all errors that occurred in visualizers of this view.
    ///
    /// This is cleared out each frame and populated after all visualizers have been executed.
    // TODO(andreas): Would be nice to bundle this with `ViewState` by making `ViewState` a struct containing errors & generic data.
    // But at point of writing this causes too much needless churn.
    visualizer_reports: HashMap<ViewStateKey, VisualizerViewReport>,

    // TODO(isse): Should we have one preview state per table/dataset?
    /// Playback state shared across all preview recordings shown in grid/table cards.
    pub preview_state: Option<PreviewState>,
}

impl re_byte_size::MemUsageTreeCapture for ViewStates {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        let Self {
            states,
            visualizer_reports,
            preview_state,
        } = self;

        let mut state_sizes = states
            .iter()
            .map(|((store_id, view_id), state)| {
                (
                    format!("{store_id:?}/{view_id:?}"),
                    state.total_size_bytes(),
                )
            })
            .collect::<Vec<_>>();
        state_sizes.sort_by(|(lhs, _), (rhs, _)| lhs.cmp(rhs));

        let mut states_node = re_byte_size::MemUsageNode::default();
        for (name, size_bytes) in state_sizes {
            states_node.add(name, size_bytes);
        }

        let mut node = re_byte_size::MemUsageNode::default();
        node.add("states", states_node.into_tree());
        node.add("visualizer_reports", visualizer_reports.heap_size_bytes());
        node.add("preview", preview_state.heap_size_bytes());
        node.with_total_size_bytes(self.total_size_bytes())
    }
}

impl ViewStates {
    pub fn get(&self, store_id: &StoreId, view_id: ViewId) -> Option<&dyn ViewState> {
        self.states
            .get(&(store_id.clone(), view_id))
            .map(|s| s.as_ref())
    }

    pub fn get_mut_or_create(
        &mut self,
        store_id: &StoreId,
        view_id: ViewId,
        view_class: &dyn ViewClass,
    ) -> &mut dyn ViewState {
        self.states
            .entry((store_id.clone(), view_id))
            .or_insert_with(|| view_class.new_state())
            .as_mut()
    }

    pub fn ensure_state_exists(
        &mut self,
        store_id: &StoreId,
        view_id: ViewId,
        view_class: &dyn ViewClass,
    ) {
        self.states
            .entry((store_id.clone(), view_id))
            .or_insert_with(|| view_class.new_state());
    }

    /// Removes all previously stored visualizer reports.
    pub fn reset_visualizer_reports(&mut self) {
        self.visualizer_reports.clear();
    }

    /// Adds visualizer reports from a system execution output for a given view.
    pub fn add_visualizer_reports_from_output(
        &mut self,
        store_id: &StoreId,
        view_id: ViewId,
        system_output: &SystemExecutionOutput,
    ) {
        let per_visualizer_reports = self
            .visualizer_reports
            .entry((store_id.clone(), view_id))
            .or_default();

        per_visualizer_reports.extend(system_output.visualizer_execution_output.iter().filter_map(
            |(id, result)| VisualizerTypeReport::from_result(result).map(|error| (*id, error)),
        ));
    }

    /// Access latest visualizer reports (warnings and errors) for a given view.
    pub fn per_visualizer_type_reports(
        &self,
        store_id: &StoreId,
        view_id: ViewId,
    ) -> Option<&VisualizerViewReport> {
        self.visualizer_reports.get(&(store_id.clone(), view_id))
    }
}
