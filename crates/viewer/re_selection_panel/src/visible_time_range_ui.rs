use std::collections::HashSet;

use egui::{NumExt as _, Ui};

use re_log_types::{EntityPath, ResolvedTimeRange, TimeType, TimelineName};
use re_space_view_spatial::{SpatialSpaceView2D, SpatialSpaceView3D};
use re_space_view_time_series::TimeSeriesSpaceView;
use re_types::{
    blueprint::components::VisibleTimeRange,
    datatypes::{TimeInt, TimeRange, TimeRangeBoundary},
    Archetype, SpaceViewClassIdentifier,
};
use re_ui::UiExt as _;
use re_viewer_context::{QueryRange, SpaceViewClass, SpaceViewState, TimeDragValue, ViewerContext};
use re_viewport_blueprint::{entity_path_for_view_property, SpaceViewBlueprint};

/// These space views support the Visible History feature.
static VISIBLE_HISTORY_SUPPORTED_SPACE_VIEWS: once_cell::sync::Lazy<
    HashSet<SpaceViewClassIdentifier>,
> = once_cell::sync::Lazy::new(|| {
    [
        SpatialSpaceView3D::identifier(),
        SpatialSpaceView2D::identifier(),
        TimeSeriesSpaceView::identifier(),
        // TODO(#7876): replace with `MapSpaceView::identifier()` when we get rid of the cargo feature
        "Map".into(),
    ]
    .map(Into::into)
    .into()
});

// TODO(#4145): This method is obviously unfortunate. It's a temporary solution until the Visualizer
// system is able to report its ability to handle the visible history feature.
fn space_view_with_visible_history(space_view_class: SpaceViewClassIdentifier) -> bool {
    VISIBLE_HISTORY_SUPPORTED_SPACE_VIEWS.contains(&space_view_class)
}

pub fn visible_time_range_ui_for_view(
    ctx: &ViewerContext<'_>,
    ui: &mut Ui,
    view: &SpaceViewBlueprint,
    view_state: &dyn SpaceViewState,
) {
    if !space_view_with_visible_history(view.class_identifier()) {
        return;
    }

    let property_path = entity_path_for_view_property(
        view.id,
        ctx.store_context.blueprint.tree(),
        re_types::blueprint::archetypes::VisibleTimeRanges::name(),
    );

    let query_range = view.query_range(
        ctx.store_context.blueprint,
        ctx.blueprint_query,
        ctx.rec_cfg.time_ctrl.read().timeline(),
        ctx.space_view_class_registry,
        view_state,
    );

    let is_space_view = true;
    visible_time_range_ui(ctx, ui, query_range, &property_path, is_space_view);
}

pub fn visible_time_range_ui_for_data_result(
    ctx: &ViewerContext<'_>,
    ui: &mut Ui,
    data_result: &re_viewer_context::DataResult,
) {
    let override_path = data_result.recursive_override_path();
    let query_range = data_result.property_overrides.query_range.clone();

    let is_space_view = false;
    visible_time_range_ui(ctx, ui, query_range, override_path, is_space_view);
}

/// Draws ui for a visible time range from a given override path and a resulting query range.
fn visible_time_range_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut Ui,
    mut resolved_query_range: QueryRange,
    time_range_override_path: &EntityPath,
    is_space_view: bool,
) {
    use re_types::Loggable as _;

    let ranges = ctx
        .blueprint_db()
        .latest_at(
            ctx.blueprint_query,
            time_range_override_path,
            std::iter::once(VisibleTimeRange::name()),
        )
        .component_batch::<VisibleTimeRange>()
        .unwrap_or_default();
    let visible_time_ranges = re_types::blueprint::archetypes::VisibleTimeRanges { ranges };

    let timeline_name = *ctx.rec_cfg.time_ctrl.read().timeline().name();
    let mut has_individual_range = visible_time_ranges
        .range_for_timeline(timeline_name.as_str())
        .is_some();

    let has_individual_range_before = has_individual_range;
    let query_range_before = resolved_query_range.clone();

    ui.scope(|ui| {
        // TODO(#6075): Because `list_item_scope` changes it. Temporary until everything is `ListItem`.
        ui.spacing_mut().item_spacing.y = ui.ctx().style().spacing.item_spacing.y;
        query_range_ui(
            ctx,
            ui,
            &mut resolved_query_range,
            &mut has_individual_range,
            is_space_view,
        );
    });

    if query_range_before != resolved_query_range
        || has_individual_range_before != has_individual_range
    {
        save_visible_time_ranges(
            ctx,
            &timeline_name,
            has_individual_range,
            resolved_query_range,
            time_range_override_path,
            visible_time_ranges,
        );
    }
}

