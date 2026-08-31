use rayon::prelude::*;
use re_sdk_types::components;
use re_sdk_types::{Archetype as _, archetypes};
use re_viewer_context::{
    IdentifiedViewSystem, SingleRequiredComponentConstraint, ViewContext, ViewQuery,
    ViewStateExt as _, ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo,
    VisualizerSystem,
};

use crate::PlotSeries;
use crate::line_series_loader::{
    LineSeriesSource, LineSeriesStyling, load_line_series_with_styling,
};
use crate::line_visualizer_system::build_line_draw_data;
use crate::util;

/// Output data from [`MeasurementsSeriesSystem`].
#[derive(Default, Clone)]
pub struct MeasurementsSeriesOutput {
    pub all_series: Vec<PlotSeries>,
}

/// The system for rendering [`archetypes::Measurements`] archetypes.
///
/// Draws each series as a line with an error band one standard deviation wide around it.
#[derive(Default, Debug)]
pub struct MeasurementsSeriesSystem;

impl IdentifiedViewSystem for MeasurementsSeriesSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        re_viewer_context::external::re_string_interner::intern_static!(
            re_viewer_context::ViewSystemIdentifier,
            "MeasurementsSeries"
        )
    }
}

impl VisualizerSystem for MeasurementsSeriesSystem {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo {
            relevant_archetype: archetypes::Measurements::name().into(),
            constraints: SingleRequiredComponentConstraint::new::<components::Scalar>(
                &archetypes::Measurements::descriptor_values(),
            )
            .with_additional_physical_types(util::series_supported_encodings())
            .with_allow_static_data(false)
            .into(),

            queried: archetypes::Measurements::all_components()
                .iter()
                .cloned()
                .collect(),
        }
    }

    fn execute(
        &self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        _context: &re_viewer_context::ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let output = VisualizerExecutionOutput::default();

        let time_per_pixel = ctx
            .view_state
            .downcast_ref::<crate::view_class::TimeSeriesViewState>()
            .map_or(1.0, |state| state.time_per_pixel);

        let data_results: Vec<_> = query
            .iter_visualizer_instruction_for(Self::identifier())
            .collect();

        let source = LineSeriesSource {
            value_descriptor: archetypes::Measurements::descriptor_values(),
            queried_components: archetypes::Measurements::all_component_identifiers().collect(),
            styling: LineSeriesStyling::measurements(),
            variance_descriptor: Some(archetypes::Measurements::descriptor_variances()),
            unit_descriptor: Some(archetypes::Measurements::descriptor_units()),
        };

        let all_series: Vec<_> = data_results
            .par_iter()
            .map(|(data_result, instruction)| {
                load_line_series_with_styling(
                    ctx,
                    query,
                    time_per_pixel,
                    data_result,
                    instruction,
                    &output,
                    &source,
                )
            })
            .collect();

        let mut all_series_flat = Vec::new();
        all_series_flat.extend(all_series.into_iter().flatten());

        let draw_data = build_line_draw_data(ctx, query, &all_series_flat)?;

        Ok(output
            .with_draw_data(draw_data)
            .with_visualizer_data(MeasurementsSeriesOutput {
                all_series: all_series_flat,
            }))
    }
}
