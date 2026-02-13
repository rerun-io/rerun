use std::collections::BTreeMap;

use re_chunk::{LatestAtQuery, TimeInt};
use re_entity_db::EntityDb;
use re_log_types::AbsoluteTimeRange;

use crate::blueprint_timeline;

/// Max number of undo points.
///
/// TODO(emilk): decide based on how much memory the blueprint uses instead.
const MAX_UNDOS: usize = 100;

/// We store the entire edit history of a blueprint in its store.
///
/// When undoing, we move back time, and redoing move it forward.
/// When editing, we first drop all data after the current time.
#[derive(Clone, Debug, Default)]
pub struct BlueprintUndoState {
    /// The current blueprint time, used for latest-at.
    ///
    /// Everything _after_ this time is in "redo-space",
    /// and will be dropped before new events are appended to the timeline.
    ///
    /// If `None`, use the max time of the blueprint timeline.
    current_time: Option<TimeInt>,

    /// The keys form a set of interesting times to undo/redo to.
    ///
    /// When the user drags a slider or similar, we get new events
    /// recorded on each frame. The user presumably wants to undo the whole
    /// slider drag, and not each increment of it.
    ///
    /// So we use a heuristic to estimate when such interactions start/stop,
    /// and add them to this set.
    ///
    /// The value is the frame number when the event happened,
    /// and is used in debug builds to detect bugs
    /// where we create undo-points every frame.
    inflection_points: BTreeMap<TimeInt, u64>,
}

impl re_byte_size::SizeBytes for BlueprintUndoState {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            current_time: _,
            inflection_points,
        } = self;
        inflection_points.heap_size_bytes()
    }
}

// We don't restore undo-state when closing the viewer.
// If you want to support this, make sure you replace the call to `cumulative_frame_nr` with something else,
// (because that resets to zero on restart) and also make sure you test it properly!
static_assertions::assert_not_impl_any!(BlueprintUndoState: serde::Serialize);

impl BlueprintUndoState {
    /// Default latest-at query
    #[inline]
    pub fn default_query() -> LatestAtQuery {
        LatestAtQuery::latest(blueprint_timeline())
    }

    /// How far back in time can we undo?
    pub fn oldest_undo_point(&self) -> Option<TimeInt> {
        self.inflection_points
            .first_key_value()
            .map(|(key, _)| *key)
    }

    pub fn blueprint_query(&self) -> LatestAtQuery {
        if let Some(time) = self.current_time {
            LatestAtQuery::new(blueprint_timeline(), time)
        } else {
            Self::default_query()
        }
    }

    /// If set, everything after this time is in "redo-space" (futurum).
    /// If `None`, there is no undo-buffer.
    pub fn redo_time(&self) -> Option<TimeInt> {
        self.current_time
    }

    pub fn set_redo_time(&mut self, time: TimeInt) {
        self.current_time = Some(time);
    }

    pub fn undo(&mut self, blueprint_db: &EntityDb) {
        let time = self
            .current_time
            .unwrap_or_else(|| max_blueprint_time(blueprint_db));

        if let Some((previous, _)) = self.inflection_points.range(..time).next_back() {
            re_log::trace!("Undo");
            self.current_time = Some(*previous);
        } else {
            // nothing to undo to
            re_log::debug!("Nothing to undo");
        }
    }

    pub fn redo(&mut self) {
        if let Some(time) = self.current_time {
            re_log::trace!("Redo");
            self.current_time = self
                .inflection_points
                .range(time.inc()..)
                .next()
                .map(|(key, _)| *key);
        } else {
            // If we have no time, we're at latest, and have nothing to redo
            re_log::debug!("Nothing to redo");
        }
    }

    pub fn redo_all(&mut self) {
        self.current_time = None;
    }

