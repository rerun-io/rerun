use re_chunk::RowId;
use re_log_types::hash::Hash64;
use re_sdk_types::ComponentIdentifier;
use re_sdk_types::blueprint::components::VisualizerInstructionId;
use re_viewer_context::{
    QueryContext, ViewerDiagnostic, ViewerReportSeverity, VisualizerComponentSource,
    VisualizerInstruction, VisualizerInstructionReport, VisualizerReportContext,
};

use crate::{
    BlueprintResolvedResults, BlueprintResolvedResultsExt as _, ChunksWithComponent,
    HybridResultsChunkIter,
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

    /// Identifies the component and its source mapping, including any selector, for data caching.
    pub fn component_source_hash(&self, component: ComponentIdentifier) -> Hash64 {
        Hash64::hash((
            component,
            self.instruction.component_mappings.get(&component),
        ))
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
                if !err.is_data_temporarily_unavailable() {
                    let report = VisualizerInstructionReport {
                        diagnostic: ViewerDiagnostic {
                            // Missing a **required** component is always a full error.
                            severity: ViewerReportSeverity::Error,
                            summary: err.summary(),
                            details: err.details(),
                        },
                        context: VisualizerReportContext {
                            component: Some(component),
                            extra: None,
                        },
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
                    diagnostic: ViewerDiagnostic {
                        severity: ViewerReportSeverity::Warning,
                        summary: err.summary(),
                        details: err.details(),
                    },
                    context: VisualizerReportContext {
                        component: Some(component),
                        extra: None,
                    },
                };

                self.output.report(self.instruction.id, report);
                ChunksWithComponent::empty(component)
            }
        };

        HybridResultsChunkIter::new(chunks_with_component, self.query_results.timeline())
    }

    /// Returns the value of `component` together with the [`RowId`] of the blueprint chunk holding it,
    /// if the component resolves to a blueprint override or view default.
    ///
    /// The iterators discard blueprint row IDs, this keeps it as a cache key for what is derived from the value.
    ///
    /// Reports a warning if the value can't be deserialized.
    pub fn blueprint_mono_with_row_id<C: re_types_core::Component>(
        &self,
        component: ComponentIdentifier,
    ) -> Option<(RowId, C)> {
        let (component_sources, overrides, view_defaults) = match self.query_results {
            BlueprintResolvedResults::LatestAt(_, results) => (
                &results.component_sources,
                &results.overrides,
                results.view_defaults,
            ),
            BlueprintResolvedResults::Range(_, results) => (
                &results.component_sources,
                &results.overrides,
                results.view_defaults,
            ),
        };
        let checked_source = component_sources.get(&component)?;
        if checked_source.error().is_some() {
            return None;
        }
        let unit_chunk = match checked_source.source() {
            VisualizerComponentSource::Override => overrides.get(component)?,
            VisualizerComponentSource::Default => view_defaults.get(component)?,
            VisualizerComponentSource::AnnotationContext
            | VisualizerComponentSource::SourceComponent { .. } => return None,
        };

        match unit_chunk.component_mono::<C>(component)? {
            Ok(value) => Some((unit_chunk.row_id()?, value)),
            Err(err) => {
                self.report_for_component(
                    component,
                    ViewerReportSeverity::Warning,
                    format!("{err:#}"),
                );
                None
            }
        }
    }

    #[inline]
    pub fn query_result_hash(&self) -> Hash64 {
        self.query_results.query_result_hash()
    }

    pub fn report_unspecified_source(
        &self,
        severity: ViewerReportSeverity,
        message: impl Into<String>,
    ) {
        self.output
            .report_unspecified_source(self.instruction.id, severity, message);
    }

    /// Report a diagnostic tied to a specific component.
    pub fn report_for_component(
        &self,
        component: re_sdk_types::ComponentIdentifier,
        severity: ViewerReportSeverity,
        summary: impl Into<String>,
    ) {
        self.output.report(
            self.instruction.id,
            VisualizerInstructionReport {
                diagnostic: ViewerDiagnostic {
                    severity,
                    summary: summary.into(),
                    details: None,
                },
                context: VisualizerReportContext {
                    component: Some(component),
                    extra: None,
                },
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
