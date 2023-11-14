use egui::NumExt as _;
use re_data_store::{ExtraQueryHistory, VisibleHistory, VisibleHistoryBoundary};
use re_log_types::{EntityPath, TimeType};
use re_space_view_spatial::{SpatialSpaceView2D, SpatialSpaceView3D};
use re_space_view_time_series::TimeSeriesSpaceView;
use re_types_core::ComponentName;
use re_viewer_context::{Item, SpaceViewClassName, ViewerContext};
use re_viewport::Viewport;
use std::collections::HashSet;
use std::ops::RangeInclusive;

/// These space views support the Visible History feature.
static VISIBLE_HISTORY_SUPPORTED_SPACE_VIEWS: once_cell::sync::Lazy<HashSet<SpaceViewClassName>> =
    once_cell::sync::Lazy::new(|| {
        [
            SpatialSpaceView3D::NAME,
            SpatialSpaceView2D::NAME,
            TimeSeriesSpaceView::NAME,
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

// TODO(#4145): This method is obviously unfortunate. It's a temporary solution until the ViewPart
// system is able to report its ability to handle the visible history feature.
pub fn has_visible_history_section(
    ctx: &mut ViewerContext<'_>,
    space_view_class: &SpaceViewClassName,
    entity_path: Option<&EntityPath>,
) -> bool {
    if !VISIBLE_HISTORY_SUPPORTED_SPACE_VIEWS.contains(space_view_class) {
        return false;
    }

    if let Some(entity_path) = entity_path {
        let store = ctx.store_db.store();
        let component_names = store.all_components(ctx.rec_cfg.time_ctrl.timeline(), entity_path);
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

pub fn visible_history_section_ui(
    ui: &mut egui::Ui,
    ctx: &mut ViewerContext<'_>,
    viewport: &mut Viewport<'_, '_>,
    item: &Item,
) {
    match item {
        Item::ComponentPath(_) => {}
        Item::SpaceView(space_view_id) => {
            if let Some(space_view) = viewport.blueprint.space_view_mut(space_view_id) {
                let space_view_class = *space_view.class_name();

                // Space Views don't inherit properties
                let projected_visible_history = ExtraQueryHistory::default();

                visible_history_ui_impl(
                    ctx,
                    ui,
                    &space_view_class,
                    true,
                    None,
                    &projected_visible_history,
                    &mut space_view.root_entity_properties.visible_history,
                );
            }
        }

        Item::InstancePath(space_view_id, instance_path) => {
            if let Some(space_view_id) = space_view_id {
                if let Some(space_view) = viewport.blueprint.space_view_mut(space_view_id) {
                    if !instance_path.instance_key.is_specific() {
                        let space_view_class = *space_view.class_name();
                        let entity_path = &instance_path.entity_path;
                        let projected_props = space_view
                            .contents
                            .data_blueprints_projected()
                            .get(entity_path);
                        let data_blueprint = space_view.contents.data_blueprints_individual();
                        let mut props = data_blueprint.get(entity_path);

                        visible_history_ui_impl(
                            ctx,
                            ui,
                            &space_view_class,
                            false,
                            Some(&instance_path.entity_path),
                            &projected_props.visible_history,
                            &mut props.visible_history,
                        );

                        data_blueprint.set(instance_path.entity_path.clone(), props);
                    }
                }
            }
        }

        Item::DataBlueprintGroup(space_view_id, data_blueprint_group_handle) => {
            if let Some(space_view) = viewport.blueprint.space_view_mut(space_view_id) {
                let space_view_class = *space_view.class_name();
                if let Some(group) = space_view.contents.group_mut(*data_blueprint_group_handle) {
                    visible_history_ui_impl(
                        ctx,
                        ui,
                        &space_view_class,
                        false,
                        None,
                        &group.properties_projected.visible_history,
                        &mut group.properties_individual.visible_history,
                    );
                }
            }
        }
    }
}

fn visible_history_ui_impl(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    space_view_class: &SpaceViewClassName,
    is_space_view: bool,
    entity_path: Option<&EntityPath>,
    projected_visible_history_prop: &ExtraQueryHistory,
    visible_history_prop: &mut ExtraQueryHistory,
) {
    if !has_visible_history_section(ctx, space_view_class, entity_path) {
        return;
    }

    let re_ui = ctx.re_ui;

    re_ui.large_collapsing_header(ui, "Visible Time Range", true, |ui| {
        ui.horizontal(|ui| {
            re_ui.radio_value(
                ui,
                &mut visible_history_prop.enabled,
                false,
                if is_space_view {
                    "Default"
                } else {
                    "Inherited"
                },
            );
            re_ui.radio_value(ui, &mut visible_history_prop.enabled, true, "Override");
        });

        let time_range = if let Some(times) = ctx
            .store_db
            .time_histogram(ctx.rec_cfg.time_ctrl.timeline())
        {
            times.min_key().unwrap_or_default()..=times.max_key().unwrap_or_default()
        } else {
            0..=0
        };

        let current_time = ctx
            .rec_cfg
            .time_ctrl
            .time_i64()
            .unwrap_or_default()
            .at_least(*time_range.start()); // accounts for timeless time (TimeInt::BEGINNING)

        let min_time = *time_range.start();

        let sequence_timeline =
            matches!(ctx.rec_cfg.time_ctrl.timeline().typ(), TimeType::Sequence);

        let (projected_visible_history, visible_history) = if sequence_timeline {
            (
                &projected_visible_history_prop.sequences,
                &mut visible_history_prop.sequences,
            )
        } else {
            (
                &projected_visible_history_prop.nanos,
                &mut visible_history_prop.nanos,
            )
        };

        if visible_history_prop.enabled {
            let current_low_boundary = visible_history.from(current_time.into()).as_i64();
            let current_high_boundary = visible_history.to(current_time.into()).as_i64();

            ui.horizontal(|ui| {
                visible_history_boundary_ui(
                    ctx,
                    re_ui,
                    ui,
                    &mut visible_history.from,
                    sequence_timeline,
                    current_time,
                    time_range.clone(),
                    min_time,
                    true,
                    current_high_boundary,
                );
            });

            ui.horizontal(|ui| {
                visible_history_boundary_ui(
                    ctx,
                    re_ui,
                    ui,
                    &mut visible_history.to,
                    sequence_timeline,
                    current_time,
                    time_range,
                    min_time,
                    false,
                    current_low_boundary,
                );
            });
        } else {
            // TODO(#4194): it should be the responsibility of the space view to provide defaults for entity props
            let (from_boundary, to_boundary) = if !projected_visible_history_prop.enabled
                && space_view_class == TimeSeriesSpaceView::NAME
            {
                // Contrary to other space views, Timeseries space view do not act like
                // `VisibleHistory::default()` when its disabled. Instead, behaves like
                // `VisibleHistory::ALL` instead.
                (&VisibleHistory::ALL.from, &VisibleHistory::ALL.to)
            } else {
                (
                    &projected_visible_history.from,
                    &projected_visible_history.to,
                )
            };

            projected_visible_history_boundary_ui(ctx, ui, from_boundary, sequence_timeline, true);
            projected_visible_history_boundary_ui(ctx, ui, to_boundary, sequence_timeline, false);
        }

        ui.add(
            egui::Label::new(
                egui::RichText::new(if sequence_timeline {
                    "These settings apply to all sequence timelines."
                } else {
                    "These settings apply to all temporal timelines."
                })
                .italics()
                .weak(),
            )
            .wrap(true),
        );
    });
}

#[allow(clippy::too_many_arguments)]
fn projected_visible_history_boundary_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    visible_history_boundary: &VisibleHistoryBoundary,
    sequence_timeline: bool,
    low_bound: bool,
) {
    let from_to = if low_bound { "From" } else { "To" };
    let boundary_type = match visible_history_boundary {
        VisibleHistoryBoundary::RelativeToTimeCursor(_) => {
            if sequence_timeline {
                "current frame"
            } else {
                "current time"
            }
        }
        VisibleHistoryBoundary::Absolute(_) => {
            if sequence_timeline {
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
                if sequence_timeline {
                    label += &format!(
                        "with {offset} frame{} offset",
                        if offset.abs() > 1 { "s" } else { "" }
                    );
                } else {
                    // This looks like it should be generically handled somewhere like re_format,
                    // but this actually is rather ad hoc and works thanks to egui::DragValue
                    // biasing towards round numbers and the auto-scaling feature of
                    // `ReUi::time_drag_value()`.
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
            let time_type = if sequence_timeline {
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
    sequence_timeline: bool,
    low_bound: bool,
) -> &'static str {
    match boundary {
        VisibleHistoryBoundary::RelativeToTimeCursor(_) => {
            if sequence_timeline {
                "current frame with offset"
            } else {
                "current time with offset"
            }
        }
        VisibleHistoryBoundary::Absolute(_) => {
            if sequence_timeline {
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
    ctx: &mut ViewerContext<'_>,
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    visible_history_boundary: &mut VisibleHistoryBoundary,
    sequence_timeline: bool,
    current_time: i64,
    mut time_range: RangeInclusive<i64>,
    min_time: i64,
    low_bound: bool,
    other_boundary_absolute: i64,
) {
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
        sequence_timeline,
        low_bound,
    ))
    .show_ui(ui, |ui| {
        ui.set_min_width(160.0);

        ui.selectable_value(
            visible_history_boundary,
            rel_time,
            visible_history_boundary_combo_label(&rel_time, sequence_timeline, low_bound),
        )
        .on_hover_text(if low_bound {
            "Show data from a time point relative to the current time."
        } else {
            "Show data until a time point relative to the current time."
        });
        ui.selectable_value(
            visible_history_boundary,
            abs_time,
            visible_history_boundary_combo_label(&abs_time, sequence_timeline, low_bound),
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
                sequence_timeline,
                low_bound,
            ),
        )
        .on_hover_text(if low_bound {
            "Show data from the beginning of the timeline"
        } else {
            "Show data until the end of the timeline"
        });
    });

    let span = time_range.end() - time_range.start();

    // Hot "usability" area! This achieves two things:
    // 1) It makes sure the time range in relative mode has enough margin beyond the current
    //    timeline's span to avoid the boundary value to be modified by changing the current time
    //    cursor
    // 2) It makes sure the two boundaries don't cross in time (i.e. low > high). It does so by
    //    prioritizing the low boundary. Moving the low boundary against the high boundary will
    //    displace the high boundary. On the other hand, the high boundary cannot be moved against
    //    the low boundary. This asymmetry is intentional, and avoids both boundaries fighting each
    //    other in some corner cases (when the user interacts with the current time cursor).
    #[allow(clippy::collapsible_else_if)] // for readability
    if matches!(
        visible_history_boundary,
        VisibleHistoryBoundary::RelativeToTimeCursor(_)
    ) {
        if low_bound {
            time_range = -span..=2 * span;
        } else {
            time_range =
                (other_boundary_absolute.saturating_sub(current_time)).at_least(-span)..=2 * span;
        }
    } else {
        if !low_bound {
            time_range = other_boundary_absolute.at_least(*time_range.start())..=*time_range.end();
        }
    }

    match visible_history_boundary {
        VisibleHistoryBoundary::RelativeToTimeCursor(value) => editable_boundary_ui(
            ctx,
            re_ui,
            ui,
            value,
            sequence_timeline,
            false,
            time_range,
            min_time,
        ),
        VisibleHistoryBoundary::Absolute(value) => editable_boundary_ui(
            ctx,
            re_ui,
            ui,
            value,
            sequence_timeline,
            true,
            time_range,
            min_time,
        ),
        VisibleHistoryBoundary::Infinite => {}
    }
}

// ---

#[allow(clippy::too_many_arguments)]
fn editable_boundary_ui(
    ctx: &mut ViewerContext<'_>,
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    value: &mut i64,
    sequence_timeline: bool,
    absolute: bool,
    time_range: RangeInclusive<i64>,
    min_time: i64,
) {
    if sequence_timeline {
        let span = time_range.end() - time_range.start();
        let speed = (span as f32 * 0.005).at_least(1.0);

        ui.add(
            egui::DragValue::new(value)
                .clamp_range(time_range)
                .speed(speed),
        );
    } else {
        time_drag_value(ctx, re_ui, ui, value, absolute, &time_range, min_time);
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

fn time_drag_value(
    ctx: &mut ViewerContext<'_>,
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    value: &mut i64,
    absolute: bool,
    time_range: &RangeInclusive<i64>,
    min_time: i64,
) {
    let base_time = if absolute {
        time_range_base_time(min_time, *time_range.end() - *time_range.start())
    } else {
        None
    };

    if let Some(base_time) = base_time {
        ui.label(format!(
            "{} + ",
            TimeType::Time.format(base_time.into(), ctx.app_options.time_zone_for_timestamps)
        ));
        time_drag_value_with_base_time(re_ui, ui, value, time_range, base_time);
    } else {
        re_ui.time_drag_value(ui, value, time_range);
    }
}

/// Wrapper over [`re_ui::ReUi::time_drag_value`] that first subtract an offset to the edited time.
fn time_drag_value_with_base_time(
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    value: &mut i64,
    time_range: &RangeInclusive<i64>,
    base_time: i64,
) {
    let time_range = time_range.start() - base_time..=time_range.end() - base_time;
    let mut offset_value = *value - base_time;
    re_ui.time_drag_value(ui, &mut offset_value, &time_range);
    *value = offset_value + base_time;
}
