use re_sdk_types::archetypes::{SeriesLines, SeriesPoints};
use re_sdk_types::blueprint::archetypes::{PlotLegend, ScalarAxis, TimeAxis};
use re_sdk_types::datatypes::TimeRange;
use re_viewer_context::ViewStateExt as _;

use crate::view_class::{TimeSeriesViewState, make_range_sane};

const MAX_NUM_TIME_SERIES_SHOWN_PER_ENTITY_BY_DEFAULT: usize = 20;
const MAX_NUM_ITEMS_IN_PLOT_LEGEND_BEFORE_HIDDEN: usize = 20;

/// Register fallback providers for TimeSeriesView-related components and view properties.
pub fn register_fallbacks(system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>) {
    for component in [
        SeriesLines::descriptor_names().component,
        SeriesPoints::descriptor_names().component,
    ] {
        system_registry.register_fallback_provider::<re_sdk_types::components::Name>(
            component,
            |ctx| {
                let state = ctx.view_state().downcast_ref::<TimeSeriesViewState>();

                state
                    .ok()
                    .and_then(|state| {
                        state
                            .default_names_for_entities
                            .get(ctx.target_entity_path)
                            .map(|name| name.clone().into())
                    })
                    .or_else(|| {
                        ctx.target_entity_path
                            .last()
                            .map(|part| part.ui_string().into())
                    })
                    .unwrap_or_default()
            },
        );
    }

    for component in [
        SeriesLines::descriptor_visible_series().component,
        SeriesPoints::descriptor_visible_series().component,
    ] {
        system_registry
            .register_array_fallback_provider::<re_sdk_types::components::SeriesVisible, _>(
                component,
                |ctx| {
                    let show_all = itertools::Either::Left(std::iter::once(true.into()));

                    let Some(time_series_state) = ctx
                        .view_state()
                        .as_any()
                        .downcast_ref::<TimeSeriesViewState>()
                    else {
                        return itertools::Either::Left(std::iter::once(true.into()));
                    };

                    // It's important to us to have the right count here at least for the simple case
                    // of a single visualizer on the entity, so that we don't show too many booleans
                    // (it does in fact not get the numbers right if we have multiple visualizers on the same entity)
                    let num_series = time_series_state
                        .num_time_series_last_frame_per_entity
                        .get(ctx.target_entity_path)
                        .map_or(0, |set| set.len());
                    let num_shown = num_series.min(MAX_NUM_TIME_SERIES_SHOWN_PER_ENTITY_BY_DEFAULT);
                    let num_hidden = num_series.saturating_sub(num_shown);

                    if num_hidden == 0 {
                        show_all // Prefer a single boolean if we can, it's nicer in the ui.
                    } else {
                        itertools::Either::Right(
                            std::iter::repeat_n(
                                true.into(),
                                MAX_NUM_TIME_SERIES_SHOWN_PER_ENTITY_BY_DEFAULT,
                            )
                            .chain(std::iter::repeat_n(false.into(), num_hidden)),
                        )
                    }
                },
            );
    }

    system_registry.register_fallback_provider(ScalarAxis::descriptor_range().component, |ctx| {
        ctx.view_state()
            .as_any()
            .downcast_ref::<TimeSeriesViewState>()
            .map(|s| make_range_sane(s.scalar_range))
            .unwrap_or_default()
    });

    system_registry.register_fallback_provider::<re_sdk_types::components::Visible>(
        PlotLegend::descriptor_visible().component,
        |ctx| {
            let Some(time_series_state) = ctx
                .view_state()
                .as_any()
                .downcast_ref::<TimeSeriesViewState>()
            else {
                return true.into();
            };

            // Don't show the plot legend if there's too many time series.
            // TODO(RR-2933): Once we can scroll it though it would be nice to show more!
            let total_num_series = time_series_state
                .num_time_series_last_frame_per_entity
                .values()
                .map(|set| set.len())
                .sum::<usize>();
            (total_num_series <= MAX_NUM_ITEMS_IN_PLOT_LEGEND_BEFORE_HIDDEN).into()
        },
    );

    system_registry.register_fallback_provider(
        TimeAxis::descriptor_view_range().component,
        |ctx| {
            let timeline_histograms = ctx.viewer_ctx().recording().timeline_histograms();
            let (timeline_min, timeline_max) = timeline_histograms
                .get(ctx.viewer_ctx().time_ctrl.timeline_name())
                .and_then(|stats| Some((stats.min_opt()?, stats.max_opt()?)))
                .unzip();
            ctx.view_state()
                .as_any()
                .downcast_ref::<TimeSeriesViewState>()
                .map(|s| {
                    re_sdk_types::blueprint::components::TimeRange(TimeRange {
                        start: if Some(s.max_time_view_range.min) == timeline_min {
                            re_sdk_types::datatypes::TimeRangeBoundary::Infinite
                        } else {
                            re_sdk_types::datatypes::TimeRangeBoundary::Absolute(
                                s.max_time_view_range.min.into(),
                            )
                        },
                        end: if Some(s.max_time_view_range.max) == timeline_max {
                            re_sdk_types::datatypes::TimeRangeBoundary::Infinite
                        } else {
                            re_sdk_types::datatypes::TimeRangeBoundary::Absolute(
                                s.max_time_view_range.max.into(),
                            )
                        },
                    })
                })
                .unwrap_or(re_sdk_types::blueprint::components::TimeRange(TimeRange {
                    start: re_sdk_types::datatypes::TimeRangeBoundary::Infinite,
                    end: re_sdk_types::datatypes::TimeRangeBoundary::Infinite,
                }))
        },
    );
}
