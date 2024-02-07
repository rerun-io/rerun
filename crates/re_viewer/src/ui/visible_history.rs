use std::collections::HashSet;
use std::ops::RangeInclusive;

use egui::{NumExt as _, Response, Ui};

use re_entity_db::{ExtraQueryHistory, TimeHistogram, VisibleHistory, VisibleHistoryBoundary};
use re_log_types::{EntityPath, TimeType, TimeZone};
use re_space_view_spatial::{SpatialSpaceView2D, SpatialSpaceView3D};
use re_space_view_time_series::TimeSeriesSpaceView;
use re_types_core::ComponentName;
use re_ui::{markdown_ui, ReUi};
use re_viewer_context::{SpaceViewClass, SpaceViewClassIdentifier, TimeControl, ViewerContext};

/// These space views support the Visible History feature.
static VISIBLE_HISTORY_SUPPORTED_SPACE_VIEWS: once_cell::sync::Lazy<
    HashSet<SpaceViewClassIdentifier>,
> = once_cell::sync::Lazy::new(|| {
    [
        SpatialSpaceView3D::IDENTIFIER,
        SpatialSpaceView2D::IDENTIFIER,
        TimeSeriesSpaceView::IDENTIFIER,
    ]
    .map(Into::into)
    .into()
});

/// Entities containing one of these components support the Visible History feature.
static VISIBLE_HISTORY_SUPPORTED_COMPONENT_NAMES: once_cell::sync::Lazy<Vec<ComponentName>> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.HalfSizes2D",
            "rerun.components.HalfSizes3D",
            "rerun.components.LineStrip2D",
            "rerun.components.LineStrip3D",
            "rerun.components.Position2D",
            "rerun.components.Position3D",
            "rerun.components.Scalar",
            "rerun.components.TensorData",
            "rerun.components.Vector3D",
        ]
        .map(Into::into)
        .into()
    });

// TODO(#4145): This method is obviously unfortunate. It's a temporary solution until the Visualizer
// system is able to report its ability to handle the visible history feature.
fn has_visible_history(
    ctx: &ViewerContext<'_>,
    time_ctrl: &TimeControl,
    space_view_class: &SpaceViewClassIdentifier,
    entity_path: Option<&EntityPath>,
) -> bool {
    if !VISIBLE_HISTORY_SUPPORTED_SPACE_VIEWS.contains(space_view_class) {
        return false;
    }

    if let Some(entity_path) = entity_path {
        let store = ctx.entity_db.store();
        let component_names = store.all_components(time_ctrl.timeline(), entity_path);
        if let Some(component_names) = component_names {
            if !component_names
                .iter()
                .any(|name| VISIBLE_HISTORY_SUPPORTED_COMPONENT_NAMES.contains(name))
            {
                return false;
            }
        } else {
            return false;
        }
    }

    true
}

