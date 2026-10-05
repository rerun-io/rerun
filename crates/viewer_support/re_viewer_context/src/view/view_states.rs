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
    ViewClass, ViewId, ViewState, ViewerDiagnostic, VisualizerTypeReport,
};

/// Combined key of recording store id and view id.
///
/// The same view may be shown for different recordings, and we don't want to share
/// view state between them since it may contain recording-specific data.
type ViewStateKey = (StoreId, ViewId);

#[derive(re_byte_size::SizeBytes)]
pub struct ActivePreview {
    pub time_control: TimeControl,

    /// Whether this one preview plays, apart from the shared play state.
    ///
    /// Set by the play button and by the pointer entering the preview, cleared by the pointer
    /// leaving it and by [`PreviewState::set_playing`].
    pub play_override: Option<bool>,

    /// Whether the preview was drawn since the last [`PreviewState::tick`].
    ///
    /// Only a preview that is being drawn advances its time.
    rendered_last_frame: bool,
}

impl Default for ActivePreview {
    fn default() -> Self {
        Self {
            time_control: TimeControl::preview_time_control(),
            play_override: None,
            rendered_last_frame: false,
        }
    }
}

/// Shared playback state for all preview recordings shown in grid or table cards.
///
/// All active previews have their own [`TimeControl`].
#[derive(re_byte_size::SizeBytes)]
pub struct PreviewState {
    /// Whether every preview plays, as set by the play-all button.
    ///
    /// A single preview can be different from this if its button is pressed or
    /// it started being hovered.
    playing: bool,

    /// Playback speed of every preview, as a multiple of real time.
    speed: f32,

    /// The previews that are currently active.
    active_previews: ahash::HashMap<StoreId, ActivePreview>,

    /// URIs that have already been requested.
    pub requested_uris: ahash::HashSet<re_uri::DatasetUri>,
}

impl Default for PreviewState {
    fn default() -> Self {
        Self {
            playing: false,
            speed: 1.0,
            active_previews: Default::default(),
            requested_uris: Default::default(),
        }
    }
}

impl PreviewState {
    /// Whether every preview plays, as set by the play-all button.
    pub fn playing(&self) -> bool {
        self.playing
    }

    /// Playback speed of every preview, as a multiple of real time.
    pub fn speed(&self) -> f32 {
        self.speed
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    /// Play or pause every preview, including the ones that are not on screen.
    pub fn set_playing(&mut self, playing: bool) {
        self.playing = playing;
        let play_state = if playing {
            re_sdk_types::blueprint::components::PlayState::Playing
        } else {
            re_sdk_types::blueprint::components::PlayState::Paused
        };
        #[expect(clippy::iter_over_hash_type)] // Each preview is updated independently.
        for preview in self.active_previews.values_mut() {
            preview.play_override = None;
            preview
                .time_control
                .set_play_state(None, play_state, None::<&AppBlueprintCtx<'_>>);
        }
    }

    /// Register a recording as an active preview clip.
    ///
    /// Called each frame by the view renderer when a preview is shown.
    pub fn register_recording(
        &mut self,
        store_id: &StoreId,
        store_bundle: &re_entity_db::StoreBundle,
    ) {
        self.active_previews
            .entry(store_id.clone())
            .or_default()
            .rendered_last_frame = true;

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
            // Set `rendered_last_frame` to false again, it's set each
            // frame.
            if !std::mem::take(&mut active_preview.rendered_last_frame) {
                continue;
            }

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

    pub fn iter_active_previews(&self) -> impl Iterator<Item = (&StoreId, &ActivePreview)> {
        self.active_previews.iter()
    }

    pub fn iter_active_previews_mut(
        &mut self,
    ) -> impl Iterator<Item = (&StoreId, &mut ActivePreview)> {
        self.active_previews.iter_mut()
    }

    pub fn active_preview(&self, store_id: &StoreId) -> Option<&ActivePreview> {
        self.active_previews.get(store_id)
    }

    /// Whether one preview is playing, `None` if no such preview is registered.
    pub fn is_recording_playing(&self, store_id: &StoreId) -> Option<bool> {
        let play_state = self.active_preview(store_id)?.time_control.play_state();
        Some(play_state == re_sdk_types::blueprint::components::PlayState::Playing)
    }

    pub fn active_preview_mut(&mut self, store_id: &StoreId) -> Option<&mut ActivePreview> {
        self.active_previews.get_mut(store_id)
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

    /// Reports about each view as a whole.
    ///
    /// Each view replaces its reports after rendering, so title-bar UI reads the reports emitted during the preceding frame.
    view_reports: HashMap<ViewStateKey, Vec<ViewerDiagnostic>>,

    // TODO(isse): Should we have one preview state per table/dataset?
    /// Playback state shared across all preview recordings shown in grid/table cards.
    pub preview_state: Option<PreviewState>,
}

impl re_byte_size::MemUsageTreeCapture for ViewStates {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        let Self {
            states,
            visualizer_reports,
            view_reports,
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
        node.add("view_reports", view_reports.heap_size_bytes());
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

    /// Replaces the reports emitted by the view as a whole.
    pub fn set_view_reports(
        &mut self,
        store_id: &StoreId,
        view_id: ViewId,
        reports: Vec<ViewerDiagnostic>,
    ) {
        let key = (store_id.clone(), view_id);
        if reports.is_empty() {
            self.view_reports.remove(&key);
        } else {
            self.view_reports.insert(key, reports);
        }
    }

    /// Access the latest reports emitted by the view as a whole.
    pub fn view_reports(&self, store_id: &StoreId, view_id: ViewId) -> &[ViewerDiagnostic] {
        self.view_reports
            .get(&(store_id.clone(), view_id))
            .map_or(&[], Vec::as_slice)
    }

    /// Every diagnostic for a view: the view's own reports followed by those of its visualizers.
    pub fn all_reports(
        &self,
        store_id: &StoreId,
        view_id: ViewId,
    ) -> impl Iterator<Item = &ViewerDiagnostic> {
        let visualizer_reports = self
            .per_visualizer_type_reports(store_id, view_id)
            .into_iter()
            .flat_map(|reports| reports.values())
            .flat_map(VisualizerTypeReport::diagnostics);
        std::iter::chain(
            self.view_reports(store_id, view_id).iter(),
            visualizer_reports,
        )
    }
}
