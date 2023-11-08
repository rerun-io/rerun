//! Contains the UI for the Visible History feature.
//!
//! This file essentially provides the UI for the [`re_data_store::ExtraQueryHistory`] structure.

use egui::NumExt;
use re_data_store::{ExtraQueryHistory, VisibleHistoryBoundary};
use re_log_types::external::re_types_core::ComponentName;
use re_log_types::{EntityPath, TimeType};
use re_viewer_context::ViewerContext;
use std::ops::RangeInclusive;

pub fn visible_history_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_path: Option<&EntityPath>,
    visible_history_prop: &mut ExtraQueryHistory,
) {
    if !should_display_visible_history(ctx, entity_path) {
        return;
    }

    let re_ui = ctx.re_ui;

    re_ui
        .checkbox(ui, &mut visible_history_prop.enabled, "Visible history")
        .on_hover_text(
            "Enable Visible History.\n\nBy default, only the last state before the \
            current time is shown. By activating Visible History, all data within a \
            time window is shown instead.",
        );

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

    let sequence_timeline = matches!(ctx.rec_cfg.time_ctrl.timeline().typ(), TimeType::Sequence);

    let visible_history = if sequence_timeline {
        &mut visible_history_prop.sequences
    } else {
        &mut visible_history_prop.nanos
    };

    let visible_history_time_range = visible_history.from(current_time.into()).as_i64()
        ..=visible_history.to(current_time.into()).as_i64();

    ui.add_enabled_ui(visible_history_prop.enabled, |ui| {
        egui::Grid::new("visible_history_boundaries")
            .num_columns(4)
            .show(ui, |ui| {
                ui.label("From");
                visible_history_boundary_ui(
                    re_ui,
                    ui,
                    &mut visible_history.from,
                    sequence_timeline,
                    current_time,
                    time_range.clone(),
                    true,
                    *visible_history_time_range.end(),
                );

                ui.end_row();

                ui.label("To");
                visible_history_boundary_ui(
                    re_ui,
                    ui,
                    &mut visible_history.to,
                    sequence_timeline,
                    current_time,
                    time_range,
                    false,
                    *visible_history_time_range.start(),
                );

                ui.end_row();
            });
    });
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
}

static VISIBLE_HISTORY_COMPONENT_NAMES: once_cell::sync::Lazy<Vec<ComponentName>> =
    once_cell::sync::Lazy::new(|| {
        [
            ComponentName::from("rerun.components.Position2D"),
            ComponentName::from("rerun.components.Position3D"),
            ComponentName::from("rerun.components.LineStrip2D"),
            ComponentName::from("rerun.components.LineStrip3D"),
            ComponentName::from("rerun.components.TensorData"),
            ComponentName::from("rerun.components.Vector3D"),
            ComponentName::from("rerun.components.HalfSizes2D"),
            ComponentName::from("rerun.components.HalfSizes3D"),
        ]
        .into()
    });

// TODO(#4145): This method is obviously unfortunate. It's a temporary solution until the ViewPart
// system is able to report its ability to handle the visible history feature.
fn should_display_visible_history(
    ctx: &mut ViewerContext<'_>,
    entity_path: Option<&EntityPath>,
) -> bool {
    if let Some(entity_path) = entity_path {
        let store = ctx.store_db.store();
        let component_names = store.all_components(ctx.rec_cfg.time_ctrl.timeline(), entity_path);
        if let Some(component_names) = component_names {
            if !component_names
                .iter()
                .any(|name| VISIBLE_HISTORY_COMPONENT_NAMES.contains(name))
            {
                return false;
            }
        } else {
            return false;
        }
    }

    true
}

#[allow(clippy::too_many_arguments)]
fn visible_history_boundary_ui(
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    visible_history_boundary: &mut VisibleHistoryBoundary,
    sequence_timeline: bool,
    current_time: i64,
    mut time_range: RangeInclusive<i64>,
    low_bound: bool,
    other_boundary_absolute: i64,
) {
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
            time_range = other_boundary_absolute.at_least(-span)..=*time_range.end();
        }
    }

    match visible_history_boundary {
        VisibleHistoryBoundary::RelativeToTimeCursor(value)
        | VisibleHistoryBoundary::Absolute(value) => {
            if sequence_timeline {
                let speed = (span as f32 * 0.005).at_least(1.0);

                ui.add(
                    egui::DragValue::new(value)
                        .clamp_range(time_range)
                        .speed(speed),
                );
            } else {
                re_ui.time_drag_value(ui, value, &time_range);
            }
        }
        VisibleHistoryBoundary::Infinite => {
            let mut unused = 0.0;
            ui.add_enabled(
                false,
                egui::DragValue::new(&mut unused).custom_formatter(|_, _| "∞".to_owned()),
            );
        }
    }

    let (abs_time, rel_time) = match visible_history_boundary {
        VisibleHistoryBoundary::RelativeToTimeCursor(value) => (*value + current_time, *value),
        VisibleHistoryBoundary::Absolute(value) => (*value, *value - current_time),
        VisibleHistoryBoundary::Infinite => (current_time, 0),
    };

    egui::ComboBox::from_id_source(if low_bound {
        "time_history_low_bound"
    } else {
        "time_history_high_bound"
    })
    .selected_text(visible_history_boundary.label())
    .show_ui(ui, |ui| {
        ui.set_min_width(64.0);

        ui.selectable_value(
            visible_history_boundary,
            VisibleHistoryBoundary::RelativeToTimeCursor(rel_time),
            VisibleHistoryBoundary::RELATIVE_LABEL,
        )
        .on_hover_text(if low_bound {
            "Show data from a time point relative to the current time."
        } else {
            "Show data until a time point relative to the current time."
        });
        ui.selectable_value(
            visible_history_boundary,
            VisibleHistoryBoundary::Absolute(abs_time),
            VisibleHistoryBoundary::ABSOLUTE_LABEL,
        )
        .on_hover_text(if low_bound {
            "Show data from an absolute time point."
        } else {
            "Show data until an absolute time point."
        });
        ui.selectable_value(
            visible_history_boundary,
            VisibleHistoryBoundary::Infinite,
            VisibleHistoryBoundary::INFINITE_LABEL,
        )
        .on_hover_text(if low_bound {
            "Show data from the beginning of the timeline"
        } else {
            "Show data until the end of the timeline"
        });
    });
}
