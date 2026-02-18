use egui::{NumExt as _, Ui};
use re_chunk::Timeline;
use re_log_types::{AbsoluteTimeRange, EntityPath, TimeType, TimelineName};
use re_sdk_types::Archetype as _;
use re_sdk_types::blueprint::archetypes as blueprint_archetypes;
use re_sdk_types::blueprint::components::VisibleTimeRange;
use re_sdk_types::datatypes::{TimeInt, TimeRange, TimeRangeBoundary};
use re_ui::list_item::{LabelContent, ListItemContentButtonsExt as _};
use re_ui::{RelativeTimeRange, TimeDragValue, UiExt as _, relative_time_range_label_text};
use re_viewer_context::{
    BlueprintContext as _, QueryRange, TimeControlCommand, ViewClass, ViewState, ViewerContext,
};
use re_viewport_blueprint::{ViewBlueprint, entity_path_for_view_property};

pub fn visible_time_range_ui_for_view(
    ctx: &ViewerContext<'_>,
    ui: &mut Ui,
    view: &ViewBlueprint,
    view_class: &dyn ViewClass,
    view_state: &dyn ViewState,
) {
    if !view_class.supports_visible_time_range() {
        return;
    }

    let property_path = entity_path_for_view_property(
        view.id,
        ctx.store_context.blueprint.tree(),
        re_sdk_types::blueprint::archetypes::VisibleTimeRanges::name(),
    );

    let query_range = view.query_range(
        ctx.store_context.blueprint,
        ctx.blueprint_query,
        ctx.time_ctrl.timeline(),
        ctx.view_class_registry(),
        view_state,
    );

    let is_view = true;
    visible_time_range_ui(ctx, ui, query_range, &property_path, is_view);
}

pub fn visible_time_range_ui_for_data_result(
    ctx: &ViewerContext<'_>,
    ui: &mut Ui,
    data_result: &re_viewer_context::DataResult,
) {
    let query_range = data_result.query_range;
    let is_view = false;
    visible_time_range_ui(
        ctx,
        ui,
        query_range,
        data_result.override_base_path(),
        is_view,
    );
}

/// Draws ui for a visible time range from a given override path and a resulting query range.
fn visible_time_range_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut Ui,
    mut resolved_query_range: QueryRange,
    time_range_override_path: &EntityPath,
    is_view: bool,
) {
    let visible_time_ranges = ctx
        .blueprint_db()
        .latest_at(
            ctx.blueprint_query,
            time_range_override_path,
            [blueprint_archetypes::VisibleTimeRanges::descriptor_ranges().component],
        )
        .component_batch::<VisibleTimeRange>(
            blueprint_archetypes::VisibleTimeRanges::descriptor_ranges().component,
        )
        .unwrap_or_default();

    let timeline_name = *ctx.time_ctrl.timeline_name();
    let mut has_individual_range = visible_time_ranges
        .iter()
        .any(|range| range.timeline.as_str() == timeline_name.as_str());

    let has_individual_range_before = has_individual_range;
    let query_range_before = resolved_query_range;

    ui.scope(|ui| {
        // TODO(#6075): Because `list_item_scope` changes it. Temporary until everything is `ListItem`.
        ui.spacing_mut().item_spacing.y = ui.ctx().style().spacing.item_spacing.y;
        query_range_ui(
            ctx,
            ui,
            &mut resolved_query_range,
            &mut has_individual_range,
            is_view,
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
            time_range_override_path.clone(),
            visible_time_ranges,
        );
    }
}

fn save_visible_time_ranges(
    ctx: &ViewerContext<'_>,
    timeline_name: &TimelineName,
    has_individual_range: bool,
    query_range: QueryRange,
    property_path: EntityPath,
    mut visible_time_range_list: Vec<VisibleTimeRange>,
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

        if let Some(existing) = visible_time_range_list
            .iter_mut()
            .find(|r| r.timeline.as_str() == timeline_name.as_str())
        {
            existing.range = time_range;
        } else {
            visible_time_range_list.push(
                re_sdk_types::datatypes::VisibleTimeRange {
                    timeline: timeline_name.as_str().into(),
                    range: time_range,
                }
                .into(),
            );
        }
    } else {
        visible_time_range_list.retain(|r| r.timeline.as_str() != timeline_name.as_str());
    }

    ctx.save_blueprint_component(
        property_path,
        &blueprint_archetypes::VisibleTimeRanges::descriptor_ranges(),
        &visible_time_range_list,
    );
}