fn save_visible_time_ranges(
    ctx: &ViewerContext<'_>,
    timeline_name: &TimelineName,
    has_individual_range: bool,
    query_range: QueryRange,
    property_path: &EntityPath,
    mut visible_time_ranges: re_types::blueprint::archetypes::VisibleTimeRanges,
) {
    if has_individual_range {
        let time_range = match query_range {
            QueryRange::TimeRange(time_range) => time_range,
            QueryRange::LatestAt => {
                re_log::error!(
                    "Latest-at queries can't be used as an override yet. They can only \
                come from defaults."
                );
                return;
            }
        };
        visible_time_ranges.set_range_for_timeline(timeline_name, Some(time_range));
    } else {
        visible_time_ranges.set_range_for_timeline(timeline_name, None);
    }

    ctx.save_blueprint_archetype(property_path, &visible_time_ranges);
}

/// Draws ui for showing and configuring a query range.
fn query_range_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut Ui,
    query_range: &mut QueryRange,
    has_individual_time_range: &mut bool,
    is_space_view: bool,
) {
    let time_ctrl = ctx.rec_cfg.time_ctrl.read().clone();
    let time_type = time_ctrl.timeline().typ();

    let mut interacting_with_controls = false;
    let markdown = "# Visible time range\n
This feature controls the time range used to display data in the space view.

Notes:
- The settings are inherited from the parent entity or enclosing space view if not overridden.
- Visible time range properties are stored on a per-timeline basis.
- The data current as of the time range starting time is included.";

    let collapsing_response = ui
        .section_collapsing_header("Visible time range")
        .default_open(true)
        .help_markdown(markdown)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.re_radio_value(has_individual_time_range, false, "Default")
                    .on_hover_text(if is_space_view {
                        "Default query range settings for this kind of space view"
                    } else {
                        "Query range settings inherited from parent entity or enclosing \
                        space view"
                    });
                ui.re_radio_value(has_individual_time_range, true, "Override")
                    .on_hover_text(if is_space_view {
                        "Set query range settings for the contents of this space view"
                    } else {
                        "Set query range settings for this entity"
                    });
            });
            let time_drag_value =
                if let Some(times) = ctx.recording().time_histogram(time_ctrl.timeline()) {
                    TimeDragValue::from_time_histogram(times)
                } else {
                    TimeDragValue::from_time_range(0..=0)
                };

            let current_time = TimeInt(
                time_ctrl
                    .time_i64()
                    .unwrap_or_default()
                    .at_least(*time_drag_value.range.start()),
            ); // accounts for timeless time (TimeInt::MIN)

            if *has_individual_time_range {
                let time_range = match query_range {
                    QueryRange::TimeRange(time_range) => time_range,
                    QueryRange::LatestAt => {
                        // This should only happen if we just flipped to an individual range and the parent used latest-at queries.
                        *query_range = QueryRange::TimeRange(TimeRange::AT_CURSOR);
                        match query_range {
                            QueryRange::TimeRange(range) => range,
                            QueryRange::LatestAt => unreachable!(),
                        }
                    }
                };

                time_range_editor(
                    ctx,
                    ui,
                    time_range,
                    current_time,
                    &mut interacting_with_controls,
                    time_type,
                    &time_drag_value,
                );
            } else {
                match &query_range {
                    QueryRange::TimeRange(range) => {
                        show_visual_time_range(ctx, ui, range, time_type, current_time);
                    }
                    QueryRange::LatestAt => {
                        let current_time =
                            time_type.format(current_time, ctx.app_options.time_zone);
                        ui.label(format!("Latest-at query at: {current_time}"))
                            .on_hover_text("Uses the latest known value for each component.");
                    }
                }
            }
        });

    // Decide when to show the visible history highlight in the timeline. The trick is that when
    // interacting with the controls, the mouse might end up outside the collapsing header rect,
    // so we must track these interactions specifically.
    // Note: visible history highlight is always reset at the beginning of the Selection Panel UI.

    let should_display_visible_time_range = interacting_with_controls
        || collapsing_response.header_response.hovered()
        || collapsing_response
            .body_response
            .map_or(false, |r| r.hovered());

    if should_display_visible_time_range {
        if let Some(current_time) = time_ctrl.time_int() {
            if let QueryRange::TimeRange(ref time_range) = &query_range {
                let absolute_time_range =
                    ResolvedTimeRange::from_relative_time_range(time_range, current_time);
                ctx.rec_cfg.time_ctrl.write().highlighted_range = Some(absolute_time_range);
            }
        }
    }
}

