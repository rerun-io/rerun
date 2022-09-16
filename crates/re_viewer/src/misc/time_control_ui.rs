use std::collections::BTreeSet;

use egui::NumExt as _;

use re_log_types::*;

use super::{log_db::TimePoints, time_control::*, TimeRangeF, TimeReal};

impl TimeControl {
    pub fn time_source_selector_ui(&mut self, time_source_axes: &TimePoints, ui: &mut egui::Ui) {
        self.select_a_valid_time_source(time_source_axes);

        egui::ComboBox::from_id_source("time_source")
            .selected_text(self.source().name().as_str())
            .show_ui(ui, |ui| {
                for source in time_source_axes.0.keys() {
                    if ui
                        .selectable_label(source == self.source(), source.name().as_str())
                        .clicked()
                    {
                        self.set_source(*source);
                    }
                }
            });

        if self.time_type() == TimeType::Sequence {
            if let Some(mut fps) = self.fps() {
                ui.add(
                    egui::DragValue::new(&mut fps)
                        .prefix("FPS: ")
                        .speed(1)
                        .clamp_range(0.0..=f32::INFINITY),
                )
                .on_hover_text("Frames Per Second");
                self.set_fps(fps);
            }
        }
    }

    pub fn selection_ui(&mut self, ui: &mut egui::Ui) {
        use egui::SelectableLabel;

        ui.label("Selection:");

        let has_selection = self.has_selection();

        if !has_selection {
            self.selection_active = false;
        }

        if ui
            .add(SelectableLabel::new(!self.selection_active, "None"))
            .on_hover_text("Disable selection")
            .clicked()
        {
            self.selection_active = false;
        }

        ui.scope(|ui| {
            ui.visuals_mut().selection.bg_fill = TimeSelectionType::Loop.color(ui.visuals());

            let is_looping =
                self.selection_active && self.selection_type == TimeSelectionType::Loop;

            if ui
                .add_enabled(has_selection, SelectableLabel::new(is_looping, "🔁"))
                .on_hover_text("Loop in selection")
                .clicked()
            {
                if is_looping {
                    self.selection_active = false; // toggle off
                } else {
                    self.set_active_selection_type(Some(TimeSelectionType::Loop));
                }
            }
        });

        ui.scope(|ui| {
            ui.visuals_mut().selection.bg_fill = TimeSelectionType::Filter.color(ui.visuals());

            let is_filtering =
                self.selection_active && self.selection_type == TimeSelectionType::Filter;

            if ui
                .add_enabled(has_selection, SelectableLabel::new(is_filtering, "⬌"))
                .on_hover_text("Show everything in selection")
                .clicked()
            {
                if is_filtering {
                    self.selection_active = false; // toggle off
                } else {
                    self.set_active_selection_type(Some(TimeSelectionType::Filter));
                    self.pause();
                }
            }
        });
    }

    pub fn play_pause_ui(&mut self, time_points: &TimePoints, ui: &mut egui::Ui) {
        // Toggle with space
        let anything_has_focus = ui.ctx().memory().focus().is_some();
        if !anything_has_focus
            && ui
                .input_mut()
                .consume_key(Default::default(), egui::Key::Space)
        {
            if self.is_playing() {
                self.pause();
            } else {
                self.play(time_points);
            }
        }

        if ui
            .selectable_label(self.is_playing(), "▶")
            .on_hover_text("Play. Toggle with SPACE")
            .clicked()
        {
            self.play(time_points);
        }
        if ui
            .selectable_label(!self.is_playing(), "⏸")
            .on_hover_text("Pause. Toggle with SPACE")
            .clicked()
        {
            self.pause();
        }

        {
            let mut looped = self.looped();
            ui.scope(|ui| {
                ui.visuals_mut().selection.bg_fill = TimeSelectionType::Loop.color(ui.visuals());
                ui.toggle_value(&mut looped, "🔁")
                    .on_hover_text("Loop playback");
            });
            if !looped && self.selection_type == TimeSelectionType::Loop {
                self.selection_active = false;
            }
            self.set_looped(looped);
        }

        {
            let mut speed = self.speed();
            let drag_speed = (speed * 0.02).at_least(0.01);
            ui.add(
                egui::DragValue::new(&mut speed)
                    .speed(drag_speed)
                    .suffix("x"),
            )
            .on_hover_text("Playback speed.");
            self.set_speed(speed);
        }

        if let Some(time_values) = time_points.0.get(self.source()) {
            let anything_has_kb_focus = ui.ctx().memory().focus().is_some();
            let step_back = ui
                .button("⏴")
                .on_hover_text("Step back to previous time with any new data (left arrow)")
                .clicked();
            let step_back = step_back
                || !anything_has_kb_focus
                    && ui
                        .input_mut()
                        .consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft);

            let step_fwd = ui
                .button("⏵")
                .on_hover_text("Step forwards to next time with any new data (right arrow)")
                .clicked();
            let step_fwd = step_fwd
                || !anything_has_kb_focus
                    && ui
                        .input_mut()
                        .consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight);

