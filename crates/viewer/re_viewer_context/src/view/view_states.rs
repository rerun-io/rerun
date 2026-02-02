//! Storage for the state of each `View`.
//!
//! The `Viewer` has ownership of this state and pass it around to users (mainly viewport and
//! selection panel).

use ahash::HashMap;

use crate::{
    PerVisualizerType, SystemExecutionOutput, ViewClass, ViewId, ViewState,
    VisualizerExecutionErrorState,
};

/// State for the `View`s that persists across frames but otherwise
/// is not saved.
#[derive(Default)]
pub struct ViewStates {
    states: HashMap<ViewId, Box<dyn ViewState>>,

    /// List of all errors that occurred in visualizers of this view.
    ///
    /// This is cleared out each frame and populated after all visualizers have been executed.
    // TODO(andreas): Would be nice to bundle this with `ViewState` by making `ViewState` a struct containing errors & generic data.
    // But at point of writing this causes too much needless churn.
    visualizer_errors: HashMap<ViewId, PerVisualizerType<VisualizerExecutionErrorState>>,
}

impl re_byte_size::SizeBytes for ViewStates {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            states,
            visualizer_errors,
        } = self;
        states
            .iter()
            .map(|(key, state)| key.total_size_bytes() + state.size_bytes())
            .sum::<u64>()
            + visualizer_errors.heap_size_bytes()
    }
}

impl ViewStates {
    pub fn get(&self, view_id: ViewId) -> Option<&dyn ViewState> {
        self.states.get(&view_id).map(|s| s.as_ref())
    }

    pub fn get_mut_or_create(
        &mut self,
        view_id: ViewId,
        view_class: &dyn ViewClass,
    ) -> &mut dyn ViewState {
        self.states
            .entry(view_id)
            .or_insert_with(|| view_class.new_state())
            .as_mut()
    }

    pub fn ensure_state_exists(&mut self, view_id: ViewId, view_class: &dyn ViewClass) {
        self.states
            .entry(view_id)
            .or_insert_with(|| view_class.new_state());
    }

    /// Removes all previously stored visualizer errors.
    pub fn reset_visualizer_errors(&mut self) {
        self.visualizer_errors.clear();
    }

    /// Reports visualizer errors from a system execution output for a given view.
    pub fn report_visualizer_errors(
        &mut self,
        view_id: ViewId,
        system_output: &SystemExecutionOutput,
    ) {
        let per_visualizer_errors = &mut self.visualizer_errors.entry(view_id).or_default().0;

        per_visualizer_errors.extend(system_output.visualizer_execution_output.iter().filter_map(
            |(id, result)| {
                VisualizerExecutionErrorState::from_result(result).map(|error| (*id, error))
            },
        ));
    }

    /// Access latest visualizer errors for a given view.
    pub fn visualizer_errors(
        &self,
        view_id: ViewId,
    ) -> Option<&PerVisualizerType<VisualizerExecutionErrorState>> {
        self.visualizer_errors.get(&view_id)
    }
}