pub fn visible_history_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    space_view_class: &SpaceViewClassIdentifier,
    is_space_view: bool,
    entity_path: Option<&EntityPath>,
    visible_history_prop: &mut ExtraQueryHistory,
    resolved_visible_history_prop: &ExtraQueryHistory,
) {
    let time_ctrl = ctx.rec_cfg.time_ctrl.read().clone();
    if !has_visible_history(ctx, &time_ctrl, space_view_class, entity_path) {
        return;
    }

    let re_ui = ctx.re_ui;

    let time_type = time_ctrl.timeline().typ();

    let mut interacting_with_controls = false;

    let collapsing_response = re_ui.collapsing_header(ui, "Visible Time Range", false, |ui| {
        ui.horizontal(|ui| {
            re_ui
                .radio_value(ui, &mut visible_history_prop.enabled, false, "Default")
                .on_hover_text(if is_space_view {
                    "Default Visible Time Range settings for this kind of Space View"
                } else {
                    "Visible Time Range settings inherited from parent Group(s) or enclosing \
                        Space View"
                });
            re_ui
                .radio_value(ui, &mut visible_history_prop.enabled, true, "Override")
                .on_hover_text(if is_space_view {
                    "Set Visible Time Range settings for the contents of this Space View"
                } else if entity_path.is_some() {
                    "Set Visible Time Range settings for this entity"
                } else {
                    "Set Visible Time Range settings for he contents of this Group"
                });
        });

        let timeline_spec = if let Some(times) = ctx.entity_db.time_histogram(time_ctrl.timeline())
        {
            TimelineSpec::from_time_histogram(times)
        } else {
            TimelineSpec::from_time_range(0..=0)
        };

        let current_time = time_ctrl
            .time_i64()
            .unwrap_or_default()
            .at_least(*timeline_spec.range.start()); // accounts for timeless time (TimeInt::BEGINNING)

        let (resolved_visible_history, visible_history) = match time_type {
            TimeType::Time => (
                &resolved_visible_history_prop.nanos,
                &mut visible_history_prop.nanos,
            ),
            TimeType::Sequence => (
                &resolved_visible_history_prop.sequences,
                &mut visible_history_prop.sequences,
            ),
        };

        if visible_history_prop.enabled {
            let current_low_boundary = visible_history
                .range_start_from_cursor(current_time.into())
                .as_i64();
            let current_high_boundary = visible_history
                .range_end_from_cursor(current_time.into())
                .as_i64();

            egui::Grid::new("from_to_ediable").show(ui, |ui| {
                re_ui.grid_left_hand_label(ui, "From");
                interacting_with_controls |= ui
                    .horizontal(|ui| {
                        visible_history_boundary_ui(
                            ctx,
                            ui,
                            &mut visible_history.from,
                            time_type,
                            current_time,
                            &timeline_spec,
                            true,
                            current_high_boundary,
                        )
                    })
                    .inner;
                ui.end_row();

                re_ui.grid_left_hand_label(ui, "To");
                interacting_with_controls |= ui
                    .horizontal(|ui| {
                        visible_history_boundary_ui(
                            ctx,
                            ui,
                            &mut visible_history.to,
                            time_type,
                            current_time,
                            &timeline_spec,
                            false,
                            current_low_boundary,
                        )
                    })
                    .inner;
                ui.end_row();
            });

            current_range_ui(ctx, ui, current_time, time_type, visible_history);
        } else {
            // Show the resolved visible range as labels (user can't edit them):

            if resolved_visible_history.from == VisibleHistoryBoundary::Infinite
                && resolved_visible_history.to == VisibleHistoryBoundary::Infinite
            {
                ui.label("Entire timeline.");
            } else if resolved_visible_history.from == VisibleHistoryBoundary::AT_CURSOR
                && resolved_visible_history.to == VisibleHistoryBoundary::AT_CURSOR
            {
                let current_time = time_type.format(current_time.into(), ctx.app_options.time_zone);
                match time_type {
                    TimeType::Time => {
                        ui.label(format!("At current time: {current_time}"));
                    }
                    TimeType::Sequence => {
                        ui.label(format!("At current frame: {current_time}"));
                    }
                }
            } else {
                egui::Grid::new("from_to_labels").show(ui, |ui| {
                    re_ui.grid_left_hand_label(ui, "From");
                    resolved_visible_history_boundary_ui(
                        ctx,
                        ui,
                        &resolved_visible_history.from,
                        time_type,
                        true,
                    );
                    ui.end_row();

                    re_ui.grid_left_hand_label(ui, "To");
                    resolved_visible_history_boundary_ui(
                        ctx,
                        ui,
                        &resolved_visible_history.to,
                        time_type,
                        false,
                    );
                    ui.end_row();
                });

                current_range_ui(ctx, ui, current_time, time_type, resolved_visible_history);
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

    let should_display_visible_history = interacting_with_controls
        || collapsing_response.header_response.hovered()
        || collapsing_response
            .body_response
            .map_or(false, |r| r.hovered());

    if should_display_visible_history {
        if let Some(current_time) = time_ctrl.time_int() {
            let visible_history = match (visible_history_prop.enabled, time_type) {
                (true, TimeType::Sequence) => visible_history_prop.sequences,
                (true, TimeType::Time) => visible_history_prop.nanos,
                (false, TimeType::Sequence) => resolved_visible_history_prop.sequences,
                (false, TimeType::Time) => resolved_visible_history_prop.nanos,
            };

            ctx.rec_cfg.time_ctrl.write().highlighted_range =
                Some(visible_history.time_range(current_time));
        }
    }

    let markdown = format!("# Visible Time Range\n
This feature controls the time range used to display data in the Space View.

The settings are inherited from parent Group(s) or enclosing Space View if not overridden.

Visible Time Range properties are stored separately for each _type_ of timelines. They may differ depending on \
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

fn current_range_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut Ui,
    current_time: i64,
    time_type: TimeType,
    visible_history: &VisibleHistory,
) {
    let time_range = visible_history.time_range(current_time.into());
    let from_formatted = time_type.format(time_range.min, ctx.app_options.time_zone);
    let to_formatted = time_type.format(time_range.max, ctx.app_options.time_zone);

    ui.label(format!("{from_formatted} to {to_formatted}"))
        .on_hover_text("Showing data in this range (inclusive).");
}

#[allow(clippy::too_many_arguments)]
fn resolved_visible_history_boundary_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    visible_history_boundary: &VisibleHistoryBoundary,
    time_type: TimeType,
    low_bound: bool,
) {
    let boundary_type = match visible_history_boundary {
        VisibleHistoryBoundary::RelativeToTimeCursor(_) => match time_type {
            TimeType::Time => "current time",
            TimeType::Sequence => "current frame",
        },
        VisibleHistoryBoundary::Absolute(_) => match time_type {
            TimeType::Time => "absolute time",
            TimeType::Sequence => "frame",
        },
        VisibleHistoryBoundary::Infinite => {
            if low_bound {
                "beginning of timeline"
            } else {
                "end of timeline"
            }
        }
    };

    let mut label = boundary_type.to_owned();

    match visible_history_boundary {
        VisibleHistoryBoundary::RelativeToTimeCursor(offset) => {
            if *offset != 0 {
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

                        label += &format!(" with {} {} offset", *offset as f64 / factor, unit);
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
        VisibleHistoryBoundary::Absolute(time) => {
            label += &format!(
                " {}",
                time_type.format((*time).into(), ctx.app_options.time_zone)
            );
        }
        VisibleHistoryBoundary::Infinite => {}
    }

    ui.label(label);
}

fn visible_history_boundary_combo_label(
    boundary: &VisibleHistoryBoundary,
    time_type: TimeType,
    low_bound: bool,
) -> &'static str {
    match boundary {
        VisibleHistoryBoundary::RelativeToTimeCursor(_) => match time_type {
            TimeType::Time => "current time with offset",
            TimeType::Sequence => "current frame with offset",
        },
        VisibleHistoryBoundary::Absolute(_) => match time_type {
            TimeType::Time => "absolute time",
            TimeType::Sequence => "absolute frame",
        },
        VisibleHistoryBoundary::Infinite => {
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
    visible_history_boundary: &mut VisibleHistoryBoundary,
    time_type: TimeType,
    current_time: i64,
    timeline_spec: &TimelineSpec,
    low_bound: bool,
    other_boundary_absolute: i64,
) -> bool {
    let (abs_time, rel_time) = match visible_history_boundary {
        VisibleHistoryBoundary::RelativeToTimeCursor(value) => (*value + current_time, *value),
        VisibleHistoryBoundary::Absolute(value) => (*value, *value - current_time),
        VisibleHistoryBoundary::Infinite => (current_time, 0),
    };
    let abs_time = VisibleHistoryBoundary::Absolute(abs_time);
    let rel_time = VisibleHistoryBoundary::RelativeToTimeCursor(rel_time);

    egui::ComboBox::from_id_source(if low_bound {
        "time_history_low_bound"
    } else {
        "time_history_high_bound"
    })
    .selected_text(visible_history_boundary_combo_label(
        visible_history_boundary,
        time_type,
        low_bound,
    ))
    .show_ui(ui, |ui| {
        ui.set_min_width(160.0);

        ui.selectable_value(
            visible_history_boundary,
            rel_time,
            visible_history_boundary_combo_label(&rel_time, time_type, low_bound),
        )
        .on_hover_text(if low_bound {
            "Show data from a time point relative to the current time."
        } else {
            "Show data until a time point relative to the current time."
        });
        ui.selectable_value(
            visible_history_boundary,
            abs_time,
            visible_history_boundary_combo_label(&abs_time, time_type, low_bound),
        )
        .on_hover_text(if low_bound {
            "Show data from an absolute time point."
        } else {
            "Show data until an absolute time point."
        });
        ui.selectable_value(
            visible_history_boundary,
            VisibleHistoryBoundary::Infinite,
            visible_history_boundary_combo_label(
                &VisibleHistoryBoundary::Infinite,
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

    let response = match visible_history_boundary {
        VisibleHistoryBoundary::RelativeToTimeCursor(value) => {
            // see note above
            let low_bound_override = if !low_bound {
                Some(other_boundary_absolute.saturating_sub(current_time))
            } else {
                None
            };

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
        VisibleHistoryBoundary::Absolute(value) => {
            // see note above
            let low_bound_override = if !low_bound {
                Some(other_boundary_absolute)
            } else {
                None
            };

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
        VisibleHistoryBoundary::Infinite => None,
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
        value: &mut i64,
        absolute: bool,
        low_bound_override: Option<i64>,
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
            time_range = low_bound_override.at_least(*time_range.start())..=*time_range.end();
        }

        ui.add(
            egui::DragValue::new(value)
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
        value: &mut i64,
        absolute: bool,
        low_bound_override: Option<i64>,
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
            time_range = low_bound_override.at_least(*time_range.start())..=*time_range.end();
        }

        let mut time_unit = (*value - offset) as f32 / factor;

        let time_range = (*time_range.start() - offset) as f32 / factor
            ..=(*time_range.end() - offset) as f32 / factor;

        let base_time_response = if absolute {
            self.base_time.map(|base_time| {
                ui.label(format!(
                    "{} + ",
                    TimeType::Time.format(base_time.into(), time_zone_for_timestamps)
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

        *value = (time_unit * factor).round() as i64 + offset;

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
