//! Playback tests: how the time cursor moves when playing forwards and backwards,
//! and that [`LoopMode`] correctly loops the playback.

use std::sync::Arc;

use re_chunk::{Chunk, RowId};
use re_entity_db::EntityDb;
use re_log_types::{AbsoluteTimeRange, StoreId, TimeInt, TimeReal, Timeline, TimelineName};
use re_sdk_types::archetypes::Scalars;
use re_sdk_types::blueprint::components::{LoopMode, PlayState};

use crate::blueprint_helpers::AppBlueprintCtx;

use super::{TimeControl, TimeControlCommand, TimeControlUpdateParams};

const NO_BLUEPRINT: Option<&AppBlueprintCtx<'static>> = None;

/// Dummy step time.
const DT: f32 = 0.1;

const SEQUENCE_STEP: f64 = 3.0;

const LAST_FRAME: i64 = 60;

fn sequence_timeline() -> Timeline {
    Timeline::new_sequence("frame")
}

fn duration_timeline() -> Timeline {
    Timeline::new_duration("time")
}

fn dummy_recording(timeline: Timeline, times: impl IntoIterator<Item = i64>) -> EntityDb {
    let mut db = EntityDb::new(StoreId::recording("test_app", "test_recording"));

    for (i, time) in times.into_iter().enumerate() {
        let chunk = Chunk::builder("/scalar")
            .with_archetype(RowId::new(), [(timeline, time)], &Scalars::single(i as f64))
            .build()
            .expect("building a chunk with a single scalar should succeed");

        db.add_chunk(&Arc::new(chunk))
            .expect("adding a chunk to a new store should succeed");
    }

    db
}

fn sequence_recording() -> EntityDb {
    dummy_recording(sequence_timeline(), (0..=LAST_FRAME).step_by(10))
}

fn send(time_ctrl: &mut TimeControl, db: &EntityDb, commands: &[TimeControlCommand]) {
    let _response = time_ctrl.handle_time_commands(NO_BLUEPRINT, db, commands);
}

fn step(time_ctrl: &mut TimeControl, db: &EntityDb) {
    run_update(time_ctrl, db, false);
}

fn step_while_streaming_in(time_ctrl: &mut TimeControl, db: &EntityDb) {
    run_update(time_ctrl, db, true);
}

fn run_update(time_ctrl: &mut TimeControl, db: &EntityDb, more_data_is_streaming_in: bool) {
    let _response = time_ctrl.update(
        db,
        &TimeControlUpdateParams {
            stable_dt: DT,
            more_data_is_streaming_in,
            is_buffering: false,
            should_diff_state: false,
        },
        NO_BLUEPRINT,
    );
}

fn time(time_ctrl: &TimeControl) -> TimeReal {
    time_ctrl
        .time()
        .expect("time control should have a time on the active timeline")
}

/// A time control playing `timeline` from `start_time` at `speed`, with looping off.
fn playing(
    db: &EntityDb,
    timeline: &TimelineName,
    start_time: impl Into<TimeReal>,
    speed: f32,
) -> TimeControl {
    let mut time_ctrl = TimeControl::default();

    send(
        &mut time_ctrl,
        db,
        &[
            TimeControlCommand::SetActiveTimeline(*timeline),
            TimeControlCommand::SetPlayState(PlayState::Playing),
            TimeControlCommand::SetSpeed(speed),
            TimeControlCommand::SetTime(start_time.into()),
        ],
    );

    if time_ctrl.just_interacted {
        // Some interactions stop time movement for one frame.
        step(&mut time_ctrl, db);
    }

    assert_eq!(time_ctrl.play_state(), PlayState::Playing);

    time_ctrl
}

#[test]
fn playing_forwards_moves_time_by_fps_times_speed() {
    let db = sequence_recording();
    let timeline = *sequence_timeline().name();

    let mut time_ctrl = playing(&db, &timeline, 0_i64, 1.0);
    step(&mut time_ctrl, &db);
    assert_eq!(time(&time_ctrl), TimeReal::from(SEQUENCE_STEP));
    step(&mut time_ctrl, &db);
    assert_eq!(time(&time_ctrl), TimeReal::from(2.0 * SEQUENCE_STEP));

    // Double speed covers twice as much time per frame.
    let mut time_ctrl = playing(&db, &timeline, 0_i64, 2.0);
    step(&mut time_ctrl, &db);
    assert_eq!(time(&time_ctrl), TimeReal::from(2.0 * SEQUENCE_STEP));
}