    /// After calling this, there is no way to redo what was once undone.
    pub fn clear_redo_buffer(&mut self, blueprint_db: &mut EntityDb) {
        re_tracing::profile_function!();

        if let Some(last_kept_event_time) = self.current_time.take() {
            let first_dropped_event_time =
                TimeInt::new_temporal(last_kept_event_time.as_i64().saturating_add(1));

            // Drop everything after the current timeline time
            let events = blueprint_db.drop_time_range(
                &blueprint_timeline(),
                AbsoluteTimeRange::new(first_dropped_event_time, re_chunk::TimeInt::MAX),
            );

            re_log::trace!("{} chunks affected when clearing redo buffer", events.len());
        }
    }

    // Call each frame
    pub fn update(&mut self, egui_ctx: &egui::Context, blueprint_db: &EntityDb) {
        re_tracing::profile_function!();

        if is_interacting(egui_ctx) {
            return; // Don't create undo points while we're still interacting.
        }

        // NOTE: we may be called several times in each frame (if we do multiple egui passes).
        let frame_nr = egui_ctx.cumulative_frame_nr();

        if let Some((_, last_frame_nr)) = self.inflection_points.last_key_value() {
            re_log::debug_assert!(
                *last_frame_nr <= frame_nr,
                "Frame counter is running backwards, from {last_frame_nr} to {frame_nr}!"
            );
        }

        // Nothing is happening - remember this as a time to undo to.
        let time = max_blueprint_time(blueprint_db);
        let inserted = self.inflection_points.insert(time, frame_nr).is_none();
        if inserted {
            re_log::trace!("Inserted new inflection point at {time:?}");
        }

        // TODO(emilk): we should _also_ look for long streaks of changes (changes every frame)
        // and disregard those, in case we miss something in `is_interacting`.
        // Note that this on its own isn't enough: if you drag a slider,
        // then you don't want an undo-point each time you pause the mouse - only on mouse-up!
        // So we still need a call to `is_interacting`, no matter what.
        // We must also make sure that this doesn't ignore actual bugs
        // (writing spurious data to the blueprint store each frame -
        // see https://github.com/rerun-io/rerun/issues/10304 and the comment below for more info).

        // Don't store too many undo-points:
        while let Some(first) = self
            .inflection_points
            .first_key_value()
            .map(|(key, _)| *key)
        {
            if MAX_UNDOS < self.inflection_points.len() {
                self.inflection_points.remove(&first);
            } else {
                break;
            }
        }

        if cfg!(debug_assertions) {
            // A bug we've seen before is that something ends up creating undo-points every frame.
            // This causes undo to effectively break, but it won't be obvious unless you try to undo.
            // So it is important that we catch this problem early.
            // See https://github.com/rerun-io/rerun/issues/10304 for more.

            // We use a simple approach here: if we're adding too many undo points
            // in a short amount of time, that's likely because of a bug.

            let n = 10;
            let mut latest_iter = self.inflection_points.iter().rev();
            let latest = latest_iter.next();
            let n_back = latest_iter.nth(n - 1);
            if let (Some((_, latest_frame_nr)), Some((_, n_back_frame_nr))) = (latest, n_back) {
                let num_frames: u64 = latest_frame_nr - n_back_frame_nr;
                if num_frames <= 2 * n as u64 {
                    // We've added `n` undo points in under 2*n frames. Very suspicious!
                    re_log::warn!(
                        "[DEBUG]: We added {n} undo points in {num_frames} frames. This likely means Undo is broken. Please investigate!"
                    );
                }
            }
        }
    }
}

fn max_blueprint_time(blueprint_db: &EntityDb) -> TimeInt {
    blueprint_db
        .time_range_for(&blueprint_timeline())
        .map(|range| range.max.as_i64())
        .map_or(TimeInt::ZERO, TimeInt::new_temporal)
}

fn is_interacting(egui_ctx: &egui::Context) -> bool {
    egui_ctx.input(|i| {
        let is_scrolling = i.smooth_scroll_delta != egui::Vec2::ZERO
                // TODO(RR-2730): If egui properly tracked when we're scrolling we wouldn't have to do this time check.
                || i.time_since_last_scroll() < 0.1;
        let is_zooming = i.zoom_delta_2d() != egui::Vec2::splat(1.0);
        i.pointer.any_down()
            || i.any_touches()
            || is_scrolling
            || !i.keys_down.is_empty()
            || is_zooming
    })
}
