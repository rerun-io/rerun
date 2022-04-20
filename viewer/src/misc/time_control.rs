use std::collections::BTreeMap;

use eframe::egui;
use egui::NumExt;
use log_types::*;

use crate::misc::TimePoints;

use super::time_axis::TimeRange;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub(crate) enum TimeSelectionType {
    // No time selection.
    None,
    // The selection is for looping the play marker.
    Loop,
    // The selection is for viewing a bunch of data at once, replacing the play marker.
    Filter,
}

impl Default for TimeSelectionType {
    fn default() -> Self {
        Self::None
    }
}

impl TimeSelectionType {
    pub fn color(&self, visuals: &egui::Visuals) -> egui::Color32 {
        use egui::Color32;
        match self {
            TimeSelectionType::None => Color32::GRAY,
            TimeSelectionType::Loop => Color32::from_rgb(50, 220, 140),
            TimeSelectionType::Filter => visuals.selection.bg_fill, // it is a form of selection, so let's be consistent
        }
    }
}

/// State per time source.
#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
struct TimeState {
    /// The current time (play marker).
    time: TimeValue,

    /// Selected time range, if any.
    #[serde(default)]
    selection: Option<TimeRange>,

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
        Self {
            time,
            selection: Default::default(),
            view: None,
        }
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
    looped: bool,
    speed: f32,

    #[serde(default)]
    pub selection_type: TimeSelectionType,
}

impl Default for TimeControl {
    fn default() -> Self {
        Self {
            time_source: Default::default(),
            states: Default::default(),
            playing: true,
            looped: true,
            speed: 1.0,
            selection_type: TimeSelectionType::default(),
        }
    }
}

