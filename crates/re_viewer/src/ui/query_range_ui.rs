use std::collections::HashSet;
use std::ops::RangeInclusive;

use egui::{NumExt as _, Response, Ui};

use re_entity_db::TimeHistogram;
use re_log_types::{EntityPath, ResolvedTimeRange, TimeType, TimeZone, TimelineName};
use re_space_view::{entity_path_for_view_property, SpaceViewBlueprint};
use re_space_view_spatial::{SpatialSpaceView2D, SpatialSpaceView3D};
use re_space_view_time_series::TimeSeriesSpaceView;
use re_types::{
    datatypes::{TimeInt, TimeRange, TimeRangeBoundary, TimeRangeBoundaryKind},
    SpaceViewClassIdentifier,
};
use re_ui::{markdown_ui, ReUi};
use re_viewer_context::{QueryRange, SpaceViewClass, ViewerContext};

/// These space views support the Visible History feature.
static VISIBLE_HISTORY_SUPPORTED_SPACE_VIEWS: once_cell::sync::Lazy<
    HashSet<SpaceViewClassIdentifier>,
> = once_cell::sync::Lazy::new(|| {
    [
        SpatialSpaceView3D::identifier(),
        SpatialSpaceView2D::identifier(),
        TimeSeriesSpaceView::identifier(),
    ]
    .map(Into::into)
    .into()
});

// TODO(#4145): This method is obviously unfortunate. It's a temporary solution until the Visualizer
// system is able to report its ability to handle the visible history feature.
fn space_view_with_visible_history(space_view_class: SpaceViewClassIdentifier) -> bool {
    VISIBLE_HISTORY_SUPPORTED_SPACE_VIEWS.contains(&space_view_class)
}

pub fn query_range_ui_space_view(
    ctx: &ViewerContext<'_>,
    ui: &mut Ui,
    space_view: &SpaceViewBlueprint,
) {
    if !space_view_with_visible_history(*space_view.class_identifier()) {
        return;
    }

    let property_path = entity_path_for_view_property::<
        re_types::blueprint::archetypes::VisibleTimeRanges,
    >(space_view.id, ctx.store_context.blueprint.tree());

    let query_range = space_view.query_range(
        ctx.store_context.blueprint,
        ctx.blueprint_query,
        ctx.rec_cfg.time_ctrl.read().timeline(),
        ctx.space_view_class_registry,
    );

    let is_space_view = true;
    visible_time_range_ui(ctx, ui, query_range, &property_path, is_space_view);
}

