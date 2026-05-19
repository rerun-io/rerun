use re_chunk_store::AbsoluteTimeRange;
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::Status;
use re_sdk_types::components::Text;
use re_viewer_context::{
    AppOptions, IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, ViewSystemIdentifier, VisualizerExecutionOutput, VisualizerQueryInfo,
    VisualizerSystem,
};

use crate::data::{StatusLane, StatusLanePhase, StatusLanesData};

/// Color palette for status phases.
#[expect(clippy::disallowed_methods)] // These are data-driven visualization colors, not UI theme colors.
const PALETTE: &[egui::Color32] = &[
    egui::Color32::from_rgb(76, 175, 80),   // green
    egui::Color32::from_rgb(255, 183, 77),  // amber
    egui::Color32::from_rgb(66, 165, 245),  // blue
    egui::Color32::from_rgb(239, 83, 80),   // red
    egui::Color32::from_rgb(171, 71, 188),  // purple
    egui::Color32::from_rgb(38, 198, 218),  // teal
    egui::Color32::from_rgb(255, 241, 118), // yellow
    egui::Color32::from_rgb(141, 110, 99),  // brown
];

fn color_for_index(idx: usize) -> egui::Color32 {
    PALETTE[idx % PALETTE.len()]
}

/// A visualizer that queries [`Status`] archetypes and groups them into status lanes per entity.
///
/// Each entity path becomes one lane. Each distinct status value within a lane gets a unique color.
#[derive(Default)]
pub struct StatusVisualizer;

impl IdentifiedViewSystem for StatusVisualizer {
    fn identifier() -> ViewSystemIdentifier {
        "StatusVisualizer".into()
    }
}

impl VisualizerSystem for StatusVisualizer {
    fn visualizer_query_info(&self, _app_options: &AppOptions) -> VisualizerQueryInfo {
        VisualizerQueryInfo::single_required_component::<Text>(
            &Status::descriptor_status(),
            &Status::all_components(),
        )
    }

    fn execute(
        &self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let output = VisualizerExecutionOutput::default();
        let query =
            re_chunk_store::RangeQuery::new(view_query.timeline, AbsoluteTimeRange::EVERYTHING);

        let mut lanes: Vec<StatusLane> = Vec::new();

        for (data_result, instruction) in
            view_query.iter_visualizer_instruction_for(Self::identifier())
        {
            let range_results = re_view::range_with_blueprint_resolved_data(
                ctx,
                None,
                &query,
                data_result,
                Status::all_component_identifiers(),
                instruction,
            );

            let results = re_view::BlueprintResolvedResults::from((query.clone(), range_results));
            let results =
                re_view::VisualizerInstructionQueryResults::new(instruction, &results, &output);

            let all_texts = results.iter_required(Status::descriptor_status().component);
            if all_texts.is_empty() {
                continue;
            }

            // Collect (time, text) pairs.
            // A null status is a fallthrough, not a phase change: the preceding phase
            // must continue across it. `slice::<String>` represents null entries as
            // zero-length slices, so we skip empty texts here.
            let mut phases: Vec<(i64, String)> = Vec::new();
            for ((data_time, _row_id), texts) in all_texts.slice::<String>() {
                let time_value = data_time.as_i64();
                for text in texts {
                    if text.is_empty() {
                        continue;
                    }
                    phases.push((time_value, text.to_string()));
                }
            }

            if phases.is_empty() {
                continue;
            }

            phases.sort_by_key(|(t, _)| *t);

            // Collect unique labels for deterministic color assignment.
            let mut unique_labels: Vec<String> = Vec::new();
            for (_, label) in &phases {
                if !unique_labels.contains(label) {
                    unique_labels.push(label.clone());
                }
            }

            let lane = StatusLane {
                label: data_result.entity_path.to_string(),
                phases: phases
                    .into_iter()
                    .map(|(t, label)| {
                        let color_idx = unique_labels.iter().position(|l| l == &label).unwrap_or(0);
                        StatusLanePhase {
                            start_time: t,
                            label,
                            color: color_for_index(color_idx),
                        }
                    })
                    .collect(),
            };
            lanes.push(lane);
        }

        Ok(output.with_visualizer_data(StatusLanesData { lanes }))
    }
}
