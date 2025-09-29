use std::collections::HashMap;

use re_entity_db::EntityDb;
use re_log_types::{StoreId, TimeReal, Timeline};
use re_analytics::event::{PlaybackSessionType, PlaybackStopReason};

use crate::viewer_analytics;

/// Tracks playback sessions to generate analytics events
#[derive(Default)]
pub struct PlaybackSessionTracker {
    /// Active sessions keyed by recording id
    active_sessions: HashMap<StoreId, ActiveSession>,
}

struct ActiveSession {
    start_time: web_time::Instant,
    timeline: Timeline,
    session_type: PlaybackSessionType,

    /// Track positions visited during the session
    min_time_visited: TimeReal,
    max_time_visited: TimeReal,

    /// Sum of absolute distances traveled
    total_distance_traveled: f64,

    /// Last known time position
    last_time_position: TimeReal,
}

impl PlaybackSessionTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start or continue a session when playback begins or user interacts with timeline
    pub fn on_playback_interaction(
        &mut self,
        recording_id: &StoreId,
        timeline: Timeline,
        current_time: TimeReal,
        interaction_type: PlaybackSessionType,
    ) {
        let session = self.active_sessions.entry(recording_id.clone()).or_insert_with(|| {
            ActiveSession {
                start_time: web_time::Instant::now(),
                timeline,
                session_type: interaction_type,
                min_time_visited: current_time,
                max_time_visited: current_time,
                total_distance_traveled: 0.0,
                last_time_position: current_time,
            }
        });

        // Update session type if it changes (e.g., from playback to scrubbing)
        if session.session_type != interaction_type {
            session.session_type = match (&session.session_type, &interaction_type) {
                (PlaybackSessionType::Playback, PlaybackSessionType::Scrubbing) |
                (PlaybackSessionType::Scrubbing, PlaybackSessionType::Playback) => PlaybackSessionType::Mixed,
                _ => interaction_type,
            };
        }

        // Update time tracking
        let time_distance = (current_time.as_f64() - session.last_time_position.as_f64()).abs();
        session.total_distance_traveled += time_distance;

        session.min_time_visited = session.min_time_visited.min(current_time);
        session.max_time_visited = session.max_time_visited.max(current_time);
        session.last_time_position = current_time;
    }

    /// End a session and emit analytics event
    pub fn end_session(
        &mut self,
        recording_id: &StoreId,
        _entity_db: &EntityDb,
        reason: PlaybackStopReason,
        build_info: re_build_info::BuildInfo,
    ) {
        if let Some(session) = self.active_sessions.remove(recording_id) {
            let wall_clock_seconds = session.start_time.elapsed().as_secs_f64();

            let time_unit = match session.timeline.typ() {
                re_log_types::TimeType::Sequence => "frames".to_string(),
                _ => "seconds".to_string(),
            };

            // Convert timeline units to appropriate measurements
            let (total_time_traveled, covered_time_distance) = match session.timeline.typ() {
                re_log_types::TimeType::Sequence => {
                    // For sequences, distance is in frame counts
                    (session.total_distance_traveled,
                     (session.max_time_visited.as_f64() - session.min_time_visited.as_f64()))
                }
                _ => {
                    // For time timelines, convert nanoseconds to seconds
                    (session.total_distance_traveled / 1e9,
                     (session.max_time_visited.as_f64() - session.min_time_visited.as_f64()) / 1e9)
                }
            };

            let event = viewer_analytics::event::playback_session(
                build_info,
                session.timeline.name().to_string(),
                wall_clock_seconds,
                session.session_type,
                total_time_traveled,
                covered_time_distance.max(0.0), // Ensure non-negative
                time_unit,
                reason,
                recording_id,
            );

            #[cfg(feature = "analytics")]
            if let Some(analytics) = re_analytics::Analytics::global_or_init() {
                analytics.record(event);
            }
        }
    }

    /// End all active sessions (e.g., when app is shutting down)
    pub fn end_all_sessions(&mut self, reason: PlaybackStopReason, build_info: re_build_info::BuildInfo) {
        let session_ids: Vec<StoreId> = self.active_sessions.keys().cloned().collect();
        for store_id in session_ids {
            // We don't have EntityDb here, but we still want to end sessions
            if let Some(session) = self.active_sessions.remove(&store_id) {
                let wall_clock_seconds = session.start_time.elapsed().as_secs_f64();

                let time_unit = match session.timeline.typ() {
                    re_log_types::TimeType::Sequence => "frames".to_string(),
                    _ => "seconds".to_string(),
                };

                let (total_time_traveled, covered_time_distance) = match session.timeline.typ() {
                    re_log_types::TimeType::Sequence => {
                        (session.total_distance_traveled,
                         (session.max_time_visited.as_f64() - session.min_time_visited.as_f64()))
                    }
                    _ => {
                        (session.total_distance_traveled / 1e9,
                         (session.max_time_visited.as_f64() - session.min_time_visited.as_f64()) / 1e9)
                    }
                };

                let event = viewer_analytics::event::playback_session(
                    build_info.clone(),
                    session.timeline.name().to_string(),
                    wall_clock_seconds,
                    session.session_type,
                    total_time_traveled,
                    covered_time_distance.max(0.0),
                    time_unit,
                    reason.clone(),
                    &store_id,
                );

                #[cfg(feature = "analytics")]
                if let Some(analytics) = re_analytics::Analytics::global_or_init() {
                    analytics.record(event);
                }
            }
        }
    }
}