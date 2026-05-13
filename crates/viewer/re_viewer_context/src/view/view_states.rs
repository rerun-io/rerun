//! Storage for the state of each `View`.
//!
//! The `Viewer` has ownership of this state and pass it around to users (mainly viewport and
//! selection panel).

use ahash::HashMap;

use re_byte_size::SizeBytes as _;
use re_log_types::{StoreId, TimeReal};

use crate::blueprint_helpers::AppBlueprintCtx;
use crate::time_control::{PreviewRecordingsDb, TimeControlUpdateParams};
use crate::view::system_execution_output::VisualizerViewReport;
use crate::{
    NeedsRepaint, SystemExecutionOutput, TimeControl, ViewClass, ViewId, ViewState,
    VisualizerTypeReport,
};

/// Combined key of recording store id and view id.
///
/// The same view may be shown for different recordings, and we don't want to share
/// view state between them since it may contain recording-specific data.
type ViewStateKey = (StoreId, ViewId);

/// Shared playback state for all preview recordings shown in grid or table cards.
///
/// All preview clips are synchronized on a single [`TimeControl`]. Its time cursor
/// is a 0-based offset from the start of any recording. Per-recording positions are
/// derived on the fly by adding `range.min` to this offset.
pub struct PreviewState {
    /// Drives play, pause, loop, timeline, and cursor for all preview recordings.
    ///
    /// The time cursor is a 0-based offset, in raw timeline units, into all clips.
    time_ctrl: TimeControl,

    /// IDs of recordings currently registered as preview clips.
    recording_ids: ahash::HashSet<StoreId>,
}

impl re_byte_size::SizeBytes for PreviewState {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            time_ctrl,
            recording_ids,
        } = self;
        time_ctrl.heap_size_bytes() + recording_ids.heap_size_bytes()
    }
}

impl Default for PreviewState {
    fn default() -> Self {
        Self {
            time_ctrl: TimeControl::preview_time_control(),
            recording_ids: Default::default(),
        }
    }
}

impl PreviewState {
    /// Register a recording as an active preview clip.
    ///
    /// Called each frame by the view renderer when a preview is shown.
    pub fn register_recording(&mut self, store_id: &StoreId) {
        self.recording_ids.insert(store_id.clone());
    }

    /// Remove registrations for recordings that are no longer loaded.
    pub fn cleanup_recordings(&mut self, is_loaded: impl Fn(&StoreId) -> bool) {
        self.recording_ids.retain(|id| is_loaded(id));
    }

    /// Advance the shared playback clock for all registered preview recordings.
    ///
    /// `resolve` looks up an [`re_entity_db::EntityDb`] for a registered [`StoreId`],
    /// returning [`None`] if the recording is no longer available.
    pub fn tick<'db>(
        &mut self,
        resolve: impl Fn(&StoreId) -> Option<&'db re_entity_db::EntityDb>,
        stable_dt: f32,
    ) -> NeedsRepaint {
        if self.recording_ids.is_empty() {
            return NeedsRepaint::No;
        }

        let recordings: Vec<&re_entity_db::EntityDb> =
            self.recording_ids.iter().filter_map(resolve).collect();

        if recordings.is_empty() {
            return NeedsRepaint::No;
        }

        let is_buffering = recordings.iter().any(|r| r.is_buffering());
        let preview_db = PreviewRecordingsDb {
            recordings: &recordings,
        };

        self.time_ctrl
            .update(
                &preview_db,
                &TimeControlUpdateParams {
                    stable_dt,
                    more_data_is_streaming_in: false,
                    is_buffering,
                    should_diff_state: false,
                },
                None::<&AppBlueprintCtx<'_>>,
            )
            .needs_repaint
    }

    /// Derive an ephemeral [`TimeControl`] for one preview recording.
    ///
    /// Clones `time_ctrl` and maps the shared playback offset onto this
    /// recording's own data range as `time = range.min + offset`, clamped to
    /// `range.max`. If the recording has no data on the canonical timeline, the
    /// clone is returned as-is.
    pub fn derive_recording_time_ctrl(&self, recording: &re_entity_db::EntityDb) -> TimeControl {
        let mut tc = self.time_ctrl.clone();
        let canonical_tl = *self.time_ctrl.timeline_name();
        let Some(range) = recording.time_range_for(&canonical_tl) else {
            return tc;
        };
        let offset = self
            .time_ctrl
            .time()
            .unwrap_or_else(|| TimeReal::from(0.0_f64));
        let time = (TimeReal::from(range.min) + offset).min(TimeReal::from(range.max));
        tc.set_time_cursor_ad_hoc(canonical_tl, time);
        tc
    }
}

/// State for the `View`s that persists across frames but otherwise
/// is not saved.
#[derive(Default)]
pub struct ViewStates {
    states: HashMap<ViewStateKey, Box<dyn ViewState>>,

    /// List of all errors that occurred in visualizers of this view.
    ///
    /// This is cleared out each frame and populated after all visualizers have been executed.
    // TODO(andreas): Would be nice to bundle this with `ViewState` by making `ViewState` a struct containing errors & generic data.
    // But at point of writing this causes too much needless churn.
    visualizer_reports: HashMap<ViewStateKey, VisualizerViewReport>,

    /// Playback state shared across all preview recordings shown in grid/table cards.
    pub preview: PreviewState,
}

impl re_byte_size::SizeBytes for ViewStates {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            states,
            visualizer_reports,
            preview,
        } = self;
        states.heap_size_bytes() + visualizer_reports.heap_size_bytes() + preview.heap_size_bytes()
    }
}

impl re_byte_size::MemUsageTreeCapture for ViewStates {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        let Self {
            states,
            visualizer_reports,
            preview,
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
        node.add("preview", preview.heap_size_bytes());
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
