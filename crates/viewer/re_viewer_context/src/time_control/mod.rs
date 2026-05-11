mod blueprint_ext;
mod command;
mod db;

use std::collections::BTreeMap;

use re_chunk::TimelineName;
use re_log_types::{
    AbsoluteTimeRange, AbsoluteTimeRangeF, Duration, TimeCell, TimeInt, TimeReal, TimeType,
    Timeline,
};
use re_sdk_types::blueprint::components::{LoopMode, PlayState};

use crate::NeedsRepaint;
use crate::blueprint_helpers::BlueprintContext;

use blueprint_ext::TimeBlueprintExt as _;

pub use blueprint_ext::{TIME_PANEL_PATH, time_panel_blueprint_entity_path};
pub use command::{MoveDirection, MoveSpeed, TimeControlCommand};
pub use db::{PreviewRecordingsDb, TimeControlDb};

/// What [`TimeControl`] should do when data is buffering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferBehavior {
    /// Advance time even while data is buffering. The visualizer is responsible
    /// for rendering whatever data it has.
    Play,

    /// Pause until the first frame after pressing play arrives, then transition to
    /// [`Self::Play`]. Used by the main viewer for playback, so the cursor doesn't
    /// start moving before any data has loaded, but later stalls don't stutter
    /// playback.
    WaitForDataThenPlay,

    /// Pause every time data is unavailable, for the entire duration of playback.
    /// Used by previews.
    AlwaysBuffer,
}

impl BufferBehavior {
    /// Should we hold the time cursor in place when [`TimeControlUpdateParams::is_buffering`] is set?
    fn pauses_on_buffer(self) -> bool {
        match self {
            Self::Play => false,
            Self::WaitForDataThenPlay | Self::AlwaysBuffer => true,
        }
    }
}

/// The time range we are currently zoomed in on.
#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct TimeView {
    /// Where start of the range.
    pub min: TimeReal,

    /// How much time the full view covers.
    ///
    /// The unit is either nanoseconds or sequence numbers.
    ///
    /// If there is gaps in the data, the actual amount of viewed time might be less.
    pub time_spanned: f64,
}

impl From<AbsoluteTimeRange> for TimeView {
    fn from(value: AbsoluteTimeRange) -> Self {
        Self {
            min: value.min().into(),
            time_spanned: value.abs_length() as f64,
        }
    }
}

/// State per timeline.
#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize, PartialEq)]
struct TimeState {
    /// The current time (play marker).
    time: TimeReal,

    /// The last time this timeline was paused at.
    ///
    /// Used for the web url.
    #[serde(skip)]
    last_paused_time: Option<TimeReal>,

    /// Frames per second, when playing sequences (they are often video recordings).
    fps: f32,

    /// Selected time range, if any.
    #[serde(default)]
    time_selection: Option<AbsoluteTimeRangeF>,

    /// The time range we are currently zoomed in on.
    ///
    /// `None` means "everything", and is the default value.
    /// In this case, the view will expand while new data is added.
    /// Only when the user actually zooms or pans will this be set.
    #[serde(default)]
    view: Option<TimeView>,
}

impl re_byte_size::SizeBytes for TimeState {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}

impl TimeState {
    fn new(time: impl Into<TimeReal>) -> Self {
        Self {
            time: time.into(),
            last_paused_time: None,
            fps: 30.0, // TODO(emilk): estimate based on data
            time_selection: Default::default(),
            view: None,
        }
    }
}

/// Which timeline is currently active in the time panel.
///
/// The active timeline can be in one of three states:
/// - Automatically chosen based on heuristics (e.g. the timeline with most data),
/// - Explicitly selected by the user,
/// - Or "pending": requested by name (via blueprint or user action) but not yet
///   present in the entity database. A pending timeline is promoted to `UserEdited`
///   once data containing that timeline arrives (see [`TimeControl::select_valid_timeline`]).
// TODO(andreas): This should be a blueprint property and follow the usual rules of how we determine fallbacks.
#[derive(serde::Deserialize, serde::Serialize, Clone, PartialEq, Debug)]
enum ActiveTimeline {
    /// Automatically selected based on heuristics. Re-evaluated every frame.
    Auto(Timeline),

    /// Explicitly selected by the user or resolved from blueprint.
    UserEdited(Timeline),