fn time_range_editor(
    ctx: &ViewerContext<'_>,
    ui: &mut Ui,
    resolved_range: &mut TimeRange,
    current_time: TimeInt,
    interacting_with_controls: &mut bool,
    time_type: TimeType,
    time_drag_value: &TimeDragValue,
) {
    let current_start = resolved_range.start.start_boundary_time(current_time);
    let current_end = resolved_range.end.end_boundary_time(current_time);

    egui::Grid::new("from_to_editable").show(ui, |ui| {
        ui.grid_left_hand_label("Start");
        *interacting_with_controls |= ui
            .horizontal(|ui| {
                visible_history_boundary_ui(
                    ctx,
                    ui,
                    &mut resolved_range.start,
                    time_type,
                    current_time,
                    time_drag_value,
                    true,
                    current_end,
                )
            })
            .inner;
        ui.end_row();

        ui.grid_left_hand_label("End");
        *interacting_with_controls |= ui
            .horizontal(|ui| {
                visible_history_boundary_ui(
                    ctx,
                    ui,
                    &mut resolved_range.end,
                    time_type,
                    current_time,
                    time_drag_value,
                    false,
                    current_start,
                )
            })
            .inner;
        ui.end_row();
    });

    current_range_ui(ctx, ui, current_time, time_type, resolved_range);
}

fn show_visual_time_range(
    ctx: &ViewerContext<'_>,
    ui: &mut Ui,
    resolved_range: &TimeRange,
    time_type: TimeType,
    current_time: TimeInt,
) {
    // Show the resolved visible range as labels (user can't edit them):
    if resolved_range.start == TimeRangeBoundary::Infinite
        && resolved_range.end == TimeRangeBoundary::Infinite
    {
        ui.label("Entire timeline");
    } else if resolved_range.start == TimeRangeBoundary::AT_CURSOR
        && resolved_range.end == TimeRangeBoundary::AT_CURSOR
    {
        let current_time = time_type.format(current_time, ctx.app_options.time_zone);
        match time_type {
            TimeType::Time => {
                ui.label(format!("At current time: {current_time}"))
            }
            TimeType::Sequence => {
                ui.label(format!("At current frame: {current_time}"))
            }
        }.on_hover_text("Does not perform a latest-at query, shows only data logged at exactly the current time cursor position.");
    } else {
        egui::Grid::new("from_to_labels").show(ui, |ui| {
            ui.grid_left_hand_label("From");
            resolved_visible_history_boundary_ui(ctx, ui, &resolved_range.start, time_type, true);
            ui.end_row();

            ui.grid_left_hand_label("To");
            resolved_visible_history_boundary_ui(ctx, ui, &resolved_range.end, time_type, false);
            ui.end_row();
        });

        current_range_ui(ctx, ui, current_time, time_type, resolved_range);
    }
}

