use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use re_integration_test::{HarnessExt as _, ViewerHarnessExt as _};
use re_log_types::{TimeReal, Timeline};
use re_sdk_types::archetypes::TextLog;
use re_view_text_log::TextView;
use re_viewer::App;
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewer_context::{TimeControlCommand, ViewClass as _};
use re_viewport_blueprint::ViewBlueprint;

fn harness(times: impl IntoIterator<Item = i64>, time: i64) -> Harness<'static, App> {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        window_size: Some(egui::vec2(1200.0, 400.0)),
        ..Default::default()
    });
    harness.init_recording();
    harness.set_blueprint_panel_opened(false);
    harness.set_selection_panel_opened(false);
    harness.set_time_panel_opened(false);
    harness.clear_current_blueprint();
    append_text_logs(&mut harness, times);
    seek(&mut harness, time);
    harness.setup_viewport_blueprint(|_, blueprint| {
        let mut view = ViewBlueprint::new_with_root_wildcard(TextView::identifier());
        view.display_name = Some("Logs".to_owned());
        blueprint.add_view_at_root(view);
    });
    harness
}

fn append_text_logs(harness: &mut Harness<'_, App>, times: impl IntoIterator<Item = i64>) {
    let times: Vec<_> = times.into_iter().collect();
    if times.is_empty() {
        return;
    }
    harness.log_entity("logs", |mut builder| {
        for time in times {
            builder = builder.with_archetype_auto_row(
                [(Timeline::log_tick(), time)],
                &TextLog::new(format!("message {time}")),
            );
        }
        builder
    });
}

fn seek(harness: &mut Harness<'_, App>, time: i64) {
    harness.send_time_commands([
        TimeControlCommand::SetActiveTimeline(*Timeline::log_tick().name()),
        TimeControlCommand::Pause,
        TimeControlCommand::SetTime(TimeReal::from(time)),
    ]);
}

fn messages(harness: &Harness<'_, App>, region: egui::Rect) -> Vec<String> {
    harness
        .query_all_by_label_contains("message ")
        .filter(|node| region.intersects(node.rect()))
        .filter_map(|node| node.value())
        .filter(|label| label.starts_with("message "))
        .collect()
}

fn scroll(harness: &mut Harness<'_, App>, pos: egui::Pos2, delta: f32) {
    harness.event(egui::Event::PointerMoved(pos));
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        delta: egui::vec2(0.0, delta),
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    harness.step_until("text log wheel stops scrolling", |harness| {
        !harness.ctx.input(|input| input.is_scrolling())
    });
    harness.event(egui::Event::PointerGone);
    harness.run();
}

#[tokio::test]
async fn cursor_positions_before_between_and_after_data_settle() {
    let mut harness = harness((0..1000).step_by(10), -100);
    harness.step_until("first text log row", |h| {
        h.query_by_label("message 0").is_some()
    });
    seek(&mut harness, 505);
    harness.step_until("rows surrounding playhead", |h| {
        h.query_by_label("message 500").is_some() && h.query_by_label("message 510").is_some()
    });
    seek(&mut harness, 2000);
    harness.step_until("last text log row", |h| {
        h.query_by_label("message 990").is_some()
    });
}

#[tokio::test]
async fn arrivals_preserve_manual_scroll_until_cursor_moves() {
    for arrival in [50, 119, 126, 1001] {
        let mut harness = harness(0..120, 1000);
        harness.step_until("initial tail", |h| {
            h.query_by_label("message 119").is_some()
        });
        let pane = harness.get_panel_position("Logs");
        scroll(&mut harness, pane.center(), 1000.0);
        let expected = messages(&harness, egui::Rect::EVERYTHING);
        assert!(!expected.is_empty());
        assert!(!expected.iter().any(|message| message == "message 119"));

        append_text_logs(&mut harness, [arrival]);
        // Inspect every frame so a transient jump cannot recover unnoticed.
        for _ in 0..8 {
            harness.step();
            let actual = messages(&harness, egui::Rect::EVERYTHING);
            if arrival == 50 {
                assert!(actual.contains(&expected[0]));
                assert!(!actual.iter().any(|message| message == "message 119"));
            } else {
                assert_eq!(actual, expected, "arrival at {arrival} moved the viewport");
            }
        }
        seek(&mut harness, arrival);
        harness.step_until("seek to arriving row", |h| {
            h.query_all_by_label(&format!("message {arrival}"))
                .next()
                .is_some()
        });
    }
}

