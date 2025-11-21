use crate::{
    PerVisualizerInView, ViewContextCollection, ViewSystemExecutionError, VisualizerCollection,
    VisualizerExecutionOutput,
};

/// Output of view system execution.
pub struct SystemExecutionOutput {
    /// Executed view systems, may hold state that the ui method needs.
    pub view_systems: VisualizerCollection,

    /// Executed context systems, may hold state that the ui method needs.
    pub context_systems: ViewContextCollection,

    /// Result of all visualizer executions for this view.
    pub visualizer_execution_output:
        PerVisualizerInView<Result<VisualizerExecutionOutput, ViewSystemExecutionError>>,
}

impl SystemExecutionOutput {
    /// Removes & returns all successfully created draw data from all visualizer executions.
    pub fn drain_draw_data(&mut self) -> impl Iterator<Item = re_renderer::QueueableDrawData> {
        self.visualizer_execution_output
            .per_visualizer
            .iter_mut()
            .filter_map(|(_, result)| {
                result
                    .as_mut()
                    .ok()
                    .map(|output| output.draw_data.drain(..))
            })
            .flatten()
    }
}