    /// A timeline was requested by name but hasn't been seen in the data yet.
    ///
    /// This happens when the blueprint or a [`TimeControlCommand::SetActiveTimeline`] references
    /// a timeline that doesn't exist in the current [`re_entity_db::EntityDb`]. We store only the name
    /// and wait for matching data to arrive, at which point this becomes `UserEdited`.
    Pending(TimelineName),
}

impl ActiveTimeline {
    /// The name of the active timeline, regardless of its state.
    pub fn name(&self) -> &TimelineName {
        match self {
            Self::Auto(timeline) | Self::UserEdited(timeline) => timeline.name(),
            Self::Pending(timeline_name) => timeline_name,
        }
    }

    /// The full [`Timeline`], if available.
    ///
    /// Returns `None` for [`Self::Pending`] since the timeline hasn't been
    /// resolved against the entity database yet.
    pub fn timeline(&self) -> Option<&Timeline> {
        match self {
            Self::Auto(timeline) | Self::UserEdited(timeline) => Some(timeline),
            Self::Pending(_) => None,
        }
    }
}

/// Controls the global view and progress of the time.
///
/// Modifications to this can be done via sending [`TimeControlCommand`]s
/// which are handled at the end of frames.
///
/// The commands write both to this struct and to blueprints when
/// applicable.
#[derive(Clone, PartialEq)]
pub struct TimeControl {
    /// Name of the timeline (e.g. `log_time`).
    timeline: ActiveTimeline,

    states: BTreeMap<TimelineName, TimeState>,

    /// If true, we are either in [`PlayState::Playing`] or [`PlayState::Following`].
    playing: bool,

    /// If true, we are in "follow" mode (see [`PlayState::Following`]).
    /// Ignored when [`Self::playing`] is `false`.
    following: bool,

    /// What this time control should do when data is buffering.
    ///
    /// See [`BufferBehavior`] for the variants.
    buffer_behavior: BufferBehavior,

    speed: f32,

    loop_mode: LoopMode,

    /// Range with special highlight.
    ///
    /// This is used during UI interactions. E.g. to show visual history range that's highlighted.
    pub highlighted_range: Option<AbsoluteTimeRange>,
}

impl Default for TimeControl {
    fn default() -> Self {
        Self {
            timeline: ActiveTimeline::Auto(Timeline::pick_best_timeline([], |_| 0)),
            states: Default::default(),
            playing: true,
            following: true,
            buffer_behavior: BufferBehavior::WaitForDataThenPlay,
            speed: 1.0,
            loop_mode: LoopMode::Off,
            highlighted_range: None,
        }
    }
}

impl re_byte_size::SizeBytes for TimeControl {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            timeline: _,
            states,
            playing: _,
            following: _,
            buffer_behavior: _,
            speed: _,
            loop_mode: _,
            highlighted_range: _,
        } = self;
        states.heap_size_bytes()
    }
}

/// Parameters for [`TimeControl::update`].
pub struct TimeControlUpdateParams {
    /// The time step in seconds.
    pub stable_dt: f32,

    /// Is more data expected to arrive (e.g. still connected to a data source)?
    ///
    /// Set to true e.g. when viewing live data,
    /// or we're still downloading a recording.
    pub more_data_is_streaming_in: bool,

    /// True if we're waiting for chunks to be downloaded,
    /// and they are expected to come (eventually).
    pub is_buffering: bool,

    /// Should we diff state changes to trigger callbacks?
    pub should_diff_state: bool,
}

#[must_use]
pub struct TimeControlResponse {
    pub needs_repaint: NeedsRepaint,

    /// Set if play state changed.
    ///
    /// * `Some(true)` if playing changed to `true`
    /// * `Some(false)` if playing changed to `false`
    /// * `None` if playing did not change
    pub playing_change: Option<bool>,

    /// Set if timeline changed.
    ///
    /// Contains the timeline name and the current time.
    pub timeline_change: Option<(Timeline, TimeReal)>,

    /// Set if the time changed.
    pub time_change: Option<TimeReal>,
}

impl TimeControlResponse {
    fn no_repaint() -> Self {
        Self::new(NeedsRepaint::No)
    }

    fn new(needs_repaint: NeedsRepaint) -> Self {
        Self {
            needs_repaint,
            playing_change: None,
            timeline_change: None,
            time_change: None,
        }
    }
}

