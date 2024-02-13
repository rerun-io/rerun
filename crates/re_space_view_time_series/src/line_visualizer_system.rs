use itertools::Itertools as _;
use re_query_cache::QueryError;
use re_types::archetypes;
use re_types::{
    archetypes::SeriesLine,
    components::{Color, MarkerShape, MarkerSize, Name, Scalar, StrokeWidth},
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
use crate::{overrides::lookup_override, PlotPoint, PlotPointAttrs, PlotSeries, PlotSeriesKind};

/// The system for rendering [`SeriesLine`] archetypes.
#[derive(Default, Debug)]
pub struct SeriesLineSystem {
    pub annotation_map: AnnotationMap,
    pub all_series: Vec<PlotSeries>,
}

impl IdentifiedViewSystem for SeriesLineSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "SeriesLine".into()
    }
}

const DEFAULT_STROKE_WIDTH: f32 = 0.75;

impl VisualizerSystem for SeriesLineSystem {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        let mut query_info = VisualizerQueryInfo::from_archetype::<archetypes::Scalar>();
        let mut series_line_queried: ComponentNameSet = SeriesLine::all_components()
            .iter()
            .map(ToOwned::to_owned)
            .collect::<ComponentNameSet>();
        query_info.queried.append(&mut series_line_queried);
        query_info.indicators = std::iter::once(SeriesLine::indicator().name()).collect();
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
        } else if *component == StrokeWidth::name() {
            Some([StrokeWidth(DEFAULT_STROKE_WIDTH)].into())
        } else {
            None
        }
    }
}

impl SeriesLineSystem {
    fn load_scalars(
        &mut self,
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
    ) -> Result<(), QueryError> {
        re_tracing::profile_function!();

        let (plot_bounds, time_per_pixel) = determine_plot_bounds_and_time_per_pixel(ctx, query);

        let data_results = query.iter_visible_data_results(Self::identifier());

        let parallel_loading = false; // TODO(emilk): enable parallel loading when it is faster, because right now it is often slower.
        if parallel_loading {
            use rayon::prelude::*;
            re_tracing::profile_wait!("load_series");
            for one_series in data_results
                .collect_vec()
                .par_iter()
                .map(|data_result| -> Result<Vec<PlotSeries>, QueryError> {
                    let annotations = self.annotation_map.find(&data_result.entity_path);
                    let mut series = vec![];
                    load_series(
                        ctx,
                        query,
                        plot_bounds,
                        time_per_pixel,
                        &annotations,
                        data_result,
                        &mut series,
                    )?;
                    Ok(series)
                })
                .collect::<Vec<Result<_, _>>>()
            {
                self.all_series.append(&mut one_series?);
            }
        } else {
            for data_result in data_results {
                let annotations = self.annotation_map.find(&data_result.entity_path);
                load_series(
                    ctx,
                    query,
                    plot_bounds,
                    time_per_pixel,
                    &annotations,
                    data_result,
                    &mut self.all_series,
                )?;
            }
        }

        Ok(())
    }
}

fn load_series(
    ctx: &ViewerContext<'_>,
    query: &ViewQuery<'_>,
    plot_bounds: Option<egui_plot::PlotBounds>,
    time_per_pixel: f64,
    annotations: &re_viewer_context::Annotations,
    data_result: &re_viewer_context::DataResult,
    all_series: &mut Vec<PlotSeries>,
) -> Result<(), QueryError> {
    re_tracing::profile_function!();

    let store = ctx.entity_db.store();
    let query_caches = ctx.entity_db.query_caches();

    let annotation_info = annotations
        .resolved_class_description(None)
        .annotation_info();
    let default_color = DefaultColor::EntityPath(&data_result.entity_path);
    let override_color = lookup_override::<Color>(data_result, ctx).map(|c| c.to_array());
    let override_series_name = lookup_override::<Name>(data_result, ctx).map(|t| t.0);
    let override_stroke_width = lookup_override::<StrokeWidth>(data_result, ctx).map(|r| r.0);

    // All the default values for a `PlotPoint`, accounting for both overrides and default
    // values.
    let default_point = PlotPoint {
        time: 0,
        value: 0.0,
        attrs: PlotPointAttrs {
            label: None,
            color: annotation_info.color(override_color, default_color),
            marker_size: override_stroke_width.unwrap_or(DEFAULT_STROKE_WIDTH),
            kind: PlotSeriesKind::Continuous,
        },
    };

    let mut points = Vec::new();

    let time_range = determine_time_range(
        query,
        data_result,
        plot_bounds,
        ctx.app_options.experimental_plot_query_clamping,
    );
    {
        re_tracing::profile_scope!("primary", &data_result.entity_path.to_string());

        let entity_path = &data_result.entity_path;
        let query = re_data_store::RangeQuery::new(query.timeline, time_range);

        // TODO(jleibs): need to do a "joined" archetype query
        // The `Scalar` archetype queries for `MarkerShape` & `MarkerSize` in the point visualizer,
        // and so it must do so here also.
        // See https://github.com/rerun-io/rerun/pull/5029
        query_caches.query_archetype_range_pov1_comp4::<
            archetypes::Scalar,
            Scalar,
            Color,
            StrokeWidth,
            MarkerSize, // unused
            MarkerShape, // unused
            _,
        >(
            store,
            &query,
            entity_path,
            |_timeless, entry_range, (times, _, scalars, colors, stroke_widths, _, _)| {
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
                if override_stroke_width.is_none() {
                    if let Some(stroke_widths) = stroke_widths {
                        for (i, stroke_width) in stroke_widths.range(entry_range.clone()).enumerate() {
                            if i >= stroke_widths.num_entries() {
                                re_log::debug_once!("more stroke width attributes than points in {entity_path:?} -- this points to a bug in the query cache");
                                break;
                            }
                            if let Some(stroke_width) = stroke_width.first().copied().flatten().map(|r| r.0) {
                                points[i].attrs.marker_size = stroke_width;
                            }
                        }
                    }
                }
            },
        )?;
    }

    // Check for an explicit label if any.
    // We're using a separate latest-at query for this since the semantics for labels changing over time are a
    // a bit unclear.
    // Sidestepping the cache here shouldn't be a problem since we do so only once per entity.
    let series_name = if let Some(override_name) = override_series_name {
        Some(override_name)
    } else {
        ctx.entity_db
            .store()
            .query_latest_component::<Name>(&data_result.entity_path, &ctx.current_query())
            .map(|name| name.value.0)
    };

    // Now convert the `PlotPoints` into `Vec<PlotSeries>`
    points_to_series(
        data_result,
        time_per_pixel,
        points,
        store,
        query,
        series_name,
        all_series,
    );
    Ok(())
}