#[test]
fn playing_backwards_moves_time_in_reverse() {
    let db = sequence_recording();
    let timeline = *sequence_timeline().name();

    let mut time_ctrl = playing(&db, &timeline, 30_i64, -1.0);
    step(&mut time_ctrl, &db);
    assert_eq!(time(&time_ctrl), TimeReal::from(30.0 - SEQUENCE_STEP));
    step(&mut time_ctrl, &db);
    assert_eq!(time(&time_ctrl), TimeReal::from(30.0 - 2.0 * SEQUENCE_STEP));
}

#[test]
fn toggling_reverse_playback_starts_from_the_end() {
    let db = sequence_recording();

    for start_time in [0, LAST_FRAME] {
        let mut time_ctrl = playing(&db, sequence_timeline().name(), start_time, -1.0);
        send(&mut time_ctrl, &db, &[TimeControlCommand::Pause]);
        assert_eq!(time_ctrl.play_state(), PlayState::Paused);
        assert_eq!(time(&time_ctrl), TimeReal::from(start_time));

        send(&mut time_ctrl, &db, &[TimeControlCommand::TogglePlayPause]);
        assert_eq!(time_ctrl.play_state(), PlayState::Playing);
        assert_eq!(time(&time_ctrl), TimeReal::from(LAST_FRAME));

        step(&mut time_ctrl, &db);
        assert_eq!(
            time(&time_ctrl),
            TimeReal::from(LAST_FRAME as f64 - SEQUENCE_STEP)
        );
    }
}

#[test]
fn playing_on_a_duration_timeline_moves_time_by_wall_time() {
    let one_second = TimeInt::from_secs(1.0).as_i64();
    let db = dummy_recording(duration_timeline(), [0, one_second]);

    let mut time_ctrl = playing(&db, duration_timeline().name(), 0_i64, 1.0);

    let num_frames = 3;
    for _ in 0..num_frames {
        step(&mut time_ctrl, &db);
    }

    // Wall time is scaled from a `f32` seconds delta, so allow for a little rounding.
    let expected = TimeInt::from_secs(f64::from(DT) * f64::from(num_frames)).as_i64();
    let elapsed = time(&time_ctrl).floor().as_i64();
    assert!(
        (elapsed - expected).abs() < 1_000,
        "expected roughly {expected}ns of playback, got {elapsed}ns"
    );
}

#[test]
fn playing_forwards_without_looping_pauses_at_the_end_of_the_data() {
    let db = sequence_recording();
    let mut time_ctrl = playing(&db, sequence_timeline().name(), 0_i64, 1.0);
    assert_eq!(time_ctrl.loop_mode(), LoopMode::Off);

    // One frame to reach the end, plus the frame that notices we got there.
    let num_frames = (LAST_FRAME as f64 / SEQUENCE_STEP).ceil() as usize + 1;
    for _ in 0..num_frames {
        step(&mut time_ctrl, &db);
        assert!(
            time(&time_ctrl) <= TimeReal::from(LAST_FRAME),
            "the cursor should never move past the end of the data"
        );
    }

    assert_eq!(time_ctrl.play_state(), PlayState::Paused);
    assert_eq!(time(&time_ctrl), TimeReal::from(LAST_FRAME));
}

#[test]
fn playing_backwards_without_looping_pauses_at_the_start_of_the_data() {
    let db = sequence_recording();
    let mut time_ctrl = playing(&db, sequence_timeline().name(), LAST_FRAME, -1.0);
    assert_eq!(time_ctrl.loop_mode(), LoopMode::Off);

    let num_frames = (LAST_FRAME as f64 / SEQUENCE_STEP).ceil() as usize + 1;
    for _ in 0..num_frames {
        step(&mut time_ctrl, &db);
        assert!(
            TimeReal::from(0_i64) <= time(&time_ctrl),
            "the cursor should never move past the start of the data"
        );
    }

    assert_eq!(time_ctrl.play_state(), PlayState::Paused);
    assert_eq!(time(&time_ctrl), TimeReal::from(0_i64));
}