impl TimeControl {
    /// Create a time control that plays in a loop, not backed by any blueprint.
    ///
    /// This will also always wait for data while buffering.
    pub fn preview_time_control() -> Self {
        Self {
            playing: true,
            following: false,
            loop_mode: LoopMode::All,
            buffer_behavior: BufferBehavior::AlwaysBuffer,
            ..Self::default()
        }
    }

    /// Hold the next playback advance until we stop buffering.
    ///
    /// Called when state changes mean we should re-pause for data, e.g. pressing
    /// play or jumping the cursor.
    fn start_buffering(&mut self) {
        if self.buffer_behavior != BufferBehavior::AlwaysBuffer {
            self.buffer_behavior = BufferBehavior::WaitForDataThenPlay;
        }
    }

    pub fn from_blueprint(blueprint_ctx: &impl BlueprintContext) -> Self {
        let mut this = Self::default();

        this.update_from_blueprint(blueprint_ctx, None);

        this
    }

    /// Read from the time panel blueprint and update the state from that.
    ///
    /// If `db` is some this will also make sure we are on a valid timeline.
    pub fn update_from_blueprint(
        &mut self,
        blueprint_ctx: &impl BlueprintContext,
        db: Option<&dyn TimeControlDb>,
    ) {
        if let Some(timeline) = blueprint_ctx.timeline() {
            if matches!(self.timeline, ActiveTimeline::Auto(_))
                || timeline.as_str() != self.timeline_name().as_str()
            {
                self.timeline = ActiveTimeline::Pending(timeline);
            }
        } else if let Some(timeline) = self.timeline() {
            self.timeline = ActiveTimeline::Auto(*timeline);
        }

        let old_timeline = *self.timeline_name();
        // Make sure we are on a valid timeline.
        if let Some(db) = db {
            self.select_valid_timeline(db);
        }

        if let Some(new_play_state) = blueprint_ctx.play_state()
            && new_play_state != self.play_state()
        {
            self.set_play_state(db, new_play_state, Some(blueprint_ctx));
        }

        if let Some(new_loop_mode) = blueprint_ctx.loop_mode() {
            self.loop_mode = new_loop_mode;

            if self.loop_mode != LoopMode::Off {
                if self.play_state() == PlayState::Following {
                    self.set_play_state(db, PlayState::Playing, Some(blueprint_ctx));
                }

                // It makes no sense with looping and follow.
                self.following = false;
            }
        }

        if let Some(playback_speed) = blueprint_ctx.playback_speed() {
            self.speed = playback_speed as f32;
        }

        let play_state = self.play_state();

        // Update the last paused time if we are paused.
        let timeline = *self.timeline_name();
        if let Some(state) = self.states.get_mut(&timeline) {
            if let Some(fps) = blueprint_ctx.fps() {
                state.fps = fps as f32;
            }

            let bp_loop_section = blueprint_ctx.time_selection();
            // If we've switched timeline, use the new timeline's cached time selection.
            if old_timeline == timeline {
                state.time_selection = bp_loop_section.map(|r| r.into());
            } else {
                match state.time_selection {
                    Some(selection) => blueprint_ctx.set_time_selection(selection.to_int()),
                    None => {
                        blueprint_ctx.clear_time_selection();
                    }
                }
            }

            match play_state {
                PlayState::Paused => {
                    state.last_paused_time = Some(state.time);
                }
                PlayState::Playing | PlayState::Following => {}
            }
        }
    }

    /// Sets the current time.
    ///
    /// This will NOT update the blueprint!
    pub fn set_time_ad_hoc(&mut self, time: TimeReal) {
        self.set_time_cursor_ad_hoc(*self.timeline_name(), time);
    }

    /// Sets the current time.
    ///
    /// This will NOT update the blueprint!
    pub fn set_time_cursor_ad_hoc(&mut self, timeline: TimelineName, time: TimeReal) {
        self.states
            .entry(timeline)
            .or_insert_with(|| TimeState::new(time))
            .time = time;
    }

