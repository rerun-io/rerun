//! Tests for time control callbacks and viewer events.
use std::cell::RefCell;
use std::rc::Rc;

use egui_kittest::Harness;
use re_integration_test::HarnessExt as _;
use re_sdk::Timeline;
use re_sdk::external::re_log_types::TimeReal;
use re_sdk::log::RowId;
use re_viewer::App;
use re_viewer::event::{ViewerEvent, ViewerEventDispatcher, ViewerEventKind};
use re_viewer::external::re_sdk_types::archetypes::TextLog;
use re_viewer::external::re_sdk_types::blueprint::components::PlayState;
use re_viewer::external::re_viewer_context::TimeControlCommand;
use re_viewer::viewer_test_utils::{self, HarnessOptions};

/// A simple event collector that records viewer events for later inspection.
struct EventCollector {
    events: Rc<RefCell<Vec<ViewerEvent>>>,
}

impl EventCollector {
    /// Creates a new event collector and hooks it into the given harness.
    fn init(harness: &mut Harness<'_, App>) -> Self {
        let events = Rc::new(RefCell::new(Vec::new()));
        {
            let events_clone = Rc::clone(&events);
            harness.state_mut().event_dispatcher =
                Some(ViewerEventDispatcher::new(Rc::new(move |event| {
                    events_clone.borrow_mut().push(event);
                })));
        }
        harness.run_ok();
        Self { events }
    }

    fn take(&self) -> Vec<ViewerEvent> {
        self.events.borrow_mut().drain(..).collect()
    }

    fn received_event<F: Fn(&ViewerEventKind) -> bool>(&self, f: F) -> bool {
        self.events.borrow().iter().any(|event| f(&event.kind))
    }
}

fn log_time_data(harness: &mut Harness<'_, App>, timeline: Timeline) {
    for i in 0..100 {
        harness.log_entity("test_entity", |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline, i)],
                &TextLog::new(format!("Log {i}")),
            )
        });
    }
}

fn send_time_commands(
    harness: &mut Harness<'_, App>,
    commands: impl IntoIterator<Item = TimeControlCommand>,
) {
    let commands: Vec<_> = commands.into_iter().collect();
    harness.run_with_viewer_context(move |ctx| {
        ctx.send_time_commands(commands);
    });

    harness.run_ok();
}

fn assert_play_state(harness: &mut Harness<'_, App>, expected: PlayState) {
    let actual = harness.run_with_viewer_context(|ctx| ctx.time_ctrl.play_state());
    assert_eq!(actual, expected, "play state mismatch");
}

/// Verifies that switching timelines emits the appropriate viewer event.
#[tokio::test]
async fn time_control_emits_timeline_switch_event() {
    #![expect(unsafe_code)] // It's only a test

    // SAFETY: it's only a test
    unsafe {
        std::env::set_var("TZ", "Europe/Stockholm");
    }

    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions::default());
    harness.init_recording();

    let timeline_a = Timeline::new_sequence("timeline_a");
    let timeline_b = Timeline::new_sequence("timeline_b");
    for i in 0..100 {
        harness.log_entity("test_entity", |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline_a, i), (timeline_b, i * 2)],
                &TextLog::new(format!("Log {i}")),
            )
        });
    }

    let initial_timeline =
        harness.run_with_viewer_context(move |ctx| *ctx.time_ctrl.timeline_name());
    let alternate_timeline = if initial_timeline == *timeline_a.name() {
        timeline_b
    } else {
        timeline_a
    };

    let events = EventCollector::init(&mut harness);

    // Switch timelines
    send_time_commands(
        &mut harness,
        [TimeControlCommand::SetActiveTimeline(
            *alternate_timeline.name(),
        )],
    );
    let timeline_after_switch =
        harness.run_with_viewer_context(|ctx| *ctx.time_ctrl.timeline_name());
    assert_eq!(timeline_after_switch, *alternate_timeline.name());

    assert!(
        events.received_event(|kind| {
            matches!(
                kind,
                ViewerEventKind::TimelineChange { timeline_name, .. }
                    if timeline_name == alternate_timeline.name()
            )
        }),
        "expected timeline change event when switching timelines"
    );
}