#[test]
fn playing_forwards_at_the_end_keeps_playing_while_more_data_is_streaming_in() {
    let db = sequence_recording();
    let start_time = TimeReal::from(LAST_FRAME) - TimeReal::from(SEQUENCE_STEP);
    let mut time_ctrl = playing(&db, sequence_timeline().name(), start_time, 1.0);

    for _ in 0..3 {
        step_while_streaming_in(&mut time_ctrl, &db);
    }

    assert!(time(&time_ctrl) <= TimeReal::from(LAST_FRAME));
    assert_eq!(
        time_ctrl.play_state(),
        PlayState::Playing,
        "playback should wait for the data still coming in instead of pausing"
    );
}

#[test]
fn playing_backwards_at_the_start_pauses_even_while_more_data_is_streaming_in() {
    let db = sequence_recording();
    let mut time_ctrl = playing(&db, sequence_timeline().name(), SEQUENCE_STEP, -1.0);

    // New data always shows up at the end of the recording, so it can never help
    // playback that is heading for the start.
    for _ in 0..3 {
        step_while_streaming_in(&mut time_ctrl, &db);
    }

    assert_eq!(time_ctrl.play_state(), PlayState::Paused);
    assert_eq!(time(&time_ctrl), TimeReal::from(0_i64));
}

#[test]
fn playing_clamps_a_cursor_sitting_outside_of_the_data() {
    let db = sequence_recording();

    // Past the end, playing forwards.
    let mut time_ctrl = playing(&db, sequence_timeline().name(), LAST_FRAME + 30, 1.0);
    step(&mut time_ctrl, &db);
    assert_eq!(time(&time_ctrl), TimeReal::from(LAST_FRAME));
    assert_eq!(time_ctrl.play_state(), PlayState::Paused);

    // Before the start, playing backwards.
    let mut time_ctrl = playing(&db, sequence_timeline().name(), -30_i64, -1.0);
    step(&mut time_ctrl, &db);
    assert_eq!(time(&time_ctrl), TimeReal::from(0_i64));
    assert_eq!(time_ctrl.play_state(), PlayState::Paused);
}

#[test]
fn looping_over_everything_wraps_at_the_end_of_the_data() {
    let db = sequence_recording();
    let mut time_ctrl = playing(
        &db,
        sequence_timeline().name(),
        TimeReal::from(LAST_FRAME) - TimeReal::from(SEQUENCE_STEP),
        1.0,
    );
    send(
        &mut time_ctrl,
        &db,
        &[TimeControlCommand::SetLoopMode(LoopMode::All)],
    );

    step(&mut time_ctrl, &db);
    assert_eq!(time(&time_ctrl), TimeReal::from(LAST_FRAME));

    step(&mut time_ctrl, &db);
    assert_eq!(time(&time_ctrl), TimeReal::from(0_i64));

    // Looping keeps playing instead of pausing at the end.
    assert_eq!(time_ctrl.play_state(), PlayState::Playing);
    step(&mut time_ctrl, &db);
    assert_eq!(time(&time_ctrl), TimeReal::from(SEQUENCE_STEP));
    assert_eq!(time_ctrl.play_state(), PlayState::Playing);
}

#[test]
fn looping_over_everything_wraps_at_the_start_when_playing_backwards() {
    let db = sequence_recording();
    let mut time_ctrl = playing(&db, sequence_timeline().name(), SEQUENCE_STEP, -1.0);
    send(
        &mut time_ctrl,
        &db,
        &[TimeControlCommand::SetLoopMode(LoopMode::All)],
    );

    step(&mut time_ctrl, &db);
    assert_eq!(time(&time_ctrl), TimeReal::from(0_i64));

    step(&mut time_ctrl, &db);
    assert_eq!(time(&time_ctrl), TimeReal::from(LAST_FRAME));
    assert_eq!(time_ctrl.play_state(), PlayState::Playing);
}

