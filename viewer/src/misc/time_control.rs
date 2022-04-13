use std::collections::BTreeMap;

use eframe::egui;
use egui::NumExt;
use log_types::*;

use crate::misc::TimePoints;

/// Controls the global view and progress of the time.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct TimeControl {
    /// Name of the time source (e.g. "log_time").
    time_source: String,

    /// The current/selected time for each time source.
    values: BTreeMap<String, TimeValue>,

    playing: bool,
    repeat: bool,
    speed: f32,
}

impl Default for TimeControl {
    fn default() -> Self {
        Self {
            time_source: Default::default(),
            values: Default::default(),
            playing: true,
            repeat: true,
            speed: 1.0,
        }
    }
}

impl TimeControl {
    pub fn time_source_selector(
        &mut self,
        time_source_axes: &TimePoints,
        ui: &mut egui::Ui,
    ) -> egui::Response {
        self.select_a_valid_time_source(time_source_axes);

        egui::ComboBox::from_id_source("time_source")
            .selected_text(&self.time_source)
            .show_ui(ui, |ui| {
                for source in time_source_axes.0.keys() {
                    if ui
                        .selectable_label(source == &self.time_source, source)
                        .clicked()
                    {
                        self.time_source = source.clone();
                    }
                }
            })
            .response
    }

    pub fn play_pause(&mut self, time_points: &TimePoints, ui: &mut egui::Ui) {
        // Toggle with space
        let anything_has_focus = ui.ctx().memory().focus().is_some();
        if !anything_has_focus
            && ui
                .input_mut()
                .consume_key(Default::default(), egui::Key::Space)
        {
            if self.playing {
                self.playing = false;
            } else {
                self.play(time_points);
            }
        }

        if ui
            .selectable_label(self.playing, "▶")
            .on_hover_text("Toggle with SPACE")
            .clicked()
        {
            self.play(time_points);
        }
        if ui
            .selectable_label(!self.playing, "⏸")
            .on_hover_text("Toggle with SPACE")
            .clicked()
        {
            self.playing = false;
        }
        if ui.selectable_label(self.repeat, "🔁").clicked() {
            self.repeat = !self.repeat;
        }

        ui.add(
            egui::Slider::new(&mut self.speed, 0.01..=100.0)
                .text("playback speed")
                .logarithmic(true),
        );
    }

    /// Update the current time
    pub fn move_time(&mut self, egui_ctx: &egui::Context, time_points: &TimePoints) {
        self.select_a_valid_time_source(time_points);

        if self.playing {
            if let Some(axis) = time_points.0.get(&self.time_source) {
                let (axis_min, axis_max) = (min(axis), max(axis));

                let value = self
                    .values
                    .entry(self.time_source.clone())
                    .or_insert(axis_min);

                match value {
                    TimeValue::Sequence(seq) => {
                        *seq += 1; // TODO: apply speed here somehow?
                    }
                    TimeValue::Time(time) => {
                        let dt = egui_ctx.input().unstable_dt.at_most(0.05);
                        *time += Duration::from_secs(dt * self.speed);
                    }
                }

                if *value > axis_max {
                    if self.repeat {
                        *value = axis_min;
                    } else {
                        *value = axis_max;
                        self.playing = false;
                    }
                }

                egui_ctx.request_repaint();
            }
        }
    }

    fn play(&mut self, time_points: &TimePoints) {
        if self.playing {
            return;
        }

        // Start from beginning if we are at the end:
        if let Some(time) = self.values.get_mut(&self.time_source) {
            if let Some(axis) = time_points.0.get(&self.time_source) {
                if *time >= max(axis) {
                    *time = min(axis);
                }
            }
        }
        self.playing = true;
    }

    fn select_a_valid_time_source(&mut self, time_points: &TimePoints) {
        for source in time_points.0.keys() {
            if &self.time_source == source {
                return; // it's valid
            }
        }
        if let Some(source) = time_points.0.keys().next() {
            self.time_source = source.clone();
        } else {
            self.time_source.clear();
        }
    }

    /// The currently selected time source
    pub fn source(&self) -> &str {
        &self.time_source
    }

    /// The current time
    pub fn time(&self) -> Option<TimeValue> {
        self.values.get(&self.time_source).copied()
    }

    pub fn set_source_and_time(&mut self, time_source: String, time: TimeValue) {
        self.time_source = time_source;
        self.values.insert(self.time_source.clone(), time);
    }

    pub fn set_time(&mut self, time: TimeValue) {
        self.values.insert(self.time_source.clone(), time);
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    /// Grouped by [`ObjectPath`], find the latest [`LogMsg`] that matches
    /// the current time source and is not after the current time.
    pub fn latest_of_each_object<'db>(
        &self,
        log_db: &'db crate::log_db::LogDb,
    ) -> Vec<&'db LogMsg> {
        crate::profile_function!();

        let current_time = if let Some(current_time) = self.time() {
            current_time
        } else {
            return Default::default();
        };
        let source = self.source();

        let mut latest: BTreeMap<&ObjectPath, (TimeValue, &LogMsg)> = BTreeMap::new();
        for (time_value, msg) in log_db
            .messages
            .values()
            .filter_map(|msg| {
                let time_value = *msg.time_point.0.get(source)?;
                Some((time_value, msg))
            })
            .filter(|(time_value, _msg)| time_value <= &current_time)
        {
            if let Some(existing) = latest.get_mut(&msg.object_path) {
                if existing.0 < time_value {
                    *existing = (time_value, msg);
                }
            } else {
                latest.insert(&msg.object_path, (time_value, msg));
            }
        }

        latest.values().map(|(_, msg)| *msg).collect()
    }
}

fn min(values: &std::collections::BTreeSet<TimeValue>) -> TimeValue {
    *values.iter().next().unwrap()
}

fn max(values: &std::collections::BTreeSet<TimeValue>) -> TimeValue {
    *values.iter().rev().next().unwrap()
}