/// Verifies that time control commands emit the appropriate viewer events.
#[tokio::test]
async fn time_control_emits_expected_viewer_events() {
    #![expect(unsafe_code)] // It's only a test

    // SAFETY: it's only a test
    unsafe {
        std::env::set_var("TZ", "Europe/Stockholm");
    }

    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions::default());
    harness.init_recording();
    log_time_data(&mut harness, Timeline::new_sequence("test_timeline"));

    // Since we start in following mode, we first need to switch to playing in order to disable following mode.
    send_time_commands(
        &mut harness,
        [TimeControlCommand::SetPlayState(PlayState::Playing)],
    );

    let events = EventCollector::init(&mut harness);

    // First, pause playback
    send_time_commands(&mut harness, [TimeControlCommand::Pause]);
    assert_play_state(&mut harness, PlayState::Paused);
    assert!(
        events.received_event(|kind| matches!(kind, ViewerEventKind::Pause)),
        "expected pause event when pausing playback"
    );

    // Seek to a specific time
    let specific_time = TimeReal::from(4_i64);
    send_time_commands(&mut harness, [TimeControlCommand::SetTime(specific_time)]);
    let time_after_seek = harness.run_with_viewer_context(|ctx| ctx.time_ctrl.time().unwrap());
    assert_eq!(time_after_seek, specific_time);
    assert!(
        events.received_event(
            |kind| matches!(kind, ViewerEventKind::TimeUpdate { time } if *time == specific_time)
        ),
        "expected time update event when seeking"
    );

    // Make sure toggling play/pause works as expected
    assert_play_state(&mut harness, PlayState::Paused);
    send_time_commands(&mut harness, [TimeControlCommand::TogglePlayPause]);
    assert_play_state(&mut harness, PlayState::Playing);
    assert!(
        events.received_event(|kind| matches!(kind, ViewerEventKind::Play)),
        "expected play event when starting playback",
    );

    send_time_commands(&mut harness, [TimeControlCommand::TogglePlayPause]);
    assert_play_state(&mut harness, PlayState::Paused);
    assert!(
        events.received_event(|kind| matches!(kind, ViewerEventKind::Pause)),
        "expected pause event when pausing playback again",
    );

    // Finally, switch to following mode
    send_time_commands(
        &mut harness,
        [TimeControlCommand::SetPlayState(PlayState::Following)],
    );
    assert_play_state(&mut harness, PlayState::Following);
    assert!(
        events.received_event(|kind| matches!(kind, ViewerEventKind::Play)),
        "expected play event when switching to following",
    );
}

/// Verifies that time update events are emitted whilst playing.
#[tokio::test]
async fn test_time_control_update_emits_time_update_events() {
    #![expect(unsafe_code)] // It's only a test

    // SAFETY: it's only a test
    unsafe {
        std::env::set_var("TZ", "Europe/Stockholm");
    }

    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions::default());
    harness.init_recording();
    let events = EventCollector::init(&mut harness);

    let timeline = Timeline::new_sequence("test_timeline");
    for i in 0..100 {
        harness.log_entity("/test_entity", |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline, i)],
                &TextLog::new(format!("Log {i}")),
            )
        });
    }

    // Ensure that an initial timeline change event is sent out for the new timeline.
    events.received_event(|kind| matches!(kind, ViewerEventKind::TimelineChange { timeline_name, .. } if timeline_name == timeline.name()));

    send_time_commands(
        &mut harness,
        [
            // We need to force `PlayState::Playing`, because we start in `PlayState::Following`
            TimeControlCommand::SetPlayState(PlayState::Playing),
            TimeControlCommand::Pause,
            TimeControlCommand::SetTime(TimeReal::from(0_i64)),
        ],
    );

    assert_eq!(
        harness.run_with_viewer_context(|ctx| ctx.time_ctrl.time().unwrap()),
        TimeReal::from(0_i64)
    );

    // Clear events before playing
    send_time_commands(&mut harness, [TimeControlCommand::TogglePlayPause]);
    events.take();

    // It is very important that no steps are taken before this loop, or the count will be off.
    let num_steps = 10;
    for _ in 0..num_steps {
        harness.step();
    }

    let num_time_update_events = events
        .take()
        .into_iter()
        .filter(|event| matches!(event.kind, ViewerEventKind::TimeUpdate { .. }))
        .count();

    assert_eq!(num_time_update_events, num_steps);
}
