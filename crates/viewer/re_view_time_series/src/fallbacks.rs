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
        system_registry.register_array_fallback_provider::<re_sdk_types::components::Name, _>(
            component,
            |ctx| {
                // If no instruction_id, fall back to entity name (e.g., for UI queries)
                let (Ok(state), Some(instruction_id)) = (
                    ctx.view_state().downcast_ref::<TimeSeriesViewState>(),
                    ctx.instruction_id,
                ) else {
                    return vec![
                        ctx.target_entity_path
                            .last()
                            .map(|part| part.ui_string().into())
                            .unwrap_or_default(),
                    ];
                };

                let Some(fallback_name) = state.default_series_name_formats.get(&instruction_id)
                else {
                    return vec![
                        ctx.target_entity_path
                            .last()
                            .map(|part| part.ui_string().into())
                            .unwrap_or_default(),
                    ];
                };

                let num_series = ctx
                    .instruction_id
                    .and_then(|id| state.num_time_series_last_frame_per_instruction.get(&id))
                    .map_or(1, |set| set.len());

                let mut series_names = Vec::new();

                if num_series == 1 {
                    series_names.push(fallback_name.clone().into());
                } else {
                    // Repeating a name never makes sense, so we fill up the remaining names with made up ones instead.
                    // Selectors that return a `ListArray` end with `[]`. In those cases we inject the series number.
                    let fallback_name = fallback_name.strip_suffix("[]").unwrap_or(fallback_name);

                    series_names.extend(
                        (series_names.len()..num_series)
                            .map(|i| format!("{fallback_name}[{i}]").into()),
                    );
                }

                series_names
            },
        );
    }

    for component in [
        SeriesLines::descriptor_colors().component,
        SeriesPoints::descriptor_colors().component,
    ] {
        system_registry.register_array_fallback_provider::<re_sdk_types::components::Color, _>(
            component,
            |ctx| {
                let state = ctx.view_state().downcast_ref::<TimeSeriesViewState>();
                let Ok(state) = state else {
                    return vec![re_viewer_context::auto_color_for_entity_path(
                        ctx.target_entity_path,
                    )];
                };

                let num_series = ctx
                    .instruction_id
                    .and_then(|id| state.num_time_series_last_frame_per_instruction.get(&id))
                    .map_or(1, |set| set.len());

                (0..num_series)
                    .map(|i| {
                        let hash = re_log_types::hash::Hash64::hash((ctx.instruction_id, i))
                            .hash64()
                            % u16::MAX as u64;
                        re_viewer_context::auto_color_egui(hash as u16).into()
                    })
                    .collect()
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

                    // Get the number of series for this specific instruction
                    let num_series = ctx
                        .instruction_id
                        .and_then(|id| {
                            time_series_state
                                .num_time_series_last_frame_per_instruction
                                .get(&id)
                        })
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
                .num_time_series_last_frame_per_instruction
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