    /// Create [`TimeControlCommand`]s to move the time forward (if playing), and perhaps pause if
    /// we've reached the end.
    pub fn update(
        &mut self,
        db: &dyn TimeControlDb,
        params: &TimeControlUpdateParams,
        blueprint_ctx: Option<&impl BlueprintContext>,
    ) -> TimeControlResponse {
        let TimeControlUpdateParams {
            stable_dt,
            more_data_is_streaming_in,
            is_buffering,
            should_diff_state,
        } = *params;

        let (old_playing, old_timeline, old_state) = (
            self.playing,
            self.timeline().copied(),
            self.states.get(self.timeline_name()).copied(),
        );

        if let Some(blueprint_ctx) = blueprint_ctx {
            self.update_from_blueprint(blueprint_ctx, Some(db));
        } else {
            self.select_valid_timeline(db);
        }

        let Some(full_range) = db.time_range_for(self.timeline_name()) else {
            return TimeControlResponse::no_repaint(); // we have no data on this timeline yet, so bail
        };

        let needs_repaint = match self.play_state() {
            PlayState::Paused => {
                // It's possible that the playback is paused because e.g. it reached its end, but
                // then the user decides to switch timelines.
                // When they do so, it might be the case that they switch to a timeline they've
                // never interacted with before, in which case we don't even have a time state yet.
                let state = self.states.entry(*self.timeline_name()).or_insert_with(|| {
                    TimeState::new(if self.following {
                        full_range.max()
                    } else {
                        full_range.min()
                    })
                });

                state.last_paused_time = Some(state.time);
                self.start_buffering(); // in case we hit play again!
                NeedsRepaint::No
            }

            PlayState::Playing => {
                let state = self
                    .states
                    .entry(*self.timeline_name())
                    .or_insert_with(|| TimeState::new(full_range.min()));

                if self.buffer_behavior.pauses_on_buffer() && is_buffering {
                    // Do not move time cursor until we are done buffering
                    NeedsRepaint::No
                } else {
                    // Don't auto-pause once we are actually playing.
                    // `AlwaysBuffer` is sticky and stays in place.
                    if self.buffer_behavior == BufferBehavior::WaitForDataThenPlay {
                        self.buffer_behavior = BufferBehavior::Play;
                    }

                    let dt = stable_dt.min(0.1) * self.speed;

                    if self.loop_mode == LoopMode::Off && full_range.max() <= state.time {
                        // We've reached the end of the data
                        self.set_time_ad_hoc(full_range.max().into());

                        if more_data_is_streaming_in {
                            // then let's wait for it without pausing!
                        } else {
                            self.pause(blueprint_ctx);
                        }
                        NeedsRepaint::No
                    } else {
                        let mut new_time = state.time;

                        let loop_range = match self.loop_mode {
                            LoopMode::Off => None,
                            LoopMode::Selection => state.time_selection,
                            LoopMode::All => Some(full_range.into()),
                        };

                        match self.timeline.timeline().map(|t| t.typ()) {
                            Some(TimeType::Sequence) => {
                                new_time += TimeReal::from(state.fps * dt);
                            }
                            Some(TimeType::DurationNs | TimeType::TimestampNs) => {
                                new_time += TimeReal::from(Duration::from_secs(dt));
                            }
                            None => {}
                        }

                        if let Some(loop_range) = loop_range
                            && loop_range.max < new_time
                        {
                            new_time = loop_range.min; // loop!
                        }

                        self.set_time_ad_hoc(new_time);

                        NeedsRepaint::Yes
                    }
                }
            }
            PlayState::Following => {
                // Set the time to the max:
                self.set_time_ad_hoc(full_range.max().into());

                NeedsRepaint::No // no need for request_repaint - we already repaint when new data arrives
            }
        };

        self.apply_state_diff_if_needed(
            TimeControlResponse::new(needs_repaint),
            should_diff_state,
            db,
            old_timeline,
            old_playing,
            old_state,
        )
    }

    /// Apply state diff to response if needed.
    #[expect(clippy::fn_params_excessive_bools)] // TODO(emilk): remove bool parameters
    fn apply_state_diff_if_needed(
        &mut self,
        response: TimeControlResponse,
        should_diff_state: bool,
        db: &dyn TimeControlDb,
        old_timeline: Option<Timeline>,
        old_playing: bool,
        old_state: Option<TimeState>,
    ) -> TimeControlResponse {
        let mut response = response;

        if should_diff_state && db.time_range_for(self.timeline_name()).is_some() {
            self.diff_with(&mut response, old_timeline, old_playing, old_state);
        }

        response
    }

