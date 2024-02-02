use re_query_cache::{MaybeCachedComponentData, QueryError};
use re_types::{
    archetypes::TimeSeriesScalar,
    components::{Color, Radius, Scalar, ScalarScattering, Text},
    Archetype, Loggable,
};
use re_viewer_context::{
    AnnotationMap, DefaultColor, IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewQuery,
    ViewerContext, VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    overrides::{initial_override_color, lookup_override},
    util::{determine_plot_bounds_and_time_per_pixel, determine_time_range, points_to_series},
    PlotPoint, PlotPointAttrs, PlotSeries, PlotSeriesKind, ScatterAttrs,
};

/// The legacy system for rendering [`TimeSeriesScalar`] archetypes.
#[derive(Default, Debug)]
pub struct LegacyTimeSeriesSystem {
    pub annotation_map: AnnotationMap,
    pub all_series: Vec<PlotSeries>,
}

impl IdentifiedViewSystem for LegacyTimeSeriesSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "LegacyTimeSeries".into()
    }
}

impl VisualizerSystem for LegacyTimeSeriesSystem {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        let mut query_info = VisualizerQueryInfo::from_archetype::<TimeSeriesScalar>();
        // Although we don't normally include indicator components in required components,
        // we don't want to show this legacy visualizer unless a user is actively using
        // the legacy archetype for their logging. Users just working with Scalar will
        // only see the new visualizer options.
        query_info
            .required
            .insert(TimeSeriesScalar::indicator().name());
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
        } else {
            None
        }
    }
}

impl LegacyTimeSeriesSystem {
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

                let override_label =
                    lookup_override::<Text>(data_result, ctx).map(|t| t.to_string());

                let override_scattered =
                    lookup_override::<ScalarScattering>(data_result, ctx).map(|s| s.0);

                let override_radius = lookup_override::<Radius>(data_result, ctx).map(|r| r.0);

                let query = re_data_store::RangeQuery::new(query.timeline, time_range);

                query_caches.query_archetype_pov1_comp4::<
                    TimeSeriesScalar,
                    Scalar,
                    ScalarScattering,
                    Color,
                    Radius,
                    Text,
                    _,
                >(
                    ctx.app_options.experimental_primary_caching_range,
                    store,
                    &query.clone().into(),
                    &data_result.entity_path,
                    |((time, _row_id), _, scalars, scatterings, colors, radii, labels)| {
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
                                    radius: 0.0,
                                    kind: PlotSeriesKind::Clear,
                                },
                            });
                            return;
                        }

                        for (scalar, scattered, color, radius, label) in itertools::izip!(
                            scalars.iter(),
                            MaybeCachedComponentData::iter_or_repeat_opt(&scatterings, scalars.len()),
                            MaybeCachedComponentData::iter_or_repeat_opt(&colors, scalars.len()),
                            MaybeCachedComponentData::iter_or_repeat_opt(&radii, scalars.len()),
                            MaybeCachedComponentData::iter_or_repeat_opt(&labels, scalars.len()),
                        ) {
                            let color = override_color.unwrap_or_else(|| {
                                annotation_info.color(color.map(|c| c.to_array()), default_color)
                            });
                            let label = override_label.clone().or_else(|| {
                                annotation_info.label(label.as_ref().map(|l| l.as_str()))
                            });
                            let scattered = override_scattered
                                .unwrap_or_else(|| scattered.map_or(false, |s| s.0));
                            let radius = override_radius
                                .unwrap_or_else(|| radius.map_or(DEFAULT_RADIUS, |r| r.0));

                            let kind= if scattered {
                                PlotSeriesKind::Scatter(ScatterAttrs::default())
                            } else {
                                PlotSeriesKind::Continuous
                            };

                            const DEFAULT_RADIUS: f32 = 0.75;

                            points.push(PlotPoint {
                                time: time.as_i64(),
                                value: scalar.0,
                                attrs: PlotPointAttrs {
                                    label,
                                    color,
                                    radius,
                                    kind,
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
