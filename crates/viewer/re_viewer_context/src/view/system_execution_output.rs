use std::sync::Arc;

use ahash::HashMap;
use re_sdk_types::blueprint::components::VisualizerInstructionId;
use vec1::Vec1;

use super::{VisualizerInstructionReport, VisualizerReportSeverity};
use crate::{
    PerVisualizerTypeInViewClass, ViewContextCollection, ViewSystemExecutionError,
    VisualizerCollection, VisualizerExecutionOutput, VisualizerReportContext,
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
pub enum VisualizerTypeReport {
    /// The entire visualizer failed to execute.
    OverallError(VisualizerInstructionReport),

    /// The visualizer executed, but had per-instruction reports (errors and warnings).
    PerInstructionReport(HashMap<VisualizerInstructionId, Vec1<VisualizerInstructionReport>>),
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
                if output.reports_per_instruction.is_empty() {
                    None
                } else {
                    Some(Self::PerInstructionReport(
                        output.reports_per_instruction.clone(),
                    ))
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