/// A time control looping the given selection, playing from `start_time` at `speed`.
fn looping_selection(
    db: &EntityDb,
    selection: AbsoluteTimeRange,
    start_time: impl Into<TimeReal>,
    speed: f32,
) -> TimeControl {
    let mut time_ctrl = playing(db, sequence_timeline().name(), start_time, speed);

    send(
        &mut time_ctrl,
        db,
        &[
            TimeControlCommand::SetTimeSelection(selection),
            TimeControlCommand::SetLoopMode(LoopMode::Selection),
        ],
    );
    assert_eq!(time_ctrl.loop_mode(), LoopMode::Selection);
    assert_eq!(
        time_ctrl.active_loop_selection(),
        Some(selection.into()),
        "selection looping should report the selection as its loop range"
    );

    time_ctrl
}

#[test]
fn looping_a_selection_wraps_within_the_selection() {
    let db = sequence_recording();
    let selection = AbsoluteTimeRange::new(20, 40);
    let mut time_ctrl = looping_selection(&db, selection, selection.min(), 1.0);

    let mut wrapped = false;
    for _ in 0..20 {
        let before = time(&time_ctrl);
        step(&mut time_ctrl, &db);
        let after = time(&time_ctrl);

        if after < before {
            wrapped = true;
            assert_eq!(
                after,
                TimeReal::from(selection.min()),
                "wrapping should put the cursor back at the start of the selection"
            );
        }

        assert!(
            TimeReal::from(selection.min()) <= after && after <= TimeReal::from(selection.max()),
            "the cursor should stay inside the selection"
        );
    }

    assert!(wrapped, "playback should have wrapped at least once");
    assert_eq!(
        time_ctrl.play_state(),
        PlayState::Playing,
        "selection looping should not pause"
    );
}

#[test]
fn looping_a_selection_wraps_at_its_start_when_playing_backwards() {
    let db = sequence_recording();
    let selection = AbsoluteTimeRange::new(20, 40);
    let mut time_ctrl = looping_selection(
        &db,
        selection,
        TimeReal::from(selection.min()) + TimeReal::from(1.0),
        -1.0,
    );

    step(&mut time_ctrl, &db);
    assert_eq!(time(&time_ctrl), TimeReal::from(selection.max()));
    assert_eq!(time_ctrl.play_state(), PlayState::Playing);
}

#[test]
fn looping_a_selection_pulls_in_a_cursor_outside_of_it() {
    let db = sequence_recording();
    let selection = AbsoluteTimeRange::new(20, 40);
    let mut time_ctrl = looping_selection(&db, selection, LAST_FRAME, 1.0);

    step(&mut time_ctrl, &db);
    assert_eq!(time(&time_ctrl), TimeReal::from(selection.min()));
}

#[test]
fn removing_the_time_selection_turns_off_selection_looping() {
    let db = sequence_recording();
    let selection = AbsoluteTimeRange::new(20, 40);
    let mut time_ctrl = looping_selection(&db, selection, selection.min(), 1.0);

    send(
        &mut time_ctrl,
        &db,
        &[TimeControlCommand::RemoveTimeSelection],
    );

    assert_eq!(time_ctrl.loop_mode(), LoopMode::Off);
    assert_eq!(time_ctrl.active_loop_selection(), None);
    assert_eq!(time_ctrl.time_selection(), None);
}

#[test]
fn looping_and_follow_mode_are_mutually_exclusive() {
    let db = sequence_recording();
    let mut time_ctrl = TimeControl::default();
    send(
        &mut time_ctrl,
        &db,
        &[
            TimeControlCommand::SetActiveTimeline(*sequence_timeline().name()),
            TimeControlCommand::SetPlayState(PlayState::Following),
        ],
    );
    assert_eq!(time_ctrl.play_state(), PlayState::Following);

    // Turning on looping leaves follow mode.
    send(
        &mut time_ctrl,
        &db,
        &[TimeControlCommand::SetLoopMode(LoopMode::All)],
    );
    assert_eq!(time_ctrl.play_state(), PlayState::Playing);
    assert_eq!(time_ctrl.loop_mode(), LoopMode::All);

    // And going back to follow mode turns looping off.
    send(
        &mut time_ctrl,
        &db,
        &[TimeControlCommand::SetPlayState(PlayState::Following)],
    );
    assert_eq!(time_ctrl.play_state(), PlayState::Following);
    assert_eq!(time_ctrl.loop_mode(), LoopMode::Off);
}