fn current_range_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut Ui,
    current_time: TimeInt,
    time_type: TimeType,
    time_range: &TimeRange,
) {
    let absolute_range = ResolvedTimeRange::from_relative_time_range(time_range, current_time);
    let from_formatted = time_type.format(absolute_range.min(), ctx.app_options.time_zone);
    let to_formatted = time_type.format(absolute_range.max(), ctx.app_options.time_zone);

    ui.label(format!("{from_formatted} to {to_formatted}"))
        .on_hover_text("Showing data in this range (inclusive).");
}

#[allow(clippy::too_many_arguments)]
fn resolved_visible_history_boundary_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    visible_history_boundary: &TimeRangeBoundary,
    time_type: TimeType,
    low_bound: bool,
) {
    let boundary_type = match visible_history_boundary {
        TimeRangeBoundary::CursorRelative(_) => match time_type {
            TimeType::Time => "current time",
            TimeType::Sequence => "current frame",
        },
        TimeRangeBoundary::Absolute(_) => match time_type {
            TimeType::Time => "absolute time",
            TimeType::Sequence => "frame",
        },
        TimeRangeBoundary::Infinite => {
            if low_bound {
                "beginning of timeline"
            } else {
                "end of timeline"
            }
        }
    };

    let mut label = boundary_type.to_owned();

    match visible_history_boundary {
        TimeRangeBoundary::CursorRelative(offset) => {
            let offset = offset.0;
            if offset != 0 {
                match time_type {
                    TimeType::Time => {
                        // This looks like it should be generically handled somewhere like re_format,
                        // but this actually is rather ad hoc and works thanks to egui::DragValue
                        // biasing towards round numbers and the auto-scaling feature of
                        // `time_drag_value()`.
                        let (unit, factor) = if offset % 1_000_000_000 == 0 {
                            ("s", 1_000_000_000.)
                        } else if offset % 1_000_000 == 0 {
                            ("ms", 1_000_000.)
                        } else if offset % 1_000 == 0 {
                            ("μs", 1_000.)
                        } else {
                            ("ns", 1.)
                        };

                        label += &format!(" with {} {} offset", offset as f64 / factor, unit);
                    }
                    TimeType::Sequence => {
                        label += &format!(
                            " with {offset} frame{} offset",
                            if offset.abs() > 1 { "s" } else { "" }
                        );
                    }
                }
            }
        }
        TimeRangeBoundary::Absolute(time) => {
            label += &format!(" {}", time_type.format(*time, ctx.app_options.time_zone));
        }
        TimeRangeBoundary::Infinite => {}
    }

    ui.label(label);
}

