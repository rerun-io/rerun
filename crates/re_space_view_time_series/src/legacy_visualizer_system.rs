use itertools::Itertools;
use re_query_cache::QueryError;
use re_types::{
    components::{Color, Radius, Scalar, ScalarScattering, Text},
    Archetype, Loggable,
};
use re_viewer_context::{
    AnnotationMap, DefaultColor, IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewQuery,
    ViewerContext, VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    overrides::initial_override_color,
    util::{determine_plot_bounds_and_time_per_pixel, determine_time_range, points_to_series},
    PlotPoint, PlotPointAttrs, PlotSeries, PlotSeriesKind, ScatterAttrs,
};

#[allow(deprecated)]
use re_types::archetypes::TimeSeriesScalar;

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
    #[allow(deprecated)]
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
                .iter_visible_data_results(ctx, Self::identifier())
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
    #[allow(deprecated)]
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
        for data_result in query.iter_visible_data_results(ctx, Self::identifier()) {
            let annotations = self.annotation_map.find(&data_result.entity_path);
            let annotation_info = annotations
                .resolved_class_description(None)
                .annotation_info();
            let default_color = DefaultColor::EntityPath(&data_result.entity_path);

            const DEFAULT_RADIUS: f32 = 0.75;

            let override_color = data_result
                .lookup_override::<Color>(ctx)
                .map(|c| c.to_array());
            let override_label = data_result.lookup_override::<Text>(ctx).map(|t| t.0);
            let override_scattered = data_result
                .lookup_override::<ScalarScattering>(ctx)
                .map(|s| s.0);
            let override_radius = data_result.lookup_override::<Radius>(ctx).map(|r| r.0);

            // All the default values for a `PlotPoint`, accounting for both overrides and default
            // values.
            let default_point = PlotPoint {
                time: 0,
                value: 0.0,
                attrs: PlotPointAttrs {
                    label: override_label.clone(), // default value is simply None
                    color: annotation_info.color(override_color, default_color),
                    marker_size: override_radius.unwrap_or(DEFAULT_RADIUS),
                    kind: if override_scattered.unwrap_or(false) {
                        PlotSeriesKind::Scatter(ScatterAttrs::default())
                    } else {
                        PlotSeriesKind::Continuous
                    },
                },
            };

            let mut points = Vec::new();

            let time_range = determine_time_range(
                ctx,
                query,
                data_result,
                plot_bounds,
                ctx.app_options.experimental_plot_query_clamping,
            );

            {
                re_tracing::profile_scope!("primary", &data_result.entity_path.to_string());

                let entity_path = &data_result.entity_path;
                let query = re_data_store::RangeQuery::new(query.timeline, time_range);

                query_caches.query_archetype_range_pov1_comp4::<
                    TimeSeriesScalar,
                    Scalar,
                    ScalarScattering,
                    Color,
                    Radius,
                    Text,
                    _,
                >(
                    store,
                    &query,
                    entity_path,
                    |_timeless, entry_range, (times, _, scalars, scatterings, colors, radii, labels)| {
                        let times = times.range(entry_range.clone()).map(|(time, _)| time.as_i64());

                        // Allocate all points.
                        points = times.map(|time| PlotPoint {
                            time,
                            ..default_point.clone()
                        }).collect_vec();

                        // Fill in values.
                        for (i, scalar) in scalars.range(entry_range.clone()).enumerate() {
                            if scalar.len() > 1 {
                                re_log::warn_once!("found a scalar batch in {entity_path:?} -- those have no effect");
                            } else if scalar.is_empty() {
                                points[i].attrs.kind = PlotSeriesKind::Clear;
                            } else {
                                points[i].value = scalar.first().map_or(0.0, |s| s.0);
                            }
                        }

                        // Make it as clear as possible to the optimizer that some parameters
                        // go completely unused as soon as overrides have been defined.

                        // Fill in series kind -- if available _and_ not overridden.
                        if override_scattered.is_none() {
                            if let Some(scatterings) = scatterings {
                                for (i, scattered) in scatterings.range(entry_range.clone()).enumerate() {
                                    if i >= points.len() {
                                        re_log::debug_once!("more scattered attributes than points in {entity_path:?} -- this points to a bug in the query cache");
                                        break;
                                    }
                                    if scattered.first().copied().flatten().map_or(false, |s| s.0) {
                                        points[i].attrs.kind  = PlotSeriesKind::Scatter(ScatterAttrs::default());
                                    };
                                }
                            }
                        }

                        // Fill in colors -- if available _and_ not overridden.
                        if override_color.is_none() {
                            if let Some(colors) = colors {
                                for (i, color) in colors.range(entry_range.clone()).enumerate() {
                                    if i >= points.len() {
                                        re_log::debug_once!("more color attributes than points in {entity_path:?} -- this points to a bug in the query cache");
                                        break;
                                    }
                                    if let Some(color) = color.first().copied().flatten().map(|c| {
                                        let [r,g,b,a] = c.to_array();
                                        if a == 255 {
                                            // Common-case optimization
                                            re_renderer::Color32::from_rgb(r, g, b)
                                        } else {
                                            re_renderer::Color32::from_rgba_unmultiplied(r, g, b, a)
                                        }
                                    }) {
                                        points[i].attrs.color = color;
                                    }
                                }
                            }
                        }

                        // Fill in radii -- if available _and_ not overridden.
                        if override_radius.is_none() {
                            if let Some(radii) = radii {
                                for (i, radius) in radii.range(entry_range.clone()).enumerate() {
                                    if i >= radii.num_entries() {
                                        re_log::debug_once!("more radius attributes than points in {entity_path:?} -- this points to a bug in the query cache");
                                        break;
                                    }
                                    if let Some(radius) = radius.first().copied().flatten().map(|r| r.0) {
                                        points[i].attrs.marker_size = radius;
                                    }
                                }
                            }
                        }

                        // Fill in labels -- if available _and_ not overridden.
                        if override_label.is_none() {
                            if let Some(labels) = labels {
                                for (i, label) in labels.range(entry_range.clone()).enumerate() {
                                    if i >= labels.num_entries() {
                                        re_log::debug_once!("more label attributes than points in {entity_path:?} -- this points to a bug in the query cache");
                                        break;
                                    }
                                    if let Some(label) = label.first().cloned().flatten().map(|l| l.0) {
                                        points[i].attrs.label = Some(label);
                                    }
                                }
                            }
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
                None, // Legacy visualizer labels its scalars, not the series.
                &mut self.all_series,
            );
        }

        Ok(())
    }
}
