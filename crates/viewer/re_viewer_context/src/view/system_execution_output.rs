use std::collections::BTreeMap;
use std::sync::Arc;

use re_sdk_types::blueprint::components::VisualizerInstructionId;
use vec1::Vec1;

use super::{VisualizerInstructionReport, VisualizerReportSeverity};
use crate::{
    PerVisualizerTypeInViewClass, ViewContextCollection, ViewSystemExecutionError,
    ViewSystemIdentifier, VisualizerCollection, VisualizerExecutionOutput, VisualizerReportContext,
};

/// Output of view system execution.
pub struct SystemExecutionOutput {
    /// Executed view systems, may hold state that the ui method needs.
    pub view_systems: VisualizerCollection,

    /// Executed context systems, may hold state that the ui method needs.
    pub context_systems: ViewContextCollection,

    /// Result of all visualizer executions for this view.
    pub visualizer_execution_output: PerVisualizerTypeInViewClass<
        Result<VisualizerExecutionOutput, Arc<ViewSystemExecutionError>>,
    >,
}

impl SystemExecutionOutput {
    /// Were any required chunks missing?
    ///
    /// If so, we should probably show a loading indicator.
    pub fn any_missing_chunks(&self) -> bool {
        self.visualizer_execution_output
            .per_visualizer
            .values()
            .filter_map(|result| result.as_ref().ok())
            .any(|output| output.any_missing_chunks())
    }

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

/// Visualizer errors, grouped by view system.
///
/// In a `BTreeMap` to ensure stable sorting.
pub type VisualizerViewReport = BTreeMap<ViewSystemIdentifier, VisualizerTypeReport>;

/// Diagnostics from executing a single visualizer type within a view.
///
/// For a high-level failure handling overview, see the `re_viewer` crate documentation.
#[derive(Clone, Debug)]
pub enum VisualizerTypeReport {
    /// The entire visualizer type failed to execute for this view.
    ///
    /// For example, "point cloud rendering broke down completely".
    /// This is rare and almost always a bug in the Viewer itself.
    /// (So rare, in fact, that today we sometimes lump these together with per-instruction
    /// errors.)
    OverallError(VisualizerInstructionReport),

    /// The visualizer executed, but produced per-instruction reports (errors and/or warnings).
    ///
    /// Keyed by instruction (≈ entity), each entry lists one or more
    /// [`VisualizerInstructionReport`]s. These are somewhat common, practically never infect
    /// other entities, and are often not completely fatal.
    PerInstructionReport(BTreeMap<VisualizerInstructionId, Vec1<VisualizerInstructionReport>>),
}

impl re_byte_size::SizeBytes for VisualizerTypeReport {
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Self::OverallError(_err) => 0, // assume small and/or rare
            Self::PerInstructionReport(reports) => reports.heap_size_bytes(),
        }
    }
}

impl VisualizerTypeReport {
    pub fn from_result(
        result: &Result<VisualizerExecutionOutput, Arc<ViewSystemExecutionError>>,
    ) -> Option<Self> {
        match result {
            Ok(output) => {
                let reports = output.reports_per_instruction.lock();
                if reports.is_empty() {
                    None
                } else {
                    Some(Self::PerInstructionReport(reports.clone()))
                }
            }

            Err(err) => Some(Self::OverallError(VisualizerInstructionReport {
                severity: VisualizerReportSeverity::Error,
                context: VisualizerReportContext {
                    component: None,
                    extra: None,
                },
                summary: re_error::format_ref(err),
                details: None,
            })),
        }
    }

    /// Get all reports for a specific instruction.
    ///
    /// Does **not** include the overall error.
    pub fn reports_for(
        &self,
        instruction_id: &VisualizerInstructionId,
    ) -> impl Iterator<Item = &VisualizerInstructionReport> {
        match self {
            Self::OverallError(report) => itertools::Either::Left(std::iter::once(report)),
            Self::PerInstructionReport(reports) => itertools::Either::Right(
                reports
                    .get(instruction_id)
                    .map_or([].as_slice(), |r| r.as_slice())
                    .iter(),
            ),
        }
    }

    /// Get reports for a specific instruction that are associated with a specific component.
    pub fn reports_for_component(
        &self,
        instruction_id: &VisualizerInstructionId,
        component: re_chunk::ComponentIdentifier,
    ) -> impl Iterator<Item = &VisualizerInstructionReport> {
        self.reports_for(instruction_id)
            .filter(move |report| report.context.component == Some(component))
    }

    /// Get reports for a specific instruction that are NOT associated with any component.
    pub fn reports_without_component(
        &self,
        instruction_id: &VisualizerInstructionId,
    ) -> impl Iterator<Item = &VisualizerInstructionReport> {
        self.reports_for(instruction_id)
            .filter(|report| report.context.component.is_none())
    }

    /// Get the highest severity report for an instruction.
    pub fn highest_severity_for(
        &self,
        instruction_id: &VisualizerInstructionId,
    ) -> Option<VisualizerReportSeverity> {
        match self {
            Self::OverallError(_) => Some(VisualizerReportSeverity::Error),
            Self::PerInstructionReport(reports) => reports
                .get(instruction_id)?
                .iter()
                .map(|r| r.severity)
                .max(),
        }
    }
}