    /// Handle updating last frame state and trigger callbacks on changes.
    fn diff_with(
        &mut self,
        response: &mut TimeControlResponse,
        old_timeline: Option<Timeline>,
        old_playing: bool,
        old_state: Option<TimeState>,
    ) {
        if old_playing != self.playing {
            response.playing_change = Some(self.playing);
        }

        if old_timeline != self.timeline().copied() {
            let time = self
                .time_for_timeline(*self.timeline_name())
                .unwrap_or(TimeReal::MIN);

            response.timeline_change = self.timeline().map(|t| (*t, time));
        }

        if let Some(state) = self.states.get_mut(self.timeline.name()) {
            // TODO(jan): throttle?
            if old_state.is_none_or(|old_state| old_state.time != state.time) {
                response.time_change = Some(state.time);
            }
        }
    }

    pub fn play_state(&self) -> PlayState {
        if self.playing {
            if self.following {
                PlayState::Following
            } else {
                PlayState::Playing
            }
        } else {
            PlayState::Paused
        }
    }

    pub fn loop_mode(&self) -> LoopMode {
        if self.play_state() == PlayState::Following {
            LoopMode::Off
        } else {
            self.loop_mode
        }
    }

    /// Updates the current play-state.
    ///
    /// If `blueprint_ctx` is specified this writes to the related
    /// blueprint.
    pub fn set_play_state(
        &mut self,
        db: Option<&dyn TimeControlDb>,
        play_state: PlayState,
        blueprint_ctx: Option<&impl BlueprintContext>,
    ) {
        if let Some(blueprint_ctx) = blueprint_ctx
            && Some(play_state) != blueprint_ctx.play_state()
        {
            blueprint_ctx.set_play_state(play_state);
        }

        match play_state {
            PlayState::Paused => {
                self.playing = false;
            }
            PlayState::Playing => {
                self.playing = true;
                self.following = false;
                self.start_buffering();

                // Start from beginning if we are at the end:
                if let Some(db) = db
                    && let Some(range) = db.time_range_for(self.timeline_name())
                {
                    if let Some(state) = self.states.get_mut(self.timeline.name()) {
                        if range.max <= state.time {
                            state.time = range.min.into();
                        }
                    } else {
                        self.states
                            .insert(*self.timeline_name(), TimeState::new(range.min));
                    }
                }
            }
            PlayState::Following => {
                self.playing = true;
                self.following = true;

                if let Some(db) = db
                    && let Some(range) = db.time_range_for(self.timeline_name())
                {
                    // Set the time to the max:
                    self.states
                        .entry(*self.timeline_name())
                        .or_insert_with(|| TimeState::new(range.max))
                        .time = range.max.into();
                }
            }
        }
    }

    fn pause(&mut self, blueprint_ctx: Option<&impl BlueprintContext>) {
        self.playing = false;
        if let Some(blueprint_ctx) = blueprint_ctx {
            blueprint_ctx.set_play_state(PlayState::Paused);
        }
        if let Some(state) = self.states.get_mut(self.timeline.name()) {
            state.last_paused_time = Some(state.time);
        }
    }

    /// Get the current [`re_entity_db::PrefetchTimeCursor`].
    ///
    /// If the whole recording is looped the loop range is
    /// `TimeInt::MIN..=TimeInt::MAX`.
    pub fn time_cursor(&self) -> Option<re_entity_db::PrefetchTimeCursor> {
        let typ = self.time_type()?;
        let speed_if_unpaused = match typ {
            TimeType::DurationNs | TimeType::TimestampNs => {
                TimeInt::from_secs(1.0).as_f64() * self.speed as f64
            }
            TimeType::Sequence => self.fps()? as f64 * self.speed as f64,
        };

        let loop_range = if self.loop_mode == LoopMode::All {
            Some(AbsoluteTimeRange::new(TimeInt::MIN, TimeInt::MAX))
        } else {
            self.active_loop_selection().map(|r| r.to_int())
        };

        Some(re_entity_db::PrefetchTimeCursor {
            time_cursor: re_log_types::TimelinePoint {
                name: *self.timeline_name(),
                typ,
                time: self.time_int()?,
            },
            speed_if_unpaused,
            loop_range,
        })
    }

    /// playback speed
    pub fn speed(&self) -> f32 {
        self.speed
    }

    /// playback fps
    pub fn fps(&self) -> Option<f32> {
        self.states.get(self.timeline_name()).map(|state| state.fps)
    }