fn visible_history_boundary_combo_label(
    boundary: TimeRangeBoundary,
    time_type: TimeType,
    low_bound: bool,
) -> &'static str {
    match boundary {
        TimeRangeBoundary::CursorRelative(_) => match time_type {
            TimeType::Time => "current time with offset",
            TimeType::Sequence => "current frame with offset",
        },
        TimeRangeBoundary::Absolute(_) => match time_type {
            TimeType::Time => "absolute time",
            TimeType::Sequence => "absolute frame",
        },
        TimeRangeBoundary::Infinite => {
            if low_bound {
                "beginning of timeline"
            } else {
                "end of timeline"
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn visible_history_boundary_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    visible_history_boundary: &mut TimeRangeBoundary,
    time_type: TimeType,
    current_time: TimeInt,
    time_drag_value: &TimeDragValue,
    low_bound: bool,
    other_boundary_absolute: TimeInt,
) -> bool {
    let (abs_time, rel_time) = match *visible_history_boundary {
        TimeRangeBoundary::CursorRelative(time) => (time + current_time, time),
        TimeRangeBoundary::Absolute(time) => (time, time - current_time),
        TimeRangeBoundary::Infinite => (current_time, TimeInt(0)),
    };
    let abs_time = TimeRangeBoundary::Absolute(abs_time);
    let rel_time = TimeRangeBoundary::CursorRelative(rel_time);

    egui::ComboBox::from_id_salt(if low_bound {
        "time_history_low_bound"
    } else {
        "time_history_high_bound"
    })
    .selected_text(visible_history_boundary_combo_label(
        *visible_history_boundary,
        time_type,
        low_bound,
    ))
    .show_ui(ui, |ui| {
        ui.selectable_value(
            visible_history_boundary,
            rel_time,
            visible_history_boundary_combo_label(rel_time, time_type, low_bound),
        )
        .on_hover_text(if low_bound {
            "Show data from a time point relative to the current time."
        } else {
            "Show data until a time point relative to the current time."
        });
        ui.selectable_value(
            visible_history_boundary,
            abs_time,
            visible_history_boundary_combo_label(abs_time, time_type, low_bound),
        )
        .on_hover_text(if low_bound {
            "Show data from an absolute time point."
        } else {
            "Show data until an absolute time point."
        });
        ui.selectable_value(
            visible_history_boundary,
            TimeRangeBoundary::Infinite,
            visible_history_boundary_combo_label(TimeRangeBoundary::Infinite, time_type, low_bound),
        )
        .on_hover_text(if low_bound {
            "Show data from the beginning of the timeline"
        } else {
            "Show data until the end of the timeline"
        });
    });

    // Note: the time range adjustment below makes sure the two boundaries don't cross in time
    // (i.e. low > high). It does so by prioritizing the low boundary. Moving the low boundary
    // against the high boundary will displace the high boundary. On the other hand, the high
    // boundary cannot be moved against the low boundary. This asymmetry is intentional, and avoids
    // both boundaries fighting each other in some corner cases (when the user interacts with the
    // current time cursor)

    let response = match visible_history_boundary {
        TimeRangeBoundary::CursorRelative(value) => {
            // see note above
            let low_bound_override = if low_bound {
                None
            } else {
                Some((other_boundary_absolute - current_time).into())
            };

            let mut edit_value = (*value).into();
            let response = match time_type {
                TimeType::Time => Some(
                    time_drag_value
                        .temporal_drag_value_ui(
                            ui,
                            &mut edit_value,
                            false,
                            low_bound_override,
                            ctx.app_options.time_zone,
                        )
                        .0
                        .on_hover_text(
                            "Time duration before/after the current time to use as time range \
                                boundary",
                        ),
                ),
                TimeType::Sequence => Some(
                    time_drag_value
                        .sequence_drag_value_ui(ui, &mut edit_value, false, low_bound_override)
                        .on_hover_text(
                            "Number of frames before/after the current time to use a time \
                        range boundary",
                        ),
                ),
            };
            *value = edit_value.into();
            response
        }
        TimeRangeBoundary::Absolute(value) => {
            // see note above
            let low_bound_override = if low_bound {
                None
            } else {
                Some(other_boundary_absolute.into())
            };

            let mut edit_value = (*value).into();
            let response = match time_type {
                TimeType::Time => {
                    let (drag_resp, base_time_resp) = time_drag_value.temporal_drag_value_ui(
                        ui,
                        &mut edit_value,
                        true,
                        low_bound_override,
                        ctx.app_options.time_zone,
                    );

                    if let Some(base_time_resp) = base_time_resp {
                        base_time_resp.on_hover_text("Base time used to set time range boundaries");
                    }

                    Some(drag_resp.on_hover_text("Absolute time to use as time range boundary"))
                }
                TimeType::Sequence => Some(
                    time_drag_value
                        .sequence_drag_value_ui(ui, &mut edit_value, true, low_bound_override)
                        .on_hover_text("Absolute frame number to use as time range boundary"),
                ),
            };
            *value = edit_value.into();
            response
        }
        TimeRangeBoundary::Infinite => None,
    };

    response.map_or(false, |r| r.dragged() || r.has_focus())
}
