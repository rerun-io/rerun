use re_log_types::hash::Hash64;
use re_sdk_types::ComponentIdentifier;
use re_sdk_types::blueprint::components::VisualizerInstructionId;
use re_viewer_context::{
    QueryContext, VisualizerInstruction, VisualizerInstructionReport, VisualizerReportContext,
    VisualizerReportSeverity,
};

use crate::{
    BlueprintResolvedResults, BlueprintResolvedResultsExt as _, ChunksWithComponent,
    ComponentMappingError, HybridResultsChunkIter,
};

/// Utility for processing queries while executing a visualizer instruction and reporting errors/warnings as they arise.
pub struct VisualizerInstructionQueryResults<'a> {
    instruction: &'a VisualizerInstruction,
    query_results: &'a BlueprintResolvedResults<'a>,
    output: &'a re_viewer_context::VisualizerExecutionOutput,
}

impl<'a> VisualizerInstructionQueryResults<'a> {
    /// Create a new query results wrapper.
    pub fn new(
        instruction: &'a VisualizerInstruction,
        query_results: &'a BlueprintResolvedResults<'a>,
        output: &'a re_viewer_context::VisualizerExecutionOutput,
    ) -> Self {
        if query_results.any_missing_chunks() {
            output.set_missing_chunks();
        }
        Self {
            instruction,
            query_results,
            output,
        }
    }

    /// The visualizer instruction ID these results are associated with.
    pub fn instruction_id(&self) -> VisualizerInstructionId {
        self.instruction.id
    }

    /// Whether the given component has an identity mapping on this visualizer instruction.
    ///
    /// Identity means the component maps directly to itself with no selector,
    /// which is also the default when no explicit mapping is present.
    pub fn has_identity_mapping_for_component(&self, component: ComponentIdentifier) -> bool {
        self.instruction
            .component_mappings
            .get(&component)
            .is_none_or(|source| source.is_identity_mapping(component))
    }

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
        &self,
        component: re_sdk_types::ComponentIdentifier,
    ) -> HybridResultsChunkIter<'a> {
        let chunks_with_component = match ChunksWithComponent::try_from(
            self.query_results.get_required_chunks(component),
        ) {
            Ok(chunks) => chunks,
            Err(err) => {
                // Don't report an error when the component is just still loading or simply not in our range.
                if !matches!(
                    err,
                    ComponentMappingError::NoComponentDataForQuery(_)
                        | ComponentMappingError::NoComponentDataForQueryButIsFetchable(_)
                ) {
                    let report = VisualizerInstructionReport {
                        severity: VisualizerReportSeverity::Error,
                        context: VisualizerReportContext {
                            component: Some(component),
                            extra: None,
                        },
                        summary: err.summary(),
                        details: err.details(),
                    };

                    self.output.report(self.instruction.id, report);
                }

                ChunksWithComponent::empty(component)
            }
        };

        HybridResultsChunkIter::new(chunks_with_component, self.query_results.timeline())
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
        &self,
        component: re_sdk_types::ComponentIdentifier,
    ) -> HybridResultsChunkIter<'a> {
        let chunks_with_component = match ChunksWithComponent::try_from(
            self.query_results.get_optional_chunks(component),
        ) {
            Ok(chunks) => chunks,
            Err(err) => {
                let report = VisualizerInstructionReport {
                    severity: VisualizerReportSeverity::Warning,
                    context: VisualizerReportContext {
                        component: Some(component),
                        extra: None,
                    },
                    summary: err.summary(),
                    details: err.details(),
                };

                self.output.report(self.instruction.id, report);
                ChunksWithComponent::empty(component)
            }
        };

        HybridResultsChunkIter::new(chunks_with_component, self.query_results.timeline())
    }

    #[inline]
    pub fn query_result_hash(&self) -> Hash64 {
        self.query_results.query_result_hash()
    }

    pub fn report_unspecified_source(
        &self,
        severity: VisualizerReportSeverity,
        message: impl Into<String>,
    ) {
        self.output
            .report_unspecified_source(self.instruction.id, severity, message);
    }

    /// Report a diagnostic tied to a specific component.
    pub fn report_for_component(
        &self,
        component: re_sdk_types::ComponentIdentifier,
        severity: VisualizerReportSeverity,
        summary: impl Into<String>,
    ) {
        self.output.report(
            self.instruction.id,
            VisualizerInstructionReport {
                severity,
                context: VisualizerReportContext {
                    component: Some(component),
                    extra: None,
                },
                summary: summary.into(),
                details: None,
            },
        );
    }

    /// Returns the [`QueryContext`] for this result.
    #[inline]
    pub fn query_context(&self) -> &QueryContext<'_> {
        self.query_results.query_context()
    }

    /// Returns the target entity path for this result.
    #[inline]
    pub fn entity_path(&self) -> &re_log_types::EntityPath {
        self.query_results.entity_path()
    }
}
