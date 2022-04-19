use std::collections::BTreeMap;

use eframe::egui;
use egui::NumExt;
use log_types::*;

use crate::misc::TimePoints;

/// The time range we are currently zoomed in on.
#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct TimeView {
    /// Where start of the the range.
    pub min: TimeValue,

    /// How much time the full view covers.
    ///
    /// The unit is either nanoseconds or sequence numbers.
    ///
    /// If there is gaps in the data, the actual amount of viewed time might be less.
    pub time_spanned: f64,
}

/// State per time source.
#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
struct TimeState {
    /// The current time
    time: TimeValue,

    /// The time range we are currently zoomed in on.
    ///
    /// `None` means "everything", and is the default value.
    /// In this case, the view will expand while new data is added.
    /// Only when the user actually zooms or pans will this be set.
    #[serde(default)]
    view: Option<TimeView>,
}

impl TimeState {
    fn new(time: TimeValue) -> Self {
        Self { time, view: None }
    }
}

/// Controls the global view and progress of the time.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct TimeControl {
    /// Name of the time source (e.g. "log_time").
    time_source: String,

    /// For each time source.
    states: BTreeMap<String, TimeState>,

    playing: bool,
    repeat: bool,
    speed: f32,
}

impl Default for TimeControl {
    fn default() -> Self {
        Self {
            time_source: Default::default(),
            states: Default::default(),
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

        let drag_speed = self.speed * 0.05;
        ui.add(
            egui::DragValue::new(&mut self.speed)
                .clamp_range(0.01..=100.0)
                .speed(drag_speed)
                .suffix("x"),
        )
        .on_hover_text("Playback speed.");
    }

    /// Update the current time
    pub fn move_time(&mut self, egui_ctx: &egui::Context, time_points: &TimePoints) {
        self.select_a_valid_time_source(time_points);

        if self.playing {
            if let Some(axis) = time_points.0.get(&self.time_source) {
                let (axis_min, axis_max) = (min(axis), max(axis));

                let state = self
                    .states
                    .entry(self.time_source.clone())
                    .or_insert_with(|| TimeState::new(axis_min));

                match &mut state.time {
                    TimeValue::Sequence(seq) => {
                        *seq += 1; // TODO: apply speed here somehow?
                    }
                    TimeValue::Time(time) => {
                        let dt = egui_ctx.input().unstable_dt.at_most(0.05);
                        *time += Duration::from_secs(dt * self.speed);
                    }
                }

                if state.time > axis_max {
                    if self.repeat {
                        state.time = axis_min;
                    } else {
                        state.time = axis_max;
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
        if let Some(axis) = time_points.0.get(&self.time_source) {
            if let Some(state) = self.states.get_mut(&self.time_source) {
                if state.time >= max(axis) {
                    state.time = min(axis);
                }
            } else {
                self.states
                    .insert(self.time_source.clone(), TimeState::new(min(axis)));
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
        self.states.get(&self.time_source).map(|state| state.time)
    }

    /// The range of time we are currently zoomed in on.
    pub fn time_view(&self) -> Option<TimeView> {
        self.states
            .get(&self.time_source)
            .and_then(|state| state.view)
    }

    /// The range of time we are currently zoomed in on.
    pub fn set_time_view(&mut self, view: TimeView) {
        self.states
            .entry(self.time_source.clone())
            .or_insert_with(|| TimeState::new(view.min))
            .view = Some(view);
    }

    /// The range of time we are currently zoomed in on.
    pub fn reset_time_view(&mut self) {
        if let Some(state) = self.states.get_mut(&self.time_source) {
            state.view = None;
        }
    }

    pub fn set_source_and_time(&mut self, time_source: String, time: TimeValue) {
        self.time_source = time_source;
        self.set_time(time);
    }

    pub fn set_time(&mut self, time: TimeValue) {
        self.states
            .entry(self.time_source.clone())
            .or_insert_with(|| TimeState::new(time))
            .time = time;
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

        log_db.latest_of_each_object(source, current_time)
    }
}

fn min(values: &std::collections::BTreeSet<TimeValue>) -> TimeValue {
    *values.iter().next().unwrap()
}

fn max(values: &std::collections::BTreeSet<TimeValue>) -> TimeValue {
    *values.iter().rev().next().unwrap()
}