/// Draws ui for showing and configuring a query range.
fn query_range_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut Ui,
    query_range: &mut QueryRange,
    has_individual_time_range: &mut bool,
    is_view: bool,
) {
    let time_ctrl = &ctx.time_ctrl;
    let Some(&timeline) = time_ctrl.timeline() else {
        ui.weak("No active timeline");
        return;
    };
    let time_type = timeline.typ();

    let markdown = "# Visible time range\n
This feature controls the time range used to display data in the view.

Notes:
- The settings are inherited from the enclosing view if not overridden.
- Visible time range properties are stored on a per-timeline basis.
- The data current as of the time range starting time is included.";

    let collapsing_response = ui
        .section_collapsing_header("Visible time range")
        .default_open(true)
        .with_help_markdown(markdown)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.re_radio_value(has_individual_time_range, false, "Default")
                    .on_hover_text(if is_view {
                        "Default query range settings for this kind of view"
                    } else {
                        "Query range settings inherited from enclosing view"
                    });
                ui.re_radio_value(has_individual_time_range, true, "Override")
                    .on_hover_text(if is_view {
                        "Set query range settings for the contents of this view"
                    } else {
                        "Set query range settings for this entity"
                    });
            });
            let time_drag_value =
                if let Some(range) = ctx.recording().time_range_for(time_ctrl.timeline_name()) {
                    TimeDragValue::from_abs_time_range(range)
                } else {
                    TimeDragValue::from_time_range(0..=0)
                };

            let current_time = TimeInt(
                time_ctrl
                    .time_i64()
                    .unwrap_or_default()
                    .at_least(*time_drag_value.range.start()),
            ); // accounts for static time (TimeInt::MIN)

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
                    time_type,
                    &time_drag_value,
                );
            } else {
                match &query_range {
                    QueryRange::TimeRange(range) => {
                        show_visual_time_range(ctx, ui, range, timeline, current_time);
                    }
                    QueryRange::LatestAt => {
                        let current_time =
                            time_type.format(current_time, ctx.app_options().timestamp_format);
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

    let mut rect = collapsing_response.header_response.rect;
    if let Some(body_response) = collapsing_response.body_response {
        rect = rect.union(body_response.rect);
    }
    let should_display_visible_time_range = ui.rect_contains_pointer(rect);

    if should_display_visible_time_range
        && let Some(current_time) = time_ctrl.time_int()
        && let QueryRange::TimeRange(time_range) = &query_range
    {
        let absolute_time_range =
            AbsoluteTimeRange::from_relative_time_range(time_range, current_time);
        ctx.send_time_commands([TimeControlCommand::HighlightRange(absolute_time_range)]);
    }
}

fn time_range_editor(
    ctx: &ViewerContext<'_>,
    ui: &mut Ui,
    resolved_range: &mut TimeRange,
    current_time: TimeInt,
    time_type: TimeType,
    time_drag_value: &TimeDragValue,
) {
    let current_start = resolved_range.start.start_boundary_time(current_time);
    let current_end = resolved_range.end.end_boundary_time(current_time);

    RelativeTimeRange {
        time_drag_value,
        value: resolved_range,
        resolved_range: AbsoluteTimeRange::new(current_start, current_end),
        time_type,
        timestamp_format: ctx.app_options().timestamp_format,
        current_time,
    }
    .ui(ui);
}

fn show_visual_time_range(
    ctx: &ViewerContext<'_>,
    ui: &mut Ui,
    resolved_range: &TimeRange,
    timeline: Timeline,
    current_time: TimeInt,
) {
    let time_type = timeline.typ();

    // Show the resolved visible range as labels (user can't edit them):
    if resolved_range == &TimeRange::EVERYTHING {
        ui.label("Entire timeline");
    } else if resolved_range == &TimeRange::AT_CURSOR {
        let current_time = time_type.format(current_time, ctx.app_options().timestamp_format);
        ui.label(format!("At {} = {current_time}", timeline.name())).on_hover_text("Does not perform a latest-at query, shows only data logged at exactly the current time cursor position.");
    } else {
        egui::Grid::new("from_to_labels").show(ui, |ui| {
            ui.grid_left_hand_label("From");
            resolved_visible_history_boundary_ui(ctx, ui, &resolved_range.start, time_type, true);
            ui.end_row();

            ui.grid_left_hand_label("To");
            resolved_visible_history_boundary_ui(ctx, ui, &resolved_range.end, time_type, false);
            ui.end_row();
        });

        let (text, on_hover) = relative_time_range_label_text(
            current_time,
            time_type,
            resolved_range,
            ctx.app_options().timestamp_format,
        );

        let response = ui
            .list_item()
            .interactive(false)
            .show_hierarchical(ui, LabelContent::new(text));

        if let Some(on_hover) = on_hover {
            response.on_hover_text(on_hover);
        }
    }
}

fn resolved_visible_history_boundary_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    visible_history_boundary: &TimeRangeBoundary,
    time_type: TimeType,
    low_bound: bool,
) {
    let boundary_type = match visible_history_boundary {
        TimeRangeBoundary::CursorRelative(_) => match time_type {
            TimeType::DurationNs | TimeType::TimestampNs => "current time",
            TimeType::Sequence => "current frame",
        },
        TimeRangeBoundary::Absolute(_) => match time_type {
            TimeType::DurationNs | TimeType::TimestampNs => "absolute time",
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
                    TimeType::DurationNs | TimeType::TimestampNs => {
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
                            " with {} offset",
                            re_format::format_plural_signed_s(offset, "frame")
                        );
                    }
                }
            }
        }
        TimeRangeBoundary::Absolute(time) => {
            label += &format!(
                " {}",
                time_type.format(*time, ctx.app_options().timestamp_format)
            );
        }
        TimeRangeBoundary::Infinite => {}
    }

    ui.label(label);
}
