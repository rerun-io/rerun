use std::collections::{BTreeMap, BTreeSet};

use egui::NumExt as _;

use re_data_store::{log_db::TimePoints, TimeQuery};
use re_log_types::*;

use super::{TimeRange, TimeRangeF, TimeReal};

/// The time range we are currently zoomed in on.
#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct TimeView {
    /// Where start of the the range.
    pub min: TimeReal,

    /// How much time the full view covers.
    ///
    /// The unit is either nanoseconds or sequence numbers.
    ///
    /// If there is gaps in the data, the actual amount of viewed time might be less.
    pub time_spanned: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub(crate) enum TimeSelectionType {
    // The selection is for looping the play marker.
    Loop,
    // The selection is for viewing a bunch of data at once, replacing the play marker.
    Filter,
}

impl Default for TimeSelectionType {
    fn default() -> Self {
        Self::Loop
    }
}

impl TimeSelectionType {
    pub fn color(&self, visuals: &egui::Visuals) -> egui::Color32 {
        use egui::Color32;
        match self {
            TimeSelectionType::Loop => Color32::from_rgb(40, 200, 130),
            TimeSelectionType::Filter => visuals.selection.bg_fill, // it is a form of selection, so let's be consistent
        }
    }
}

/// State per time source.
#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
struct TimeState {
    /// The current time (play marker).
    time: TimeReal,

    /// Frames Per Second, when playing sequences (they are often video recordings).
    fps: f32,

    /// Selected time range, if any.
    #[serde(default)]
    selection: Option<TimeRangeF>,

    /// The time range we are currently zoomed in on.
    ///
    /// `None` means "everything", and is the default value.
    /// In this case, the view will expand while new data is added.
    /// Only when the user actually zooms or pans will this be set.
    #[serde(default)]
    view: Option<TimeView>,
}