            if step_back || step_fwd {
                self.pause();

                if let Some(time_range) = self.time_filter_range() {
                    let length = time_range.length();
                    let new_min = if step_back {
                        step_back_time(time_range.min, time_values)
                    } else {
                        step_fwd_time(time_range.min, time_values)
                    };
                    let new_min = TimeReal::from(new_min);
                    let new_max = new_min + length;
                    self.set_time_selection(TimeRangeF::new(new_min, new_max));
                } else if let Some(time) = self.time() {
                    #[allow(clippy::collapsible_else_if)]
                    let new_time = if let Some(loop_range) = self.loop_range() {
                        if step_back {
                            step_back_time_looped(time, time_values, &loop_range)
                        } else {
                            step_fwd_time_looped(time, time_values, &loop_range)
                        }
                    } else {
                        if step_back {
                            step_back_time(time, time_values).into()
                        } else {
                            step_fwd_time(time, time_values).into()
                        }
                    };
                    self.set_time(new_time);
                }
            }
        }
    }
}

fn min(values: &BTreeSet<TimeInt>) -> TimeInt {
    *values.iter().next().unwrap()
}

fn max(values: &BTreeSet<TimeInt>) -> TimeInt {
    *values.iter().rev().next().unwrap()
}

fn step_fwd_time(time: TimeReal, values: &BTreeSet<TimeInt>) -> TimeInt {
    if let Some(next) = values
        .range((
            std::ops::Bound::Excluded(TimeInt::from(time)),
            std::ops::Bound::Unbounded,
        ))
        .next()
    {
        *next
    } else {
        min(values)
    }
}

fn step_back_time(time: TimeReal, values: &BTreeSet<TimeInt>) -> TimeInt {
    if let Some(previous) = values.range(..TimeInt::from(time)).rev().next() {
        *previous
    } else {
        max(values)
    }
}

fn step_fwd_time_looped(
    time: TimeReal,
    values: &BTreeSet<TimeInt>,
    loop_range: &TimeRangeF,
) -> TimeReal {
    if time < loop_range.min || loop_range.max <= time {
        loop_range.min
    } else if let Some(next) = values
        .range((
            std::ops::Bound::Excluded(TimeInt::from(time)),
            std::ops::Bound::Included(TimeInt::from(loop_range.max)),
        ))
        .next()
    {
        TimeReal::from(*next)
    } else {
        step_fwd_time(time, values).into()
    }
}

fn step_back_time_looped(
    time: TimeReal,
    values: &BTreeSet<TimeInt>,
    loop_range: &TimeRangeF,
) -> TimeReal {
    if time <= loop_range.min || loop_range.max < time {
        loop_range.max
    } else if let Some(previous) = values
        .range(TimeInt::from(loop_range.min)..TimeInt::from(time))
        .rev()
        .next()
    {
        TimeReal::from(*previous)
    } else {
        step_back_time(time, values).into()
    }
}
