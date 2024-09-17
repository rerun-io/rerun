use crate::{ViewContextCollection, VisualizerCollection};

/// Output of space view system execution.
pub struct SystemExecutionOutput {
    /// Executed view systems, may hold state that the ui method needs.
    pub view_systems: VisualizerCollection,

    /// Executed context systems, may hold state that the ui method needs.
    pub context_systems: ViewContextCollection,

    /// Draw data gathered during execution of the view part systems.
    ///
    /// Ui methods are supposed to use this to create [`re_renderer::ViewBuilder`]s.
    // _TODO(andreas)_: In the future view builder execution should be outside of the space view ui method.
    //                This would allow to run the wgpu command buffer buildup in parallel.
    //                (This implies that we'd pass out the readily built command buffer here instead of drawables.)
    pub draw_data: Vec<re_renderer::QueueableDrawData>,
}
