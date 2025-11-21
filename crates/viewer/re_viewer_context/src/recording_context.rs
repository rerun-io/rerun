use re_chunk::TimelineName;
use re_log_types::{AbsoluteTimeRange, TimeInt, TimeType, Timeline};

use crate::ItemCollection;

#[derive(Default)]
pub struct RecordingContext {
    pub current_selection: Timeline,
    pub current_time_selection: AbsoluteTimeRange,
    pub time_point: AbsoluteTimeRange,
    pub selection: ItemCollection,
}

impl RecordingContext {
    pub fn new(
        current_selection: Timeline,
        current_time_selection: AbsoluteTimeRange,
        time_point: AbsoluteTimeRange,
        selection: ItemCollection,
    ) -> Self {
        Self {
            current_selection,
            current_time_selection,
            time_point,
            selection,
        }
    }

    /// Set the current timeline selection.
    pub fn set_current_selection(&mut self, timeline: Timeline) {
        self.current_selection = timeline;
    }

    /// Set the current timeline selection by name and type.
    pub fn set_current_selection_by_name(&mut self, name: impl Into<TimelineName>, typ: TimeType) {
        self.current_selection = Timeline::new(name, typ);
    }

    /// Update the current timeline selection's name.
    pub fn set_current_selection_name(&mut self, name: impl Into<TimelineName>) {
        self.current_selection = Timeline::new(name, self.current_selection.typ());
    }

    /// Update the current timeline selection's type.
    pub fn set_current_selection_type(&mut self, typ: TimeType) {
        self.current_selection = Timeline::new(*self.current_selection.name(), typ);
    }

    /// Set the current time selection range.
    pub fn set_current_time_selection(&mut self, time_range: AbsoluteTimeRange) {
        self.current_time_selection = time_range;
    }

    /// Set the current time selection range from min and max values.
    pub fn set_current_time_selection_range(
        &mut self,
        min: impl TryInto<TimeInt>,
        max: impl TryInto<TimeInt>,
    ) {
        self.current_time_selection = AbsoluteTimeRange::new(min, max);
    }

    /// Update the minimum time of the current time selection.
    pub fn set_current_time_selection_min(&mut self, min: impl TryInto<TimeInt>) {
        self.current_time_selection.set_min(min);
    }

    /// Update the maximum time of the current time selection.
    pub fn set_current_time_selection_max(&mut self, max: impl TryInto<TimeInt>) {
        self.current_time_selection.set_max(max);
    }

    /// Set the time point range.
    pub fn set_time_point(&mut self, time_range: AbsoluteTimeRange) {
        self.time_point = time_range;
    }

    /// Set the time point range from min and max values.
    pub fn set_time_point_range(&mut self, min: impl TryInto<TimeInt>, max: impl TryInto<TimeInt>) {
        self.time_point = AbsoluteTimeRange::new(min, max);
    }

    /// Set the time point to a single point in time.
    pub fn set_time_point_to(&mut self, time: impl TryInto<TimeInt>) {
        self.time_point = AbsoluteTimeRange::point(time);
    }

    /// Update the minimum time of the time point.
    pub fn set_time_point_min(&mut self, min: impl TryInto<TimeInt>) {
        self.time_point.set_min(min);
    }

    /// Update the maximum time of the time point.
    pub fn set_time_point_max(&mut self, max: impl TryInto<TimeInt>) {
        self.time_point.set_max(max);
    }

    /// Update all fields at once.
    pub fn update_all(
        &mut self,
        current_selection: Option<Timeline>,
        current_time_selection: Option<AbsoluteTimeRange>,
        time_point: Option<AbsoluteTimeRange>,
    ) {
        if let Some(selection) = current_selection {
            self.current_selection = selection;
        }
        if let Some(time_selection) = current_time_selection {
            self.current_time_selection = time_selection;
        }
        if let Some(point) = time_point {
            self.time_point = point;
        }
    }
}