pub fn query_range_ui_data_result(
    ctx: &ViewerContext<'_>,
    ui: &mut Ui,
    data_result: &re_viewer_context::DataResult,
) {
    let Some(override_path) = data_result.recursive_override_path() else {
        re_log::error_once!("No override computed yet for entity");
        return;
    };
    let Some(overrides) = data_result.property_overrides.as_ref() else {
        re_log::error_once!("No override computed yet for entity");
        return;
    };
    let query_range = overrides.query_range.clone();

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
    let visible_time_ranges = ctx
        .store_context
        .blueprint
        .latest_at_archetype::<re_types::blueprint::archetypes::VisibleTimeRanges>(
            time_range_override_path,
            ctx.blueprint_query,
        )
        .ok()
        .flatten()
        .map_or(Default::default(), |(_, arch)| arch);

    let timeline_name = *ctx.rec_cfg.time_ctrl.read().timeline().name();
    let mut has_individual_range = visible_time_ranges
        .range_for_timeline(timeline_name.as_str())
        .is_some();

    let has_individual_range_before = has_individual_range;
    let query_range_before = resolved_query_range.clone();

    query_range_ui(
        ctx,
        ui,
        &mut resolved_query_range,
        &mut has_individual_range,
        is_space_view,
    );

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
    mut visible_time_ranges: re_types::blueprint::archetypes::VisibleTimeRanges,
) {
    if has_individual_range {
        let time_range = match query_range {
            QueryRange::TimeRange(time_range) => time_range,
            QueryRange::LatestAt => {
                re_log::error!("Latest-at queries can't be used as an override yet. They can only come from defaults.");
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
    let re_ui = ctx.re_ui;
    let time_ctrl = ctx.rec_cfg.time_ctrl.read().clone();
    let time_type = time_ctrl.timeline().typ();

    let mut interacting_with_controls = false;

    let collapsing_response = ctx
        .re_ui
        .collapsing_header(ui, "Visible time range", false, |ui| {
            ui.horizontal(|ui| {
                re_ui
                    .radio_value(ui, has_individual_time_range, false, "Default")
                    .on_hover_text(if is_space_view {
                        "Default query range settings for this kind of space view"
                    } else {
                        "Query range settings inherited from parent Entity or enclosing \
                        space view"
                    });
                re_ui
                    .radio_value(ui, has_individual_time_range, true, "Override")
                    .on_hover_text(if is_space_view {
                        "Set query range settings for the contents of this space view"
                    } else {
                        "Set query range settings for this entity"
                    });
            });
            let timeline_spec =
                if let Some(times) = ctx.recording().time_histogram(time_ctrl.timeline()) {
                    TimelineSpec::from_time_histogram(times)
                } else {
                    TimelineSpec::from_time_range(0..=0)
                };

            let current_time = TimeInt(
                time_ctrl
                    .time_i64()
                    .unwrap_or_default()
                    .at_least(*timeline_spec.range.start()),
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
                    &timeline_spec,
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

    // Add spacer after the visible history section.
    //TODO(ab): figure out why `item_spacing.y` is added _only_ in collapsed state.
    if collapsing_response.body_response.is_some() {
        ui.add_space(ui.spacing().item_spacing.y / 2.0);
    } else {
        ui.add_space(-ui.spacing().item_spacing.y / 2.0);
    }
    ReUi::full_span_separator(ui);
    ui.add_space(ui.spacing().item_spacing.y / 2.0);

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

    let markdown = format!("# Visible time range\n
This feature controls the time range used to display data in the space view.

The settings are inherited from the parent Entity or enclosing space view if not overridden.

Visible time range properties are stored separately for each _type_ of timelines. They may differ depending on \
whether the current timeline is temporal or a sequence. The current settings apply to all _{}_ timelines.

Notes that the data current as of the time range starting time is included.",
        match time_type {
            TimeType::Time => "temporal",
            TimeType::Sequence => "sequence",
        }
    );

    collapsing_response.header_response.on_hover_ui(|ui| {
        markdown_ui(ui, egui::Id::new(markdown.as_str()), &markdown);
    });
}

fn time_range_editor(
    ctx: &ViewerContext<'_>,
    ui: &mut Ui,
    resolved_range: &mut TimeRange,
    current_time: TimeInt,
    interacting_with_controls: &mut bool,
    time_type: TimeType,
    timeline_spec: &TimelineSpec,
) {
    let current_start = resolved_range.start.start_boundary_time(current_time);
    let current_end = resolved_range.end.end_boundary_time(current_time);

    egui::Grid::new("from_to_editable").show(ui, |ui| {
        ctx.re_ui.grid_left_hand_label(ui, "Start");
        *interacting_with_controls |= ui
            .horizontal(|ui| {
                visible_history_boundary_ui(
                    ctx,
                    ui,
                    &mut resolved_range.start,
                    time_type,
                    current_time,
                    timeline_spec,
                    true,
                    current_end,
                )
            })
            .inner;
        ui.end_row();

        ctx.re_ui.grid_left_hand_label(ui, "End");
        *interacting_with_controls |= ui
            .horizontal(|ui| {
                visible_history_boundary_ui(
                    ctx,
                    ui,
                    &mut resolved_range.end,
                    time_type,
                    current_time,
                    timeline_spec,
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
    if resolved_range.start.kind == TimeRangeBoundaryKind::Infinite
        && resolved_range.end.kind == TimeRangeBoundaryKind::Infinite
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
            ctx.re_ui.grid_left_hand_label(ui, "From");
            resolved_visible_history_boundary_ui(ctx, ui, &resolved_range.start, time_type, true);
            ui.end_row();

            ctx.re_ui.grid_left_hand_label(ui, "To");
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
    let boundary_type = match visible_history_boundary.kind {
        TimeRangeBoundaryKind::RelativeToTimeCursor => match time_type {
            TimeType::Time => "current time",
            TimeType::Sequence => "current frame",
        },
        TimeRangeBoundaryKind::Absolute => match time_type {
            TimeType::Time => "absolute time",
            TimeType::Sequence => "frame",
        },
        TimeRangeBoundaryKind::Infinite => {
            if low_bound {
                "beginning of timeline"
            } else {
                "end of timeline"
            }
        }
    };

    let mut label = boundary_type.to_owned();

    match visible_history_boundary.kind {
        TimeRangeBoundaryKind::RelativeToTimeCursor => {
            let offset = visible_history_boundary.time.0;
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
        TimeRangeBoundaryKind::Absolute => {
            let time = visible_history_boundary.time;
            label += &format!(" {}", time_type.format(time, ctx.app_options.time_zone));
        }
        TimeRangeBoundaryKind::Infinite => {}
    }

    ui.label(label);
}

fn visible_history_boundary_combo_label(
    boundary: TimeRangeBoundaryKind,
    time_type: TimeType,
    low_bound: bool,
) -> &'static str {
    match boundary {
        TimeRangeBoundaryKind::RelativeToTimeCursor => match time_type {
            TimeType::Time => "current time with offset",
            TimeType::Sequence => "current frame with offset",
        },
        TimeRangeBoundaryKind::Absolute => match time_type {
            TimeType::Time => "absolute time",
            TimeType::Sequence => "absolute frame",
        },
        TimeRangeBoundaryKind::Infinite => {
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
    timeline_spec: &TimelineSpec,
    low_bound: bool,
    other_boundary_absolute: TimeInt,
) -> bool {
    let (abs_time, rel_time) = match visible_history_boundary.kind {
        TimeRangeBoundaryKind::RelativeToTimeCursor => (
            visible_history_boundary.time + current_time,
            visible_history_boundary.time,
        ),
        TimeRangeBoundaryKind::Absolute => (
            visible_history_boundary.time,
            visible_history_boundary.time - current_time,
        ),
        TimeRangeBoundaryKind::Infinite => (current_time, TimeInt(0)),
    };
    let abs_time = TimeRangeBoundary::absolute(abs_time);
    let rel_time = TimeRangeBoundary::relative_to_time_cursor(rel_time);

    egui::ComboBox::from_id_source(if low_bound {
        "time_history_low_bound"
    } else {
        "time_history_high_bound"
    })
    .selected_text(visible_history_boundary_combo_label(
        visible_history_boundary.kind,
        time_type,
        low_bound,
    ))
    .show_ui(ui, |ui| {
        ui.set_min_width(160.0);

        ui.selectable_value(
            visible_history_boundary,
            rel_time,
            visible_history_boundary_combo_label(rel_time.kind, time_type, low_bound),
        )
        .on_hover_text(if low_bound {
            "Show data from a time point relative to the current time."
        } else {
            "Show data until a time point relative to the current time."
        });
        ui.selectable_value(
            visible_history_boundary,
            abs_time,
            visible_history_boundary_combo_label(abs_time.kind, time_type, low_bound),
        )
        .on_hover_text(if low_bound {
            "Show data from an absolute time point."
        } else {
            "Show data until an absolute time point."
        });
        ui.selectable_value(
            &mut visible_history_boundary.kind,
            TimeRangeBoundaryKind::Infinite,
            visible_history_boundary_combo_label(
                TimeRangeBoundaryKind::Infinite,
                time_type,
                low_bound,
            ),
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

    let response = match visible_history_boundary.kind {
        TimeRangeBoundaryKind::RelativeToTimeCursor => {
            // see note above
            let low_bound_override = if low_bound {
                None
            } else {
                Some(other_boundary_absolute - current_time)
            };
            let value = &mut visible_history_boundary.time;

            match time_type {
                TimeType::Time => Some(
                    timeline_spec
                        .temporal_drag_value(
                            ui,
                            value,
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
                    timeline_spec
                        .sequence_drag_value(ui, value, false, low_bound_override)
                        .on_hover_text(
                            "Number of frames before/after the current time to use a time \
                        range boundary",
                        ),
                ),
            }
        }
        TimeRangeBoundaryKind::Absolute => {
            // see note above
            let low_bound_override = if low_bound {
                None
            } else {
                Some(other_boundary_absolute)
            };
            let value = &mut visible_history_boundary.time;

            match time_type {
                TimeType::Time => {
                    let (drag_resp, base_time_resp) = timeline_spec.temporal_drag_value(
                        ui,
                        value,
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
                    timeline_spec
                        .sequence_drag_value(ui, value, true, low_bound_override)
                        .on_hover_text("Absolute frame number to use as time range boundary"),
                ),
            }
        }
        TimeRangeBoundaryKind::Infinite => None,
    };

    response.map_or(false, |r| r.dragged() || r.has_focus())
}

// ---

/// Compute and store various information about a timeline related to how the UI should behave.
#[derive(Debug)]
struct TimelineSpec {
    /// Actual range of logged data on the timelines (excluding timeless data).
    range: RangeInclusive<i64>,

    /// For timelines with large offsets (e.g. `log_time`), this is a rounded time just before the
    /// first logged data, which can be used as offset in the UI.
    base_time: Option<i64>,

    // used only for temporal timelines
    /// For temporal timelines, this is a nice unit factor to use.
    unit_factor: i64,

    /// For temporal timelines, this is the unit symbol to display.
    unit_symbol: &'static str,

    /// This is a nice range of absolute times to use when editing an absolute time. The boundaries
    /// are extended to the nearest rounded unit to minimize glitches.
    abs_range: RangeInclusive<i64>,

    /// This is a nice range of relative times to use when editing an absolute time. The boundaries
    /// are extended to the nearest rounded unit to minimize glitches.
    rel_range: RangeInclusive<i64>,
}

impl TimelineSpec {
    fn from_time_histogram(times: &TimeHistogram) -> Self {
        Self::from_time_range(
            times.min_key().unwrap_or_default()..=times.max_key().unwrap_or_default(),
        )
    }

    fn from_time_range(range: RangeInclusive<i64>) -> Self {
        let span = range.end() - range.start();
        let base_time = time_range_base_time(*range.start(), span);
        let (unit_symbol, unit_factor) = unit_from_span(span);

        // `abs_range` is used by the DragValue when editing an absolute time, its bound expended to
        // nearest unit to minimize glitches.
        let abs_range =
            round_down(*range.start(), unit_factor)..=round_up(*range.end(), unit_factor);

        // `rel_range` is used by the DragValue when editing a relative time offset. It must have
        // enough margin either side to accommodate for all possible values of current time.
        let rel_range = round_down(-span, unit_factor)..=round_up(2 * span, unit_factor);

        Self {
            range,
            base_time,
            unit_factor,
            unit_symbol,
            abs_range,
            rel_range,
        }
    }

    fn sequence_drag_value(
        &self,
        ui: &mut egui::Ui,
        value: &mut TimeInt,
        absolute: bool,
        low_bound_override: Option<TimeInt>,
    ) -> Response {
        let mut time_range = if absolute {
            self.abs_range.clone()
        } else {
            self.rel_range.clone()
        };

        // speed must be computed before messing with time_range for consistency
        let span = time_range.end() - time_range.start();
        let speed = (span as f32 * 0.005).at_least(1.0);

        if let Some(low_bound_override) = low_bound_override {
            time_range = low_bound_override.0.at_least(*time_range.start())..=*time_range.end();
        }

        ui.add(
            egui::DragValue::new(&mut value.0)
                .clamp_range(time_range)
                .speed(speed),
        )
    }

    /// Show a temporal drag value.
    ///
    /// Feature rich:
    /// - scale to the proper units
    /// - display the base time if any
    /// - etc.
    ///
    /// Returns a tuple of the [`egui::DragValue`]'s [`egui::Response`], and the base time label's
    /// [`egui::Response`], if any.
    fn temporal_drag_value(
        &self,
        ui: &mut egui::Ui,
        value: &mut TimeInt,
        absolute: bool,
        low_bound_override: Option<TimeInt>,
        time_zone_for_timestamps: TimeZone,
    ) -> (Response, Option<Response>) {
        let mut time_range = if absolute {
            self.abs_range.clone()
        } else {
            self.rel_range.clone()
        };

        let factor = self.unit_factor as f32;
        let offset = if absolute {
            self.base_time.unwrap_or(0)
        } else {
            0
        };

        // speed must be computed before messing with time_range for consistency
        let speed = (time_range.end() - time_range.start()) as f32 / factor * 0.005;

        if let Some(low_bound_override) = low_bound_override {
            time_range = low_bound_override.0.at_least(*time_range.start())..=*time_range.end();
        }

        let mut time_unit = (value.0 - offset) as f32 / factor;

        let time_range = (*time_range.start() - offset) as f32 / factor
            ..=(*time_range.end() - offset) as f32 / factor;

        let base_time_response = if absolute {
            self.base_time.map(|base_time| {
                ui.label(format!(
                    "{} + ",
                    TimeType::Time.format(TimeInt(base_time), time_zone_for_timestamps)
                ))
            })
        } else {
            None
        };

        let drag_value_response = ui.add(
            egui::DragValue::new(&mut time_unit)
                .clamp_range(time_range)
                .speed(speed)
                .suffix(self.unit_symbol),
        );

        *value = TimeInt((time_unit * factor).round() as i64 + offset);

        (drag_value_response, base_time_response)
    }
}

fn unit_from_span(span: i64) -> (&'static str, i64) {
    if span / 1_000_000_000 > 0 {
        ("s", 1_000_000_000)
    } else if span / 1_000_000 > 0 {
        ("ms", 1_000_000)
    } else if span / 1_000 > 0 {
        ("μs", 1_000)
    } else {
        ("ns", 1)
    }
}

/// Value of the start time over time span ratio above which an explicit offset is handled.
static SPAN_TO_START_TIME_OFFSET_THRESHOLD: i64 = 10;

fn time_range_base_time(min_time: i64, span: i64) -> Option<i64> {
    if min_time <= 0 {
        return None;
    }

    if span.saturating_mul(SPAN_TO_START_TIME_OFFSET_THRESHOLD) < min_time {
        let factor = if span / 1_000_000 > 0 {
            1_000_000_000
        } else if span / 1_000 > 0 {
            1_000_000
        } else {
            1_000
        };

        Some(min_time - (min_time % factor))
    } else {
        None
    }
}

fn round_down(value: i64, factor: i64) -> i64 {
    value - (value.rem_euclid(factor))
}

fn round_up(value: i64, factor: i64) -> i64 {
    let val = round_down(value, factor);

    if val == value {
        val
    } else {
        val + factor
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_round_down() {
        assert_eq!(round_down(2200, 1000), 2000);
        assert_eq!(round_down(2000, 1000), 2000);
        assert_eq!(round_down(-2200, 1000), -3000);
        assert_eq!(round_down(-3000, 1000), -3000);
        assert_eq!(round_down(0, 1000), 0);
    }

    #[test]
    fn test_round_up() {
        assert_eq!(round_up(2200, 1000), 3000);
        assert_eq!(round_up(2000, 1000), 2000);
        assert_eq!(round_up(-2200, 1000), -2000);
        assert_eq!(round_up(-3000, 1000), -3000);
        assert_eq!(round_up(0, 1000), 0);
    }
}
