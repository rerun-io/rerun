use re_log_types::{ResolvedTimeRange, TimeInt, TimeType, TimelineName};
use re_ui::{list_item, UiExt};
use re_viewer_context::{TimeDragValue, ViewerContext};

use crate::view_query::QueryMode;

/// Helper to handle the UI for the various query modes are they are shown to the user.
///
/// This struct is the "UI equivalent" of the [`QueryMode`] enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiQueryMode {
    LatestAt { time: TimeInt },
    TimeRangeAll,
    TimeRange { from: TimeInt, to: TimeInt },
}

impl UiQueryMode {
    /// Show the UI for the query mode selector.
    pub(crate) fn ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        time_drag_value: &TimeDragValue,
        timeline_name: &TimelineName,
        time_type: TimeType,
    ) -> bool {
        let orig_self = *self;

        ui.vertical(|ui| {
            //
            // LATEST AT
            //

            ui.horizontal(|ui| {
                let mut is_latest_at = matches!(self, Self::LatestAt { .. });

                let mut changed = ui
                    .re_radio_value(&mut is_latest_at, true, "Latest at")
                    .changed();

                if is_latest_at {
                    let mut time = if let Self::LatestAt { time } = self {
                        *time
                    } else {
                        TimeInt::MAX
                    }
                    .into();

                    changed |= match time_type {
                        TimeType::Time => time_drag_value
                            .temporal_drag_value_ui(
                                ui,
                                &mut time,
                                true,
                                None,
                                ctx.app_options.time_zone,
                            )
                            .0
                            .changed(),
                        TimeType::Sequence => time_drag_value
                            .sequence_drag_value_ui(ui, &mut time, true, None)
                            .changed(),
                    };

                    if changed {
                        *self = Self::LatestAt { time: time.into() };
                    }
                }
            });

            //
            // TIME RANGE ALL
            //

            ui.horizontal(|ui| {
                let mut is_time_range_all = matches!(self, Self::TimeRangeAll);
                if ui
                    .re_radio_value(&mut is_time_range_all, true, "From –∞ to +∞")
                    .changed()
                    && is_time_range_all
                {
                    *self = Self::TimeRangeAll;
                }
            });

            //
            // TIME RANGE CUSTOM
            //

            ui.vertical(|ui| {
                let mut is_time_range_custom = matches!(self, Self::TimeRange { .. });
                let mut changed = ui
                    .re_radio_value(&mut is_time_range_custom, true, "Define time range")
                    .changed();

                let mut should_display_time_range = false;

                if is_time_range_custom {
                    ui.spacing_mut().indent = ui.spacing().icon_width + ui.spacing().icon_spacing;
                    ui.indent("time_range_custom", |ui| {
                        let mut from = if let Self::TimeRange { from, .. } = self {
                            (*from).into()
                        } else {
                            (*time_drag_value.range.start()).into()
                        };

                        let mut to = if let Self::TimeRange { to, .. } = self {
                            (*to).into()
                        } else {
                            (*time_drag_value.range.end()).into()
                        };

                        list_item::list_item_scope(ui, "time_range_custom_scope", |ui| {
                            ui.list_item_flat_noninteractive(
                                list_item::PropertyContent::new("Start").value_fn(|ui, _| {
                                    let response = match time_type {
                                        TimeType::Time => {
                                            time_drag_value
                                                .temporal_drag_value_ui(
                                                    ui,
                                                    &mut from,
                                                    true,
                                                    None,
                                                    ctx.app_options.time_zone,
                                                )
                                                .0
                                        }
                                        TimeType::Sequence => time_drag_value
                                            .sequence_drag_value_ui(ui, &mut from, true, None),
                                    };

                                    changed |= response.changed();
                                    should_display_time_range |= response.hovered()
                                        || response.dragged()
                                        || response.has_focus();
                                }),
                            );

                            ui.list_item_flat_noninteractive(
                                list_item::PropertyContent::new("End").value_fn(|ui, _| {
                                    let response = match time_type {
                                        TimeType::Time => {
                                            time_drag_value
                                                .temporal_drag_value_ui(
                                                    ui,
                                                    &mut to,
                                                    true,
                                                    Some(from),
                                                    ctx.app_options.time_zone,
                                                )
                                                .0
                                        }
                                        TimeType::Sequence => time_drag_value
                                            .sequence_drag_value_ui(ui, &mut to, true, Some(from)),
                                    };

                                    changed |= response.changed();
                                    should_display_time_range |= response.hovered()
                                        || response.dragged()
                                        || response.has_focus();
                                }),
                            );
                        });

                        if changed {
                            *self = Self::TimeRange {
                                from: from.into(),
                                to: to.into(),
                            };
                        }

                        if should_display_time_range {
                            let mut time_ctrl = ctx.rec_cfg.time_ctrl.write();
                            if time_ctrl.timeline().name() == timeline_name {
                                time_ctrl.highlighted_range =
                                    Some(ResolvedTimeRange::new(from, to));
                            }
                        }
                    });
                }
            });
        });

        *self != orig_self
    }
}

impl From<QueryMode> for UiQueryMode {
    fn from(value: QueryMode) -> Self {
        match value {
            QueryMode::LatestAt { time } => Self::LatestAt { time },
            QueryMode::Range {
                from: TimeInt::MIN,
                to: TimeInt::MAX,
            } => Self::TimeRangeAll,
            QueryMode::Range { from, to } => Self::TimeRange { from, to },
        }
    }
}

impl From<UiQueryMode> for QueryMode {
    fn from(value: UiQueryMode) -> Self {
        match value {
            UiQueryMode::LatestAt { time } => Self::LatestAt { time },
            UiQueryMode::TimeRangeAll => Self::Range {
                from: TimeInt::MIN,
                to: TimeInt::MAX,
            },
            UiQueryMode::TimeRange { from, to } => Self::Range { from, to },
        }
    }
}