    /// Make sure the selected timeline is a valid one
    fn select_valid_timeline(&mut self, db: &dyn TimeControlDb) {
        let timelines = db.timelines();

        let reset_timeline = match &self.timeline {
            // If the timeline is auto refresh it every frame.
            ActiveTimeline::Auto(_) => true,
            // If it's user edited, refresh it if it's invalid.
            ActiveTimeline::UserEdited(selected) => !timelines.contains_key(selected.name()),
            // If it's pending never automatically refresh it.
            ActiveTimeline::Pending(timeline_name) => {
                // If the pending timeline is valid, it shouldn't be pending anymore.
                if let Some(timeline) = timelines.get(timeline_name) {
                    self.timeline = ActiveTimeline::UserEdited(*timeline);
                }

                false
            }
        };

        if reset_timeline || matches!(self.timeline, ActiveTimeline::Auto(_)) {
            self.timeline =
                ActiveTimeline::Auto(Timeline::pick_best_timeline(timelines.values(), |t| {
                    db.num_temporal_rows_on_timeline(t.name())
                }));
        }
    }

    /// The currently selected timeline
    #[inline]
    pub fn timeline(&self) -> Option<&Timeline> {
        self.timeline.timeline()
    }

    pub fn timeline_name(&self) -> &TimelineName {
        self.timeline.name()
    }

    /// The time type of the currently selected timeline
    pub fn time_type(&self) -> Option<TimeType> {
        self.timeline().map(|t| t.typ())
    }

    /// The current time.
    pub fn time(&self) -> Option<TimeReal> {
        self.states
            .get(self.timeline_name())
            .map(|state| state.time)
    }

    pub fn last_paused_time(&self) -> Option<TimeReal> {
        if matches!(self.play_state(), PlayState::Paused) {
            self.time()
        } else {
            self.states
                .get(self.timeline_name())
                .and_then(|state| state.last_paused_time)
        }
    }

    /// The current time & timeline.
    pub fn time_cell(&self) -> Option<TimeCell> {
        let t = self.time()?;
        Some(TimeCell::new(self.time_type()?, t.floor().as_i64()))
    }

    /// The current time.
    pub fn time_int(&self) -> Option<TimeInt> {
        self.time().map(|t| t.floor())
    }

    /// The current time.
    pub fn time_i64(&self) -> Option<i64> {
        self.time().map(|t| t.floor().as_i64())
    }

    /// Query for latest value at the currently selected time on the currently selected timeline.
    pub fn current_query(&self) -> re_chunk_store::LatestAtQuery {
        re_chunk_store::LatestAtQuery::new(
            *self.timeline_name(),
            self.time().map_or(TimeInt::MAX, |t| t.floor()),
        )
    }

    /// The current loop range, if selection looping is turned on.
    pub fn active_loop_selection(&self) -> Option<AbsoluteTimeRangeF> {
        if self.loop_mode == LoopMode::Selection {
            self.states.get(self.timeline_name())?.time_selection
        } else {
            None
        }
    }

    /// The selected slice of time that is called the "loop selection".
    ///
    /// This can still return `Some` even if looping is currently off.
    pub fn time_selection(&self) -> Option<AbsoluteTimeRangeF> {
        self.states.get(self.timeline_name())?.time_selection
    }

    /// Is the current time in the selection range (if any), or at the current time mark?
    pub fn is_time_selected(&self, timeline: &TimelineName, needle: TimeInt) -> bool {
        if timeline != self.timeline_name() {
            return false;
        }

        if let Some(state) = self.states.get(self.timeline_name()) {
            state.time.floor() == needle
        } else {
            false
        }
    }

    /// Is the active timeline pending resolution?
    ///
    /// When `true`, the requested timeline name hasn't been found in the data yet,
    /// so [`Self::timeline()`] returns `None` and time-dependent queries may not work.
    pub fn is_pending(&self) -> bool {
        matches!(self.timeline, ActiveTimeline::Pending(_))
    }

    pub fn time_for_timeline(&self, timeline: TimelineName) -> Option<TimeReal> {
        self.states.get(&timeline).map(|state| state.time)
    }

    /// The range of time we are currently zoomed in on.
    pub fn time_view(&self) -> Option<TimeView> {
        self.states
            .get(self.timeline_name())
            .and_then(|state| state.view)
    }
}