impl TimeState {
    fn new(time: impl Into<TimeReal>) -> Self {
        Self {
            time: time.into(),
            fps: 30.0, // TODO(emilk): estimate based on data
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
    time_source: TimeSource,

    states: BTreeMap<TimeSource, TimeState>,

    playing: bool,
    looped: bool,
    speed: f32,

    #[serde(default)]
    pub selection_active: bool,

    #[serde(default)]
    pub selection_type: TimeSelectionType,
}

impl Default for TimeControl {
    fn default() -> Self {
        Self {
            time_source: Default::default(),
            states: Default::default(),
            playing: true,
            looped: false,
            speed: 1.0,
            selection_active: false,
            selection_type: TimeSelectionType::default(),
        }
    }
}

impl TimeControl {
    /// None when not active
    pub fn active_selection_type(&self) -> Option<TimeSelectionType> {
        self.selection_active.then_some(self.selection_type)
    }

    /// None when not active
    pub fn set_active_selection_type(&mut self, typ: Option<TimeSelectionType>) {
        match typ {
            None => {
                self.selection_active = false;
            }
            Some(typ) => {
                self.selection_active = true;
                self.selection_type = typ;
                if typ == TimeSelectionType::Loop {
                    self.looped = true;
                }
            }
        }
    }

    /// Is there a "filtering" selection, i.e. selecting a section of the time line
    pub fn is_time_filter_active(&self) -> bool {
        self.selection_active && self.selection_type == TimeSelectionType::Filter
    }

    pub fn has_selection(&self) -> bool {
        self.states
            .get(&self.time_source)
            .map_or(false, |state| state.selection.is_some())
    }

    /// Update the current time
    pub fn move_time(&mut self, egui_ctx: &egui::Context, time_points: &TimePoints) {
        self.select_a_valid_time_source(time_points);

        if !self.playing {
            return;
        }

        let full_range = if let Some(full_range) = self.full_range(time_points) {
            full_range
        } else {
            return;
        };

        let active_selection_type = self.active_selection_type();

        let state = self
            .states
            .entry(self.time_source)
            .or_insert_with(|| TimeState::new(full_range.min));

        let loop_range = if self.looped && active_selection_type == Some(TimeSelectionType::Loop) {
            state.selection.unwrap_or_else(|| full_range.into())
        } else {
            full_range.into()
        };

        let dt = egui_ctx.input().stable_dt.at_most(0.1) * self.speed;

        // ----
        // Are we moving a selection or a single marker?

        if active_selection_type == Some(TimeSelectionType::Filter) {
            if let Some(time_selection) = state.selection {
                // Move filter selection

                let length = time_selection.length();

                let mut new_min = time_selection.min;

                if self.looped {
                    // max must be in the range:
                    new_min = new_min.max(loop_range.min - length);
                }

                if time_selection.max >= loop_range.max && !self.looped {
                    // Don't pause or rewind, just stop moving time forward
                    // until we receive more data!
                    // This is important for "live view".
                    return;
                }

                match self.time_source.typ() {
                    TimeType::Sequence => {
                        new_min += TimeReal::from(state.fps * dt);
                    }
                    TimeType::Time => new_min += TimeReal::from(Duration::from_secs(dt)),
                }
                egui_ctx.request_repaint(); // keep playing next frame

                if new_min > loop_range.max && self.looped {
                    // Put max just at start of loop:
                    new_min = loop_range.min - length;
                }

                let new_max = new_min + length;
                state.selection = Some(TimeRangeF::new(new_min, new_max));

                return;
            }
        }

        // Normal time marker:

        if self.looped {
            state.time = state.time.max(loop_range.min);
        }

        if state.time >= loop_range.max && !self.looped {
            // Don't pause or rewind, just stop moving time forward
            // until we receive more data!
            // This is important for "live view".
            return;
        }

        match self.time_source.typ() {
            TimeType::Sequence => {
                state.time += TimeReal::from(state.fps * dt);
            }
            TimeType::Time => state.time += TimeReal::from(Duration::from_secs(dt)),
        }
        egui_ctx.request_repaint(); // keep playing next frame

        if state.time > loop_range.max && self.looped {
            state.time = loop_range.min;
        }
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn play(&mut self, time_points: &TimePoints) {
        if self.playing {
            return;
        }

        // Start from beginning if we are at the end:
        if let Some(axis) = time_points.0.get(&self.time_source) {
            if let Some(state) = self.states.get_mut(&self.time_source) {
                if state.time >= max(axis) {
                    state.time = min(axis).into();
                }
            } else {
                self.states
                    .insert(self.time_source, TimeState::new(min(axis)));
            }
        }
        self.playing = true;
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    /// playback speed
    pub fn speed(&self) -> f32 {
        self.speed
    }

    /// playback speed
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    /// playback fps
    pub fn fps(&self) -> Option<f32> {
        self.states.get(&self.time_source).map(|state| state.fps)
    }

    /// playback fps
    pub fn set_fps(&mut self, fps: f32) {
        if let Some(state) = self.states.get_mut(&self.time_source) {
            state.fps = fps;
        }
    }

    /// looped playback enabled?
    pub fn looped(&self) -> bool {
        self.looped
    }

    /// looped playback enabled?
    pub fn set_looped(&mut self, looped: bool) {
        self.looped = looped;
    }

    /// Make sure the selected time source is a valid one
    pub fn select_a_valid_time_source(&mut self, time_points: &TimePoints) {
        for source in time_points.0.keys() {
            if &self.time_source == source {
                return; // it's valid
            }
        }
        if let Some(source) = time_points.0.keys().next() {
            self.time_source = *source;
        } else {
            self.time_source = Default::default();
        }
    }

    /// The currently selected time source
    pub fn source(&self) -> &TimeSource {
        &self.time_source
    }

    /// The time type of the currently selected time source
    pub fn time_type(&self) -> TimeType {
        self.time_source.typ()
    }

    pub fn set_source(&mut self, time_source: TimeSource) {
        self.time_source = time_source;
    }

    /// The current time. Note that this only makes sense if there is no time selection!
    pub fn time(&self) -> Option<TimeReal> {
        if self.is_time_filter_active() {
            return None; // no single time
        }

        self.states.get(&self.time_source).map(|state| state.time)
    }

    /// The current filtered time.
    /// Returns a "point" range if we have no selection (normal play)
    pub fn time_range(&self) -> Option<TimeRangeF> {
        let state = self.states.get(&self.time_source)?;

        if self.is_time_filter_active() {
            state.selection
        } else {
            Some(TimeRangeF::point(state.time))
        }
    }

    /// If the time filter is active, what range does it cover?
    pub fn time_filter_range(&self) -> Option<TimeRangeF> {
        if self.is_time_filter_active() {
            self.states.get(&self.time_source)?.selection
        } else {
            None
        }
    }

    /// The current loop range, iff looping is turned on
    pub fn loop_range(&self) -> Option<TimeRangeF> {
        if self.selection_active && self.selection_type == TimeSelectionType::Loop {
            self.states.get(&self.time_source)?.selection
        } else {
            None
        }
    }

    /// The full range of times for the current time source
    pub fn full_range(&self, time_points: &TimePoints) -> Option<TimeRange> {
        time_points.0.get(&self.time_source).map(range)
    }

    /// Is the current time in the selection range (if any), or at the current time mark?
    pub fn is_time_selected(&self, time_source: &TimeSource, needle: TimeReal) -> bool {
        if time_source != &self.time_source {
            return false;
        }

        if let Some(state) = self.states.get(&self.time_source) {
            if self.is_time_filter_active() {
                if let Some(range) = state.selection {
                    return range.contains(needle);
                }
            }

            state.time == needle
        } else {
            false
        }
    }

    pub fn set_source_and_time(&mut self, time_source: TimeSource, time: impl Into<TimeReal>) {
        self.time_source = time_source;
        self.set_time(time);
    }

    pub fn set_time(&mut self, time: impl Into<TimeReal>) {
        if self.is_time_filter_active() {
            self.selection_active = false;
        }

        let time = time.into();

        self.states
            .entry(self.time_source)
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
            .entry(self.time_source)
            .or_insert_with(|| TimeState::new(view.min))
            .view = Some(view);
    }

    /// The range of time we are currently zoomed in on.
    pub fn reset_time_view(&mut self) {
        if let Some(state) = self.states.get_mut(&self.time_source) {
            state.view = None;
        }
    }

    pub fn time_selection(&self) -> Option<TimeRangeF> {
        self.states.get(&self.time_source)?.selection
    }

    pub fn set_time_selection(&mut self, selection: TimeRangeF) {
        self.states
            .entry(self.time_source)
            .or_insert_with(|| TimeState::new(selection.min))
            .selection = Some(selection);
    }

    /// Return the data that should be visible at this time.
    ///
    /// This is either based on a time selection, or it is the latest message at the current time.
    pub fn selected_objects<'db>(
        &self,
        log_db: &'db re_data_store::log_db::LogDb,
    ) -> re_data_store::Objects<'db> {
        crate::profile_function!();

        let mut objects = re_data_store::Objects::default();
        if let Some(time_query) = self.time_query() {
            if let Some(store) = log_db.data_store.get(&self.time_source) {
                objects.query(store, &time_query, &log_db.obj_types);
            }
        }

        objects
    }

    pub fn time_query(&self) -> Option<TimeQuery<i64>> {
        if self.is_time_filter_active() {
            if let Some(state) = self.states.get(&self.time_source) {
                if let Some(range) = state.selection {
                    return Some(TimeQuery::Range(
                        range.min.ceil().as_i64()..=range.max.floor().as_i64(),
                    ));
                }
            }
        }
        Some(TimeQuery::LatestAt(self.time()?.floor().as_i64()))
    }
}

fn min(values: &BTreeSet<TimeInt>) -> TimeInt {
    *values.iter().next().unwrap()
}

fn max(values: &BTreeSet<TimeInt>) -> TimeInt {
    *values.iter().rev().next().unwrap()
}

fn range(values: &BTreeSet<TimeInt>) -> TimeRange {
    TimeRange::new(min(values), max(values))
}
