use std::collections::HashSet;
use std::ops::RangeInclusive;

use egui::{NumExt as _, Response, Ui};

use re_entity_db::{ExtraQueryHistory, TimeHistogram, VisibleHistory, VisibleHistoryBoundary};
use re_log_types::{EntityPath, TimeType, TimeZone};
use re_space_view_spatial::{SpatialSpaceView2D, SpatialSpaceView3D};
use re_space_view_time_series::TimeSeriesSpaceView;
use re_types_core::ComponentName;
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

    let is_sequence_timeline = matches!(time_ctrl.timeline().typ(), TimeType::Sequence);

    let mut interacting_with_controls = false;

    let collapsing_response = re_ui.collapsing_header(ui, "Visible Time Range", true, |ui| {
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

        let (resolved_visible_history, visible_history) = if is_sequence_timeline {
            (
                &resolved_visible_history_prop.sequences,
                &mut visible_history_prop.sequences,
            )
        } else {
            (
                &resolved_visible_history_prop.nanos,
                &mut visible_history_prop.nanos,
            )
        };

        if visible_history_prop.enabled {
            let current_low_boundary = visible_history.range_start_from_cursor(current_time.into()).as_i64();
            let current_high_boundary = visible_history.range_end_from_cursor(current_time.into()).as_i64();

            interacting_with_controls |= ui
                .horizontal(|ui| {
                    visible_history_boundary_ui(
                        ctx,
                        ui,
                        &mut visible_history.from,
                        is_sequence_timeline,
                        current_time,
                        &timeline_spec,
                        true,
                        current_high_boundary,
                    )
                })
                .inner;

            interacting_with_controls |= ui
                .horizontal(|ui| {
                    visible_history_boundary_ui(
                        ctx,
                        ui,
                        &mut visible_history.to,
                        is_sequence_timeline,
                        current_time,
                        &timeline_spec,
                        false,
                        current_low_boundary,
                    )
                })
                .inner;
        } else {
            resolved_visible_history_boundary_ui(
                ctx,
                ui,
                &resolved_visible_history.from,
                is_sequence_timeline,
                true,
            );
            resolved_visible_history_boundary_ui(
                ctx,
                ui,
                &resolved_visible_history.to,
                is_sequence_timeline,
                false,
            );
        }

        current_range_ui(ctx, ui, current_time, is_sequence_timeline, visible_history);

        ui.add(
            egui::Label::new(
                egui::RichText::new(if is_sequence_timeline {
                    "These settings apply to all sequence timelines."
                } else {
                    "These settings apply to all temporal timelines."
                })
                .italics()
                .weak(),
            )
            .wrap(true),
        )
        .on_hover_text(
            "Visible Time Range properties are stored separately for each types of timelines. \
            They may differ depending on whether the current timeline is temporal or a sequence.",
        );
    });

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
            let visible_history = match (visible_history_prop.enabled, is_sequence_timeline) {
                (true, true) => visible_history_prop.sequences,
                (true, false) => visible_history_prop.nanos,
                (false, true) => resolved_visible_history_prop.sequences,
                (false, false) => resolved_visible_history_prop.nanos,
            };

            ctx.rec_cfg.time_ctrl.write().highlighted_range =
                Some(visible_history.time_range(current_time));
        }
    }

    collapsing_response.header_response.on_hover_text(
        "Controls the time range used to display data in the Space View.\n\n\
        Note that the data current as of the time range starting time is included.",
    );
}

fn current_range_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut Ui,
    current_time: i64,
    is_sequence_timeline: bool,
    visible_history: &VisibleHistory,
) {
    let (time_type, quantity_name) = if is_sequence_timeline {
        (TimeType::Sequence, "frame")
    } else {
        (TimeType::Time, "time")
    };

    let time_range = visible_history.time_range(current_time.into());
    let from_formatted = time_type.format(time_range.min, ctx.app_options.time_zone_for_timestamps);

    ui.label(format!(
        "Showing data between {quantity_name}s {from_formatted} and {} (included).",
        time_type.format(time_range.max, ctx.app_options.time_zone_for_timestamps)
    ));
}