impl TimeControl {
    pub fn time_source_selector_ui(
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

    pub fn selection_ui(&mut self, ui: &mut egui::Ui) {
        use egui::SelectableLabel;

        ui.label("Selection:");

        let has_selection = self
            .states
            .get(&self.time_source)
            .map_or(false, |state| state.selection.is_some());

        if ui
            .add(SelectableLabel::new(
                self.selection_type == TimeSelectionType::None || !has_selection,
                "None",
            ))
            .on_hover_text("Disable selection")
            .clicked()
        {
            self.selection_type = TimeSelectionType::None;
        }

        ui.scope(|ui| {
            ui.visuals_mut().selection.bg_fill = TimeSelectionType::Loop.color(ui.visuals());

            if ui
                .add_enabled(
                    has_selection,
                    SelectableLabel::new(self.selection_type == TimeSelectionType::Loop, "🔁"),
                )
                .on_hover_text("Loop in selection")
                .clicked()
            {
                self.selection_type = TimeSelectionType::Loop;
                self.looped = true;
            }
        });

        ui.scope(|ui| {
            ui.visuals_mut().selection.bg_fill = TimeSelectionType::Filter.color(ui.visuals());

            if ui
                .add_enabled(
                    has_selection,
                    SelectableLabel::new(self.selection_type == TimeSelectionType::Filter, "⬌"),
                )
                .on_hover_text("Show everything in selection")
                .clicked()
            {
                self.selection_type = TimeSelectionType::Filter;
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
            if self.playing {
                self.playing = false;
            } else {
                self.play(time_points);
            }
        }

        if ui
            .selectable_label(self.playing, "▶")
            .on_hover_text("Play. Toggle with SPACE")
            .clicked()
        {
            self.play(time_points);
        }
        if ui
            .selectable_label(!self.playing, "⏸")
            .on_hover_text("Pause. Toggle with SPACE")
            .clicked()
        {
            self.playing = false;
        }

        ui.scope(|ui| {
            ui.visuals_mut().selection.bg_fill = TimeSelectionType::Loop.color(ui.visuals());

            if ui
                .selectable_label(self.looped, "🔁")
                .on_hover_text("Loop playback")
                .clicked()
            {
                self.looped = !self.looped;

                if !self.looped && self.selection_type == TimeSelectionType::Loop {
                    self.selection_type = TimeSelectionType::None;
                }
            }
        });

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

        if !self.playing {
            return;
        }

        let full_range = if let Some(full_range) = time_points.0.get(&self.time_source).map(range) {
            full_range
        } else {
            return;
        };

        let state = self
            .states
            .entry(self.time_source.clone())
            .or_insert_with(|| TimeState::new(full_range.min));

        egui_ctx.request_repaint();

        let dt = egui_ctx.input().unstable_dt.at_most(0.05);

        if self.selection_type == TimeSelectionType::Filter {
            if let Some(time_selection) = state.selection {
                // Move selection

                let span = if let Some(span) = time_selection.span() {
                    span
                } else {
                    state.selection = None;
                    return;
                };

                let mut new_min = time_selection.min;

                if self.looped {
                    // max must be in the range:
                    new_min = new_min.max(full_range.min.add_offset_f64(-span));
                }

                match &mut new_min {
                    TimeValue::Sequence(seq) => {
                        *seq += 1; // TODO: apply speed here somehow?
                    }
                    TimeValue::Time(time) => {
                        *time += Duration::from_secs(dt * self.speed);
                    }
                }

                if new_min > full_range.max {
                    if self.looped {
                        // Put max just at start of loop:
                        new_min = full_range.min.add_offset_f64(-span);
                    } else {
                        new_min = full_range.max;
                        self.playing = false;
                    }
                }

                let new_max = new_min.add_offset_f64(span);
                state.selection = Some(TimeRange::new(new_min, new_max));

                return;
            }
        }

        // Normal time marker:

        let loop_range = if self.looped && self.selection_type == TimeSelectionType::Loop {
            state.selection.unwrap_or(full_range)
        } else {
            full_range
        };

        if self.looped {
            state.time = state.time.max(loop_range.min);
        }

        match &mut state.time {
            TimeValue::Sequence(seq) => {
                *seq += 1; // TODO: apply speed here somehow?
            }
            TimeValue::Time(time) => {
                *time += Duration::from_secs(dt * self.speed);
            }
        }

        if state.time > loop_range.max {
            if self.looped {
                state.time = loop_range.min;
            } else {
                state.time = loop_range.max;
                self.playing = false;
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

    pub fn is_playing(&self) -> bool {
        self.playing
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

    /// The current time. Note that this only makes sense if there is no time selection!
    pub fn time(&self) -> Option<TimeValue> {
        if self.selection_type == TimeSelectionType::Filter {
            return None; // no single time
        }

        self.states.get(&self.time_source).map(|state| state.time)
    }

    /// The current viewed/selected time.
    /// Returns a "point" range if we have no selection (normal play)
    pub fn time_range(&self) -> Option<TimeRange> {
        let state = self.states.get(&self.time_source)?;

        if self.selection_type == TimeSelectionType::Filter {
            state.selection
        } else {
            Some(TimeRange::point(state.time))
        }
    }

    /// Is the current time in the selection range (if any), or at the current time mark?
    pub fn is_time_selected(&self, time_source: &str, needle: TimeValue) -> bool {
        if time_source != self.time_source {
            return false;
        }

        if let Some(state) = self.states.get(&self.time_source) {
            if self.selection_type == TimeSelectionType::Filter {
                if let Some(range) = state.selection {
                    return range.contains(needle);
                }
            }

            state.time == needle
        } else {
            false
        }
    }

    pub fn set_source_and_time(&mut self, time_source: String, time: TimeValue) {
        self.time_source = time_source;
        self.set_time(time);
    }

    pub fn set_time(&mut self, time: TimeValue) {
        if self.selection_type == TimeSelectionType::Filter {
            self.selection_type = TimeSelectionType::None;
        }

        self.states
            .entry(self.time_source.clone())
            .or_insert_with(|| TimeState::new(time))
            .time = time;
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

    pub fn time_selection(&self) -> Option<TimeRange> {
        self.states.get(&self.time_source)?.selection
    }

    pub fn set_time_selection(&mut self, selection: TimeRange) {
        self.states
            .entry(self.time_source.clone())
            .or_insert_with(|| TimeState::new(selection.min))
            .selection = Some(selection);
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    /// Return the messages that should be visible at this time.
    ///
    /// This is either based on a time selection, or it is the latest message at the current time.
    pub fn selected_messages<'db>(&self, log_db: &'db crate::log_db::LogDb) -> Vec<&'db LogMsg> {
        crate::profile_function!();

        let state = if let Some(state) = self.states.get(&self.time_source) {
            state
        } else {
            return Default::default();
        };

        if let Some(range) = state.selection {
            if self.selection_type == TimeSelectionType::Filter {
                return log_db.messages_in_range(self.source(), range);
            }
        }

        log_db.latest_of_each_object(self.source(), state.time)
    }
}

fn min(values: &std::collections::BTreeSet<TimeValue>) -> TimeValue {
    *values.iter().next().unwrap()
}

fn max(values: &std::collections::BTreeSet<TimeValue>) -> TimeValue {
    *values.iter().rev().next().unwrap()
}

fn range(values: &std::collections::BTreeSet<TimeValue>) -> TimeRange {
    TimeRange::new(min(values), max(values))
}
