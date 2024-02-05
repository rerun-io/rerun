use re_query_cache::{MaybeCachedComponentData, QueryError};
use re_types::archetypes;
use re_types::components::{MarkerShape, StrokeWidth};
use re_types::{
    archetypes::SeriesPoint,
    components::{Color, Radius, Scalar, Text},
    Archetype as _, ComponentNameSet, Loggable,
};
use re_viewer_context::{
    AnnotationMap, DefaultColor, IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewQuery,
    ViewerContext, VisualizerQueryInfo, VisualizerSystem,
};

use crate::overrides::initial_override_color;
use crate::util::{
    determine_plot_bounds_and_time_per_pixel, determine_time_range, points_to_series,
};
use crate::ScatterAttrs;
use crate::{overrides::lookup_override, PlotPoint, PlotPointAttrs, PlotSeries, PlotSeriesKind};

/// The system for rendering [`SeriesPoint`] archetypes.
#[derive(Default, Debug)]
pub struct SeriesPointSystem {
    pub annotation_map: AnnotationMap,
    pub all_series: Vec<PlotSeries>,
}

impl IdentifiedViewSystem for SeriesPointSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "SeriesPoint".into()
    }
}

// We use a larger default radius for scatter plots so the marker is
// visible.
const DEFAULT_RADIUS: f32 = 3.0;

impl VisualizerSystem for SeriesPointSystem {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        let mut query_info = VisualizerQueryInfo::from_archetype::<archetypes::Scalar>();
        let mut series_point_queried: ComponentNameSet = SeriesPoint::all_components()
            .iter()
            .map(ToOwned::to_owned)
            .collect::<ComponentNameSet>();
        query_info.queried.append(&mut series_point_queried);
        // TODO(jleibs): Use StrokeWidth instead
        query_info.queried.insert(Radius::name());
        query_info
    }

    fn execute(
        &mut self,
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
        _context: &re_viewer_context::ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        self.annotation_map.load(
            ctx,
            &query.latest_at_query(),
            query
                .iter_visible_data_results(Self::identifier())
                .map(|data| &data.entity_path),
        );

        match self.load_scalars(ctx, query) {
            Ok(_) | Err(QueryError::PrimaryNotFound(_)) => Ok(Vec::new()),
            Err(err) => Err(err.into()),
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn initial_override_value(
        &self,
        _ctx: &ViewerContext<'_>,
        _query: &re_data_store::LatestAtQuery,
        _store: &re_data_store::DataStore,
        entity_path: &re_log_types::EntityPath,
        component: &re_types::ComponentName,
    ) -> Option<re_log_types::DataCell> {
        if *component == Color::name() {
            Some([initial_override_color(entity_path)].into())
        } else if *component == Radius::name() {
            Some([Radius(DEFAULT_RADIUS)].into())
        } else {
            None
        }
    }
}

impl SeriesPointSystem {
    fn load_scalars(
        &mut self,
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
    ) -> Result<(), QueryError> {
        re_tracing::profile_function!();

        let query_caches = ctx.entity_db.query_caches();
        let store = ctx.entity_db.store();

        let (plot_bounds, time_per_pixel) = determine_plot_bounds_and_time_per_pixel(ctx, query);

        // TODO(cmc): this should be thread-pooled in case there are a gazillon series in the same plot…
        for data_result in query.iter_visible_data_results(Self::identifier()) {
            let mut points = Vec::new();

            let time_range = determine_time_range(
                query,
                data_result,
                plot_bounds,
                ctx.app_options.experimental_plot_query_clamping,
            );

            {
                re_tracing::profile_scope!("primary", &data_result.entity_path.to_string());

                let annotations = self.annotation_map.find(&data_result.entity_path);
                let annotation_info = annotations
                    .resolved_class_description(None)
                    .annotation_info();
                let default_color = DefaultColor::EntityPath(&data_result.entity_path);

                let override_color = lookup_override::<Color>(data_result, ctx).map(|c| {
                    let arr = c.to_array();
                    egui::Color32::from_rgba_unmultiplied(arr[0], arr[1], arr[2], arr[3])
                });

                let override_label = lookup_override::<Text>(data_result, ctx).map(|t| t.0);

                let override_radius = lookup_override::<Radius>(data_result, ctx).map(|r| r.0);

                let override_marker = lookup_override::<MarkerShape>(data_result, ctx);

                let query = re_data_store::RangeQuery::new(query.timeline, time_range);

                // TODO(jleibs): need to do a "joined" archetype query
                // The `Scalar` archetype queries for `StrokeWidth` in the line visualizer,
                // and so it must do so here also.
                // See https://github.com/rerun-io/rerun/pull/5029
                query_caches
                    .query_archetype_pov1_comp4::<archetypes::Scalar, Scalar, Color, StrokeWidth, MarkerShape, Text, _>(
                        ctx.app_options.experimental_primary_caching_range,
                        store,
                        &query.clone().into(),
                        &data_result.entity_path,
                        |((time, _row_id), _, scalars, colors, _, markers, labels)| {
                            let Some(time) = time else {
                                return;
                            }; // scalars cannot be timeless

                            // This is a clear: we want to split the chart.
                            if scalars.is_empty() {
                                points.push(PlotPoint {
                                    time: time.as_i64(),
                                    value: 0.0,
                                    attrs: PlotPointAttrs {
                                        label: None,
                                        color: egui::Color32::BLACK,
                                        stroke_width: 0.0,
                                        kind: PlotSeriesKind::Clear,
                                    },
                                });
                                return;
                            }

                            for (scalar, color, marker, label) in itertools::izip!(
                                scalars.iter(),
                                MaybeCachedComponentData::iter_or_repeat_opt(
                                    &colors,
                                    scalars.len()
                                ),
                                MaybeCachedComponentData::iter_or_repeat_opt(
                                    &markers,
                                    scalars.len()
                                ),
                                //MaybeCachedComponentData::iter_or_repeat_opt(&radii, scalars.len()),
                                MaybeCachedComponentData::iter_or_repeat_opt(
                                    &labels,
                                    scalars.len()
                                ),
                            ) {
                                // TODO(jleibs): Replace with StrokeWidth
                                let radius: Option<Radius> = None;
                                let color = override_color.unwrap_or_else(|| {
                                    annotation_info
                                        .color(color.map(|c| c.to_array()), default_color)
                                });
                                let label = override_label.clone().or_else(|| {
                                    annotation_info.label_utf8(label.as_deref().cloned())
                                });
                                let radius = override_radius
                                    .unwrap_or_else(|| radius.map_or(DEFAULT_RADIUS, |r| r.0));

                                let marker = override_marker.unwrap_or(marker.unwrap_or_default());

                                points.push(PlotPoint {
                                    time: time.as_i64(),
                                    value: scalar.0,
                                    attrs: PlotPointAttrs {
                                        label,
                                        color,
                                        stroke_width: radius,
                                        kind: PlotSeriesKind::Scatter(ScatterAttrs{marker}),
                                    },
                                });
                            }
                        },
                    )?;
            }

            // Now convert the `PlotPoints` into `Vec<PlotSeries>`
            points_to_series(
                data_result,
                time_per_pixel,
                points,
                store,
                query,
                &mut self.all_series,
            );
        }

        Ok(())
    }
}
