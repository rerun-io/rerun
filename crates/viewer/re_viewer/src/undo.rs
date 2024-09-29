use std::collections::BTreeSet;

use re_chunk::{LatestAtQuery, TimeInt};
use re_entity_db::EntityDb;
use re_log_types::ResolvedTimeRange;
use re_viewer_context::blueprint_timeline;

/// We store the entire edit history of a blueprint in its store.
///
/// When undoing, we move back time, and redoing move it forward.
/// When editing, we first drop all data after the current time.
#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct BlueprintUndoState {
    /// The current blueprint time, used for latest-at.
    ///
    /// Everything _after_ this time is in "redo-space",
    /// and will be dropped before new events are appended to the timeline.
    ///
    /// If `None`, use the max time of the blueprint timeline.
    current_time: Option<TimeInt>,

    /// Interesting times to undo/redo to.
    ///
    /// When the user drags a slider or similar, we get new events
    /// recorded on each frame. The user presumably wants to undo the whole
    /// slider drag, and not each increment of it.
    ///
    /// So we use a heuristic to estimate when such interactions start/stop,
    /// and add them to this set.
    inflection_points: BTreeSet<TimeInt>,
}

impl BlueprintUndoState {
    /// Default latest-at query
    #[inline]
    pub fn default_query() -> LatestAtQuery {
        LatestAtQuery::latest(blueprint_timeline())
    }

    pub fn blueprint_query(&self) -> LatestAtQuery {
        if let Some(time) = self.current_time {
            LatestAtQuery::new(blueprint_timeline(), time)
        } else {
            Self::default_query()
        }
    }

    pub fn undo(&mut self, blueprint_db: &EntityDb) {
        let time = self
            .current_time
            .unwrap_or_else(|| max_blueprint_time(blueprint_db));

        if let Some(previous) = self.inflection_points.range(..time).next_back().copied() {
            self.current_time = Some(previous);
        } else {
            // nothing to undo to
        }
    }

    pub fn redo(&mut self, _blueprint_db: &EntityDb) {
        if let Some(time) = self.current_time {
            self.current_time = self.inflection_points.range(time.inc()..).next().copied();
        } else {
            // If we have no time, we're at latest, and have nothing to redo
        }
    }

    pub fn clear_redo(&mut self, blueprint_db: &mut EntityDb) {
        re_tracing::profile_function!();

        if let Some(last_kept_event_time) = self.current_time.take() {
            let first_dropped_event_time =
                TimeInt::new_temporal(last_kept_event_time.as_i64().saturating_add(1));

            // Drop everything before the current timeline time
            blueprint_db.drop_time_range(
                &blueprint_timeline(),
                ResolvedTimeRange::new(first_dropped_event_time, re_chunk::TimeInt::MAX),
            );
        }
    }

    // Call each frame
    pub fn update(&mut self, egui_ctx: &egui::Context, blueprint_db: &EntityDb) {
        if is_interacting(egui_ctx) {
            return;
        }

        // Nothing is happening - remember this as a time to undo to.
        let time = max_blueprint_time(blueprint_db);
        let inserted = self.inflection_points.insert(time);
        if inserted {
            re_log::trace!("Inserted new inflection point at {time:?}");
        }

        // TODO(emilk): we should _also_ look for long streaks of changes (changes every frame)
        // and disregard those, in case we miss something in `is_interacting`.
        // Note that this on its own won't enough though - if you drag a slider,
        // then you don't want an undo-point each time you pause the mouse - only on mouse-up!
    }
}

fn max_blueprint_time(blueprint_db: &EntityDb) -> TimeInt {
    blueprint_db
        .time_histogram(&blueprint_timeline())
        .and_then(|times| times.max_key())
        .map_or(TimeInt::ZERO, TimeInt::new_temporal)
}

fn is_interacting(egui_ctx: &egui::Context) -> bool {
    egui_ctx.input(|i| {
        let is_scrolling = i.smooth_scroll_delta != egui::Vec2::ZERO;
        let is_zooming = i.zoom_delta_2d() != egui::Vec2::splat(1.0);
        i.pointer.any_down()
            || i.any_touches()
            || is_scrolling
            || !i.keys_down.is_empty()
            || is_zooming
    })
}