#[tokio::test]
async fn arrivals_preserve_settled_viewport_until_playhead_moves() {
    let mut harness = harness(0..120, 1000);
    harness.step_until("initial tail", |h| {
        h.query_by_label("message 119").is_some()
    });
    harness.run();
    let expected: Vec<_> = messages(&harness, egui::Rect::EVERYTHING)
        .into_iter()
        .map(|label| {
            let rect = harness.get_by_label(&label).rect();
            (label, rect)
        })
        .collect();
    assert!(!expected.is_empty(), "The initial tail must be visible");
    append_text_logs(&mut harness, 120..160);
    for _ in 0..8 {
        harness.step();
        // Appends may fill unused space below the tail, but must not move existing rows.
        for (label, rect) in &expected {
            assert_eq!(
                harness.get_by_label(label).rect(),
                *rect,
                "Arrival moved {label}"
            );
        }
        assert!(harness.query_by_label("message 159").is_none());
    }
    seek(&mut harness, 159);
    harness.step_until("new tail after seek", |h| {
        h.query_by_label("message 159").is_some()
    });
}

#[tokio::test]
async fn initially_empty_recording_displays_newly_discovered_sources() {
    let mut harness = harness([], 100);
    assert!(messages(&harness, egui::Rect::EVERYTHING).is_empty());
    append_text_logs(&mut harness, [95]);
    harness.step_until("new text log source", |h| {
        h.query_by_label("message 95").is_some()
    });
}

#[tokio::test]
async fn filter_changes_update_visible_rows() {
    let mut harness = harness([], 0);
    for (tick, level) in [(0, "INFO"), (1000, "WARN")] {
        harness.log_entity("logs", |builder| {
            builder.with_archetype_auto_row(
                [(Timeline::log_tick(), tick)],
                &TextLog::new(format!("{level} message")).with_level(level),
            )
        });
    }
    harness.step_until("unfiltered text logs", |h| {
        h.query_by_label("INFO message").is_some() && h.query_by_label("WARN message").is_some()
    });
    harness.set_size(egui::vec2(1200.0, 900.0));
    harness.set_blueprint_panel_opened(true);
    harness.set_selection_panel_opened(true);
    harness.blueprint_tree().click_list_item("Logs");
    harness
        .selection_panel()
        .toggle_nth_hierarchical_list("Filter by log level", 0);
    harness.step_until("text log level controls", |h| {
        h.query_by_role_and_label(egui::Role::CheckBox, "INFO")
            .is_some()
    });
    harness
        .get_by_role_and_label(egui::Role::CheckBox, "INFO")
        .click();
    harness.step_until("filtered text logs", |h| {
        h.query_by_label("WARN message").is_some() && h.query_by_label("INFO message").is_none()
    });
}

#[tokio::test]
async fn shared_index_keeps_two_rendered_viewports_independent() {
    let mut harness = harness(0..200, 100);
    harness.clear_current_blueprint();
    let container = harness.add_blueprint_container(egui_tiles::ContainerKind::Horizontal, None);
    harness.setup_viewport_blueprint(move |_, blueprint| {
        blueprint.add_views(
            ["Left logs", "Right logs"]
                .map(|name| {
                    let mut view = ViewBlueprint::new_with_root_wildcard(TextView::identifier());
                    view.display_name = Some(name.to_owned());
                    view
                })
                .into_iter(),
            Some(container),
            None,
        );
    });
    let left = harness.get_panel_position("Left logs");
    let right = harness.get_panel_position("Right logs");
    harness.step_until("both views seek to playhead", |h| {
        [left, right]
            .into_iter()
            .all(|region| messages(h, region).contains(&"message 100".to_owned()))
    });
    harness.run();
    let expected_right = messages(&harness, right);
    scroll(&mut harness, left.center(), 600.0);
    let expected_left = messages(&harness, left);
    assert_ne!(expected_left, expected_right);
    assert_eq!(messages(&harness, right), expected_right);
    for _ in 0..5 {
        harness.step();
        assert_eq!(messages(&harness, left), expected_left);
        assert_eq!(messages(&harness, right), expected_right);
    }
    seek(&mut harness, 150);
    harness.step_until("both views follow new playhead", |h| {
        [left, right]
            .into_iter()
            .all(|region| messages(h, region).contains(&"message 150".to_owned()))
    });
}
