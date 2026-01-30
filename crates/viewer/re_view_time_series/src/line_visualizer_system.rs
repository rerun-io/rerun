use itertools::Itertools as _;
use re_chunk_store::{LatestAtQuery, RangeQuery, RowId};
use re_log_types::{EntityPath, TimeInt};
use re_sdk_types::components::{AggregationPolicy, Color, StrokeWidth};
use re_sdk_types::{Archetype as _, archetypes};
use re_sdk_types::{Component as _, components};
use re_view::{
    ChunksWithComponent, RangeResultsExt as _, latest_at_with_blueprint_resolved_data,
    range_with_blueprint_resolved_data,
};
use re_viewer_context::external::re_entity_db::InstancePath;
use re_viewer_context::{
    AnyPhysicalDatatypeRequirement, IdentifiedViewSystem, ViewContext, ViewQuery,
    ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
    typed_fallback_for,
};

use crate::series_query::{
    allocate_plot_points, collect_colors, collect_radius_ui, collect_scalars, collect_series_name,
    collect_series_visibility, determine_num_series,
};
use crate::{LoadSeriesError, PlotPoint, PlotPointAttrs, PlotSeries, PlotSeriesKind, util};

/// The system for rendering [`archetypes::SeriesLines`] archetypes.
#[derive(Default, Debug)]
pub struct SeriesLinesSystem {
    pub all_series: Vec<PlotSeries>,
}

impl IdentifiedViewSystem for SeriesLinesSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "SeriesLines".into()
    }
}

impl VisualizerSystem for SeriesLinesSystem {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo {
            relevant_archetype: archetypes::SeriesLines::name().into(),
            required: AnyPhysicalDatatypeRequirement {
                semantic_type: components::Scalar::name(),
                physical_types: util::series_supported_datatypes().into_iter().collect(),
                allow_static_data: false,
            }
            .into(),

            queried: archetypes::Scalars::all_components()
                .iter()
                .chain(archetypes::SeriesLines::all_components().iter())
                .cloned()
                .collect(),
        }
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        _context: &re_viewer_context::ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        self.load_scalars(ctx, query)
    }
}

impl SeriesLinesSystem {
    fn load_scalars(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        use rayon::prelude::*;

        let mut output = VisualizerExecutionOutput::default();

        let plot_mem =
            egui_plot::PlotMemory::load(ctx.viewer_ctx.egui_ctx(), crate::plot_id(query.view_id));
        let time_per_pixel = util::determine_time_per_pixel(ctx.viewer_ctx, plot_mem.as_ref());

        let data_results = query.iter_visualizer_instruction_for(Self::identifier());

        for result in data_results
            .collect_vec()
            .par_iter()
            .map(|(data_result, instruction)| {
                Self::load_series(ctx, query, time_per_pixel, data_result, instruction)
            })
            .collect::<Vec<_>>()
        {
            match result {
                Err(LoadSeriesError::ViewPropertyQuery(err)) => {
                    return Err(err.into());
                }
                Err(LoadSeriesError::InstructionSpecificVisualizerError {
                    instruction_id,
                    err,
                }) => {
                    output.report_error_for(instruction_id, err);
                }
                Ok(series) => {
                    self.all_series.extend(series);
                }
            }
        }

        Ok(output)
    }

