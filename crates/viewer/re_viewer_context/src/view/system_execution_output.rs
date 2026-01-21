use nohash_hasher::IntMap;
use re_chunk::EntityPath;

use crate::{
    PerVisualizerInViewClass, ViewContextCollection, ViewSystemExecutionError,
    VisualizerCollection, VisualizerExecutionOutput,
};

/// Output of view system execution.
pub struct SystemExecutionOutput {
    /// Executed view systems, may hold state that the ui method needs.
    pub view_systems: VisualizerCollection,

    /// Executed context systems, may hold state that the ui method needs.
    pub context_systems: ViewContextCollection,

    /// Result of all visualizer executions for this view.
    pub visualizer_execution_output: PerVisualizerInViewClass<
        Result<VisualizerExecutionOutput, std::sync::Arc<ViewSystemExecutionError>>,
    >,
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

/// Errors that occurred during the execution of a visualizer.
///
/// For convenience, the actual execution method of visualizer is using a `Result` type,
/// but this enum is more suited for storing errors throughout a frame.
#[derive(Clone, Debug)]
pub enum VisualizerExecutionErrorState {
    /// The entire visualizer failed to execute.
    Overall(std::sync::Arc<ViewSystemExecutionError>),

    /// The visualizer executed, but had per-entity errors.
    PerEntity(IntMap<EntityPath, String>),
}

impl re_byte_size::SizeBytes for VisualizerExecutionErrorState {
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Self::Overall(_err) => 0, // assume small and/or rare
            Self::PerEntity(errors) => errors.heap_size_bytes(),
        }
    }
}

impl VisualizerExecutionErrorState {
    pub fn from_result(
        result: &Result<VisualizerExecutionOutput, std::sync::Arc<ViewSystemExecutionError>>,
    ) -> Option<Self> {
        match result {
            Ok(output) => {
                if output.errors_per_entity.is_empty() {
                    None
                } else {
                    Some(Self::PerEntity(output.errors_per_entity.clone()))
                }
            }
            Err(err) => Some(Self::Overall(err.clone())),
        }
    }

    pub fn error_string_for(&self, path: &EntityPath) -> Option<String> {
        match self {
            Self::Overall(err) => Some(re_error::format_ref(&err)),
            Self::PerEntity(errors) => errors.get(path).cloned(),
        }
    }

    pub fn is_overall(&self) -> bool {
        matches!(self, Self::Overall(_))
    }
}
