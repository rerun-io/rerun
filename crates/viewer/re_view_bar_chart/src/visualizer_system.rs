use std::collections::BTreeMap;

use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityPath;
use re_sdk_types::{
    archetypes::BarChart,
    components::{self, Length},
    datatypes,
};
use re_view::{
    BlueprintResolvedResultsExt as _, ComponentMappingError, DataResultQuery as _,
    clamped_vec_or_else,
};
use re_viewer_context::{
    IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery, ViewSystemExecutionError,
    VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem, typed_fallback_for,
};

#[derive(Default)]
pub struct BarChartData {
    pub abscissa: datatypes::TensorData,
    pub widths: Vec<f32>,
    pub values: datatypes::TensorData,
    pub color: components::Color,
}

/// A bar chart system, with everything needed to render it.
#[derive(Default)]
pub struct BarChartVisualizerSystem {
    pub charts: BTreeMap<EntityPath, BarChartData>,
}

impl IdentifiedViewSystem for BarChartVisualizerSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "BarChart".into()
    }
}

impl VisualizerSystem for BarChartVisualizerSystem {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<BarChart>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        let timeline_query = LatestAtQuery::new(view_query.timeline, view_query.latest_at);

        let mut output = VisualizerExecutionOutput::default();

        for (data_result, instruction) in
            view_query.iter_visualizer_instruction_for(Self::identifier())
        {
            let warning_reporter = |error: &ComponentMappingError| {
                output.report_warning_for(instruction.id, error);
            };

            let results = data_result.latest_at_with_blueprint_resolved_data::<BarChart>(
                ctx,
                &timeline_query,
                Some(instruction),
            );

            let Some(tensor) = results.get_required_mono::<components::TensorData>(
                BarChart::descriptor_values().component,
            ) else {
                continue;
            };

            if tensor.is_vector() {
                let length: u64 = tensor.shape().iter().product();

                let abscissa: components::TensorData =
                    results.get_mono_with_fallback(BarChart::descriptor_abscissa().component);
                let color = results.get_mono_with_fallback(BarChart::descriptor_color().component);
                let widths = results.iter_optional(
                    warning_reporter,
                    view_query.timeline,
                    BarChart::descriptor_widths().component,
                );
                let widths: &[f32] = widths
                    .slice::<f32>()
                    .next()
                    .map_or(&[], |((_time, _row), slice)| slice);

                let widths = clamped_vec_or_else(widths, length as usize, || {
                    typed_fallback_for::<Length>(
                        &ctx.query_context(data_result, &view_query.latest_at_query()),
                        BarChart::descriptor_widths().component,
                    )
                    .0
                    .into()
                });
                self.charts.insert(
                    data_result.entity_path.clone(),
                    BarChartData {
                        abscissa: abscissa.0.clone(),
                        values: tensor.0.clone(),
                        color,
                        widths: widths.into(),
                    },
                );
            }
        }

        Ok(output)
    }
}