    fn load_series(
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        time_per_pixel: f64,
        data_result: &re_viewer_context::DataResult,
        instruction: &re_viewer_context::VisualizerInstruction,
    ) -> Result<Vec<PlotSeries>, LoadSeriesError> {
        re_tracing::profile_function!();

        let current_query = ctx.current_query();
        let query_ctx = ctx.query_context(data_result, &current_query);

        let time_range = util::determine_time_range(ctx, data_result)?;

        {
            use re_view::RangeResultsExt as _;

            re_tracing::profile_scope!("primary", &data_result.entity_path.to_string());

            let entity_path = &data_result.entity_path;
            let query = re_chunk_store::RangeQuery::new(view_query.timeline, time_range)
                // We must fetch data with extended bounds, otherwise the query clamping would
                // cut-off the data early at the edge of the view.
                .include_extended_bounds(true);

            let results = range_with_blueprint_resolved_data(
                ctx,
                None,
                &query,
                data_result,
                archetypes::Scalars::all_component_identifiers()
                    .chain(archetypes::SeriesLines::all_component_identifiers()),
                instruction,
            );

            // If we have no scalars, we can't do anything.
            let all_scalar_chunks: ChunksWithComponent<'_> = results
                .get_required_chunk(archetypes::Scalars::descriptor_scalars().component)
                .try_into()
                .map_err(|err| LoadSeriesError::InstructionSpecificVisualizerError {
                    instruction_id: instruction.id,
                    err,
                })?;

            if all_scalar_chunks.is_empty() {
                return Err(LoadSeriesError::InstructionSpecificVisualizerError {
                    instruction_id: instruction.id,
                    err: "No valid scalar data found".to_owned(),
                });
            }

            // All the default values for a `PlotPoint`, accounting for both overrides and default values.
            // TODO(andreas): Fallback should produce several colors. Instead, we generate additional ones on the fly if necessary right now.
            let fallback_color: Color = typed_fallback_for(
                &query_ctx,
                archetypes::SeriesLines::descriptor_colors().component,
            );
            let fallback_stroke_width: StrokeWidth = typed_fallback_for(
                &query_ctx,
                archetypes::SeriesLines::descriptor_widths().component,
            );
            let default_point = PlotPoint {
                time: 0,
                value: 0.0,
                attrs: PlotPointAttrs {
                    color: fallback_color.into(),
                    radius_ui: 0.5 * *fallback_stroke_width.0,
                    kind: PlotSeriesKind::Continuous,
                },
            };

            let num_series = determine_num_series(&all_scalar_chunks);
            let mut points_per_series =
                allocate_plot_points(&query, &default_point, &all_scalar_chunks, num_series);

            collect_scalars(&all_scalar_chunks, &mut points_per_series);

            // The plot view visualizes scalar data within a specific time range, without any kind
            // of time-alignment / bootstrapping behavior:
            // * For the scalar themselves, this is what you want: if you're trying to plot some
            //   data between t=100 and t=200, you don't want to display a point from t=20 (and
            //   _extended bounds_ will take care of lines crossing the limit).
            // * For the secondary components (colors, radii, names, etc), this is a problem
            //   though: you don't want your plot to change color depending on what the currently
            //   visible time range is! Secondary components have to be bootstrapped.
            let bootstrapped_results = latest_at_with_blueprint_resolved_data(
                ctx,
                None,
                &LatestAtQuery::new(query.timeline, query.range.min()),
                data_result,
                archetypes::SeriesLines::all_component_identifiers(),
                Some(instruction),
            );

            collect_colors(
                entity_path,
                &query,
                &bootstrapped_results,
                &results,
                &all_scalar_chunks,
                &mut points_per_series,
                &archetypes::SeriesLines::descriptor_colors(),
            );
            collect_radius_ui(
                &query,
                &bootstrapped_results,
                &results,
                &all_scalar_chunks,
                &mut points_per_series,
                &archetypes::SeriesLines::descriptor_widths(),
                0.5,
            );

            // Now convert the `PlotPoints` into `Vec<PlotSeries>`
            let aggregation_policy_descr = archetypes::SeriesLines::descriptor_aggregation_policy();
            let aggregator = bootstrapped_results
                .get_optional_chunks(aggregation_policy_descr.component)
                .iter(|err| {
                    // TODO(RR-3506): This should be a visualizer warning instead!
                    re_log::warn_once!("could not retrieve aggregation policy: {err}");
                })
                .chain(
                    results
                        .get_optional_chunks(aggregation_policy_descr.component)
                        .iter(|err| {
                            // TODO(RR-3506): This should be a visualizer warning instead!
                            re_log::warn_once!("could not retrieve aggregation policy: {err}");
                        }),
                )
                .find(|chunk| !chunk.chunk.is_empty())
                .and_then(|chunk| {
                    chunk
                        .chunk
                        .component_mono::<AggregationPolicy>(chunk.component, 0)?
                        .ok()
                })
                // TODO(andreas): Relying on the default==placeholder here instead of going through a fallback provider.
                //                This is fine, because we know there's no `TypedFallbackProvider`, but wrong if one were to be added.
                .unwrap_or_default();

            // NOTE: The chunks themselves are already sorted as best as possible (hint: overlap)
            // by the query engine.
            let all_chunks_sorted_and_not_overlapped =
                all_scalar_chunks.iter().tuple_windows().all(|(lhs, rhs)| {
                    let lhs_time_max = lhs
                        .chunk
                        .timelines()
                        .get(query.timeline())
                        .map_or(TimeInt::MAX, |time_column| time_column.time_range().max());
                    let rhs_time_min = rhs
                        .chunk
                        .timelines()
                        .get(query.timeline())
                        .map_or(TimeInt::MIN, |time_column| time_column.time_range().min());
                    lhs_time_max <= rhs_time_min
                });

            let has_discontinuities = {
                // Find all clears that may apply, in order to render discontinuities properly.

                re_tracing::profile_scope!("discontinuities");

                let cleared_indices = collect_recursive_clears(ctx, &query, entity_path);
                let has_discontinuities = !cleared_indices.is_empty();

                for points in &mut points_per_series {
                    points.extend(cleared_indices.iter().map(|(data_time, _)| PlotPoint {
                        time: data_time.as_i64(),
                        value: 0.0,
                        attrs: PlotPointAttrs {
                            color: egui::Color32::TRANSPARENT,
                            radius_ui: 0.0,
                            kind: PlotSeriesKind::Clear,
                        },
                    }));
                }

                has_discontinuities
            };

            // This is _almost_ sorted already: all the individual chunks are sorted, but we still
            // have to deal with overlapped chunks, or discontinuities introduced by query-time clears.
            if !all_chunks_sorted_and_not_overlapped || has_discontinuities {
                re_tracing::profile_scope!("sort");
                for points in &mut points_per_series {
                    points.sort_by_key(|p| p.time);
                }
            }

            let series_visibility = collect_series_visibility(
                &query,
                &bootstrapped_results,
                &results,
                num_series,
                archetypes::SeriesLines::descriptor_visible_series().component,
            );
            let series_names = collect_series_name(
                &query_ctx,
                &bootstrapped_results,
                &results,
                num_series,
                &archetypes::SeriesLines::descriptor_names(),
            );

            let mut series = Vec::with_capacity(num_series);

            debug_assert_eq!(points_per_series.len(), series_names.len());
            for (instance, (points, label, visible)) in itertools::izip!(
                points_per_series.into_iter(),
                series_names.into_iter(),
                series_visibility.into_iter()
            )
            .enumerate()
            {
                let instance_path = if num_series == 1 {
                    InstancePath::entity_all(data_result.entity_path.clone())
                } else {
                    InstancePath::instance(data_result.entity_path.clone(), instance as u64)
                };

                util::points_to_series(
                    instance_path,
                    time_per_pixel,
                    visible,
                    points,
                    ctx.recording_engine().store(),
                    view_query,
                    label,
                    aggregator,
                    &mut series,
                    instruction.id,
                )
                .map_err(|err| {
                    LoadSeriesError::InstructionSpecificVisualizerError {
                        instruction_id: instruction.id,
                        err,
                    }
                })?;
            }

            Ok(series)
        }
    }
}

