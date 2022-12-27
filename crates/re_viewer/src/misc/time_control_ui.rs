use std::collections::BTreeMap;

use egui::NumExt as _;

use re_data_store::TimesPerTimeline;
use re_log_types::{TimeInt, TimeRangeF, TimeReal, TimeType};

use super::time_control::{Looping, TimeControl};

impl TimeControl {
    pub fn timeline_selector_ui(
        &mut self,
        times_per_timeline: &TimesPerTimeline,
        ui: &mut egui::Ui,
    ) {
        self.select_a_valid_timeline(times_per_timeline);

        egui::ComboBox::from_id_source("timeline")
            .selected_text(self.timeline().name().as_str())
            .show_ui(ui, |ui| {
                for timeline in times_per_timeline.timelines() {
                    if ui
                        .selectable_label(timeline == self.timeline(), timeline.name().as_str())
                        .clicked()
                    {
                        self.set_timeline(*timeline);
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

    pub fn play_pause_ui(
        &mut self,
        re_ui: &re_ui::ReUi,
        times_per_timeline: &TimesPerTimeline,
        ui: &mut egui::Ui,
    ) {
        self.play_button_ui(re_ui, ui, times_per_timeline);
        self.pause_button_ui(re_ui, ui);
        self.step_time_button_ui(re_ui, ui, times_per_timeline);
        self.loop_button_ui(re_ui, ui);
        self.playback_speed_ui(ui);
    }

    fn play_button_ui(
        &mut self,
        re_ui: &re_ui::ReUi,
        ui: &mut egui::Ui,
        times_per_timeline: &TimesPerTimeline,
    ) {
        if re_ui
            .large_button_selected(ui, &re_ui::icons::PLAY, self.is_playing())
            .on_hover_text(format!("Play.{}", toggle_playback_text(ui.ctx())))
            .clicked()
        {
            self.play(times_per_timeline);
        }
    }

    fn pause_button_ui(&mut self, re_ui: &re_ui::ReUi, ui: &mut egui::Ui) {
        if re_ui
            .large_button_selected(ui, &re_ui::icons::PAUSE, !self.is_playing())
            .on_hover_text(format!("Pause.{}", toggle_playback_text(ui.ctx())))
            .clicked()
        {
            self.pause();
        }
    }

    fn step_time_button_ui(
        &mut self,
        re_ui: &re_ui::ReUi,
        ui: &mut egui::Ui,
        times_per_timeline: &TimesPerTimeline,
    ) {
        let Some(time_values) = times_per_timeline.get(self.timeline()) else { return; };

        let anything_has_kb_focus = ui.ctx().memory().focus().is_some();
        let step_back = re_ui
            .large_button(ui, &re_ui::icons::ARROW_LEFT)
            .on_hover_text("Step back to previous time with any new data (left arrow)")
            .clicked();
        let step_back = step_back
            || !anything_has_kb_focus
                && ui
                    .input_mut()
                    .consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft);

        let step_fwd = re_ui
            .large_button(ui, &re_ui::icons::ARROW_RIGHT)
            .on_hover_text("Step forwards to next time with any new data (right arrow)")
            .clicked();
        let step_fwd = step_fwd
            || !anything_has_kb_focus
                && ui
                    .input_mut()
                    .consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight);

        if step_back || step_fwd {
            self.pause();

            if let Some(time) = self.time() {
                #[allow(clippy::collapsible_else_if)]
                let new_time = if let Some(loop_range) = self.active_loop_selection() {
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

    fn loop_button_ui(&mut self, re_ui: &re_ui::ReUi, ui: &mut egui::Ui) {
        let icon = &re_ui::icons::LOOP;

        ui.scope(|ui| {
            // Loop-button cycles between states:
            match self.looping {
                Looping::Off => {
                    if re_ui
                        .large_button_selected(ui, icon, false)
                        .on_hover_text("Looping is off")
                        .clicked()
                    {
                        self.looping = Looping::All;
                    }
                }
                Looping::All => {
                    if re_ui
                        .large_button_selected(ui, icon, true)
                        .on_hover_text("Looping entire recording")
                        .clicked()
                    {
                        self.looping = Looping::Selection;
                    }
                }
                Looping::Selection => {
                    ui.visuals_mut().selection.bg_fill = re_ui::ReUi::loop_selection_color();
                    #[allow(clippy::collapsible_else_if)]
                    if re_ui
                        .large_button_selected(ui, icon, true)
                        .on_hover_text("Looping selection")
                        .clicked()
                    {
                        self.looping = Looping::Off;
                    }
                }
            }
        });
    }

    fn playback_speed_ui(&mut self, ui: &mut egui::Ui) {
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
}

fn min<T>(values: &BTreeMap<TimeInt, T>) -> TimeInt {
    *values.keys().next().unwrap()
}

fn max<T>(values: &BTreeMap<TimeInt, T>) -> TimeInt {
    *values.keys().rev().next().unwrap()
}

fn step_fwd_time<T>(time: TimeReal, values: &BTreeMap<TimeInt, T>) -> TimeInt {
    if let Some(next) = values
        .range((
            std::ops::Bound::Excluded(time.floor()),
            std::ops::Bound::Unbounded,
        ))
        .next()
    {
        *next.0
    } else {
        min(values)
    }
}

fn step_back_time<T>(time: TimeReal, values: &BTreeMap<TimeInt, T>) -> TimeInt {
    if let Some(previous) = values.range(..time.ceil()).rev().next() {
        *previous.0
    } else {
        max(values)
    }
}

fn step_fwd_time_looped<T>(
    time: TimeReal,
    values: &BTreeMap<TimeInt, T>,
    loop_range: &TimeRangeF,
) -> TimeReal {
    if time < loop_range.min || loop_range.max <= time {
        loop_range.min
    } else if let Some(next) = values
        .range((
            std::ops::Bound::Excluded(time.floor()),
            std::ops::Bound::Included(loop_range.max.floor()),
        ))
        .next()
    {
        TimeReal::from(*next.0)
    } else {
        step_fwd_time(time, values).into()
    }
}

fn step_back_time_looped<T>(
    time: TimeReal,
    values: &BTreeMap<TimeInt, T>,
    loop_range: &TimeRangeF,
) -> TimeReal {
    if time <= loop_range.min || loop_range.max < time {
        loop_range.max
    } else if let Some(previous) = values
        .range(loop_range.min.ceil()..time.ceil())
        .rev()
        .next()
    {
        TimeReal::from(*previous.0)
    } else {
        step_back_time(time, values).into()
    }
}

fn toggle_playback_text(egui_ctx: &egui::Context) -> String {
    if let Some(shortcut) = re_ui::Command::PlaybackTogglePlayPause.kb_shortcut() {
        format!(" Toggle with {}", egui_ctx.format_shortcut(&shortcut))
    } else {
        Default::default()
    }
}
