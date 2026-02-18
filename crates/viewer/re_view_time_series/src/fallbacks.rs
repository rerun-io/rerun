use re_sdk_types::blueprint::archetypes::{PlotLegend, ScalarAxis, TimeAxis};
use re_sdk_types::datatypes::TimeRange;
use re_sdk_types::{
    archetypes::{SeriesLines, SeriesPoints},
    datatypes::TimeRangeBoundary,
};
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
        system_registry.register_array_fallback_provider::<re_sdk_types::components::Visible, _>(
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
        |ctx| -> re_sdk_types::blueprint::components::TimeRange {
            use re_chunk_store::TimeType;

            let timeline = ctx.viewer_ctx().time_ctrl.timeline();

            let recording_range = timeline
                .and_then(|timeline| ctx.viewer_ctx().recording().time_range_for(timeline.name()));

            let data_range = ctx
                .view_state()
                .as_any()
                .downcast_ref::<TimeSeriesViewState>()
                .map(|s| s.full_data_time_range)
                .filter(|range| !range.is_empty());

            // It is worth noting that the range of just the plot data (`data_range`)
            // may be smaller full range of ALL recording data (`recording_range`).

            if let Some(timeline) = timeline
                && let Some(data_range) = data_range
            {
                let span = data_range.abs_length();

                // When viewing large recordings (spanning hours), it is VERY important
                // that we only show part of the data by default, for two reasons:
                //
                // # Performance
                // If we show all the data, we need to collect and aggregate all the data. This can be VERY slow.
                //
                // # Legibility
                // A sufficiently zoomed out plot is indistinguishable from noise

                const NS_PER_SEC: i64 = 1_000_000_000;

                match timeline.typ() {
                    TimeType::Sequence => {
                        if 2_000 < span {
                            return TimeRange::from_cursor_plus_minus(1_000).into();
                        }
                    }
                    TimeType::TimestampNs | TimeType::DurationNs => {
                        if (60 * NS_PER_SEC as u64) < span {
                            return TimeRange::from_cursor_plus_minus(30 * NS_PER_SEC).into();
                        }
                    }
                }
            }

            // View the entire data_range:

            if let Some(data_range) = data_range
                && let Some(recording_range) = recording_range
            {
                TimeRange {
                    start: if data_range.min == recording_range.min {
                        TimeRangeBoundary::Infinite
                    } else {
                        TimeRangeBoundary::Absolute(data_range.min.into())
                    },

                    end: if data_range.max == recording_range.max {
                        TimeRangeBoundary::Infinite
                    } else {
                        TimeRangeBoundary::Absolute(data_range.max.into())
                    },
                }
            } else {
                TimeRange::EVERYTHING
            }
            .into()
        },
    );
}