fn collect_recursive_clears(
    ctx: &ViewContext<'_>,
    query: &RangeQuery,
    entity_path: &EntityPath,
) -> Vec<(TimeInt, RowId)> {
    re_tracing::profile_function!();

    let mut cleared_indices = Vec::new();

    let mut clear_entity_path = entity_path.clone();
    let clear_descriptor = archetypes::Clear::descriptor_is_recursive();

    // Bootstrap in case there's a pending clear out of the visible time range.
    {
        let results = ctx.recording_engine().cache().latest_at(
            &LatestAtQuery::new(query.timeline, query.range.min()),
            &clear_entity_path,
            [clear_descriptor.component],
        );

        cleared_indices.extend(
            results
                .get_chunks(clear_descriptor.component, false)
                .iter()
                .flat_map(|chunk| {
                    itertools::izip!(
                        chunk.iter_component_indices(*query.timeline()),
                        chunk.iter_slices::<bool>()
                    )
                })
                .filter_map(|(index, is_recursive_buffer)| {
                    let is_recursive =
                        !is_recursive_buffer.is_empty() && is_recursive_buffer.value(0);
                    (is_recursive || clear_entity_path == *entity_path).then_some(index)
                }),
        );
    }

    loop {
        let results = ctx.recording_engine().cache().range(
            query,
            &clear_entity_path,
            [clear_descriptor.component],
        );

        cleared_indices.extend(
            results
                .get_chunks(clear_descriptor.component, false)
                .iter()
                .flat_map(|chunk| {
                    itertools::izip!(
                        chunk.iter_component_indices(*query.timeline()),
                        chunk.iter_slices::<bool>()
                    )
                })
                .filter_map(|(index, is_recursive_buffer)| {
                    let is_recursive =
                        !is_recursive_buffer.is_empty() && is_recursive_buffer.value(0);
                    (is_recursive || clear_entity_path == *entity_path).then_some(index)
                }),
        );

        let Some(parent_entity_path) = clear_entity_path.parent() else {
            break;
        };

        clear_entity_path = parent_entity_path;
    }

    cleared_indices
}
