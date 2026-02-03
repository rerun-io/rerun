use re_log_types::hash::Hash64;
use re_sdk_types::blueprint::components::VisualizerInstructionId;

use crate::{BlueprintResolvedResults, BlueprintResolvedResultsExt as _, HybridResultsChunkIter};

/// Utility for processing queries while executing a visualizer instruction and reporting errors/warnings as they arise.
pub struct VisualizerInstructionQueryResults<'a> {
    pub instruction_id: VisualizerInstructionId,
    pub query_results: &'a BlueprintResolvedResults<'a>,
    pub output: &'a mut re_viewer_context::VisualizerExecutionOutput,
    pub timeline: re_log_types::TimelineName,
}

impl<'a> VisualizerInstructionQueryResults<'a> {
    /// Returns a zero-copy iterator over all the results for the given `(timeline, component)` pair.
    ///
    /// Reports an error if there's no chunks for the given component.
    /// Use this for required components where row IDs are needed for caching or identification.
    ///
    /// Blueprint row IDs are always discarded.
    ///
    /// Call one of the following methods on the returned [`HybridResultsChunkIter`]:
    /// * [`HybridResultsChunkIter::slice`]
    /// * [`HybridResultsChunkIter::slice_from_struct_field`]
    #[inline]
    pub fn iter_required(
        &mut self,
        component: re_sdk_types::ComponentIdentifier,
    ) -> HybridResultsChunkIter<'a> {
        self.query_results.iter_required(
            |err| self.output.report_error_for(self.instruction_id, err),
            self.timeline,
            component,
        )
    }

    /// Returns a zero-copy iterator over all the results for the given `(timeline, component)` pair.
    ///
    /// Use this for optional/recommended components where the original row IDs would otherwise
    /// interfere with range zipping on latest-at queries.
    ///
    /// **WARNING**: For latest-at queries, the row IDs are always zeroed out to allow for range zipping.
    /// Blueprint row IDs are always discarded.
    ///
    /// Call one of the following methods on the returned [`HybridResultsChunkIter`]:
    /// * [`HybridResultsChunkIter::slice`]
    /// * [`HybridResultsChunkIter::slice_from_struct_field`]
    #[inline]
    pub fn iter_optional(
        &mut self,
        component: re_sdk_types::ComponentIdentifier,
    ) -> HybridResultsChunkIter<'a> {
        self.query_results.iter_optional(
            |err| self.output.report_warning_for(self.instruction_id, err),
            self.timeline,
            component,
        )
    }

    #[inline]
    pub fn query_result_hash(&self) -> Hash64 {
        self.query_results.query_result_hash()
    }
}