#[allow(clippy::too_many_arguments)]
fn resolved_visible_history_boundary_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    visible_history_boundary: &VisibleHistoryBoundary,
    is_sequence_timeline: bool,
    low_bound: bool,
) {
    let from_to = if low_bound { "From" } else { "To" };
    let boundary_type = match visible_history_boundary {
        VisibleHistoryBoundary::RelativeToTimeCursor(_) => {
            if is_sequence_timeline {
                "current frame"
            } else {
                "current time"
            }
        }
        VisibleHistoryBoundary::Absolute(_) => {
            if is_sequence_timeline {
                "frame"
            } else {
                "absolute time"
            }
        }
        VisibleHistoryBoundary::Infinite => {
            if low_bound {
                "the beginning of the timeline"
            } else {
                "the end of the timeline"
            }
        }
    };

    let mut label = format!("{from_to} {boundary_type}");

    match visible_history_boundary {
        VisibleHistoryBoundary::RelativeToTimeCursor(offset) => {
            if *offset != 0 {
                if is_sequence_timeline {
                    label += &format!(
                        " with {offset} frame{} offset",
                        if offset.abs() > 1 { "s" } else { "" }
                    );
                } else {
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
            }
        }
        VisibleHistoryBoundary::Absolute(time) => {
            let time_type = if is_sequence_timeline {
                TimeType::Sequence
            } else {
                TimeType::Time
            };

            label += &format!(
                " {}",
                time_type.format((*time).into(), ctx.app_options.time_zone_for_timestamps)
            );
        }
        VisibleHistoryBoundary::Infinite => {}
    }

    ui.label(label);
}

fn visible_history_boundary_combo_label(
    boundary: &VisibleHistoryBoundary,
    is_sequence_timeline: bool,
    low_bound: bool,
) -> &'static str {
    match boundary {
        VisibleHistoryBoundary::RelativeToTimeCursor(_) => {
            if is_sequence_timeline {
                "current frame with offset"
            } else {
                "current time with offset"
            }
        }
        VisibleHistoryBoundary::Absolute(_) => {
            if is_sequence_timeline {
                "absolute frame"
            } else {
                "absolute time"
            }
        }
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
    is_sequence_timeline: bool,
    current_time: i64,
    timeline_spec: &TimelineSpec,
    low_bound: bool,
    other_boundary_absolute: i64,
) -> bool {
    ui.label(if low_bound { "From" } else { "To" });

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
        is_sequence_timeline,
        low_bound,
    ))
    .show_ui(ui, |ui| {
        ui.set_min_width(160.0);

        ui.selectable_value(
            visible_history_boundary,
            rel_time,
            visible_history_boundary_combo_label(&rel_time, is_sequence_timeline, low_bound),
        )
        .on_hover_text(if low_bound {
            "Show data from a time point relative to the current time."
        } else {
            "Show data until a time point relative to the current time."
        });
        ui.selectable_value(
            visible_history_boundary,
            abs_time,
            visible_history_boundary_combo_label(&abs_time, is_sequence_timeline, low_bound),
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
                is_sequence_timeline,
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

            if is_sequence_timeline {
                Some(
                    timeline_spec
                        .sequence_drag_value(ui, value, false, low_bound_override)
                        .on_hover_text(
                            "Number of frames before/after the current time to use a time \
                        range boundary",
                        ),
                )
            } else {
                Some(
                    timeline_spec
                        .temporal_drag_value(
                            ui,
                            value,
                            false,
                            low_bound_override,
                            ctx.app_options.time_zone_for_timestamps,
                        )
                        .0
                        .on_hover_text(
                            "Time duration before/after the current time to use as time range \
                                boundary",
                        ),
                )
            }
        }
        VisibleHistoryBoundary::Absolute(value) => {
            // see note above
            let low_bound_override = if !low_bound {
                Some(other_boundary_absolute)
            } else {
                None
            };

            if is_sequence_timeline {
                Some(
                    timeline_spec
                        .sequence_drag_value(ui, value, true, low_bound_override)
                        .on_hover_text("Absolute frame number to use as time range boundary"),
                )
            } else {
                let (drag_resp, base_time_resp) = timeline_spec.temporal_drag_value(
                    ui,
                    value,
                    true,
                    low_bound_override,
                    ctx.app_options.time_zone_for_timestamps,
                );

                if let Some(base_time_resp) = base_time_resp {
                    base_time_resp.on_hover_text("Base time used to set time range boundaries");
                }

                Some(drag_resp.on_hover_text("Absolute time to use as time range boundary"))
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
