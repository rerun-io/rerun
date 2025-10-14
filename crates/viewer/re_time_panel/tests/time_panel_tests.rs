#![cfg(feature = "testing")]

use egui::Vec2;

use re_chunk_store::{LatestAtQuery, RowId};
use re_entity_db::InstancePath;
use re_log_types::{
    AbsoluteTimeRange, EntityPath, TimeInt, TimePoint, TimeType, Timeline, build_frame_nr,
    example_components::{MyPoint, MyPoints},
};
use re_test_context::{TestContext, external::egui_kittest::SnapshotOptions};
use re_time_panel::TimePanel;
use re_types::archetypes::Points2D;
use re_viewer_context::{CollapseScope, TimeControlCommand, TimeView, blueprint_timeline};
use re_viewport_blueprint::ViewportBlueprint;

fn add_sparse_data(test_context: &mut TestContext) {
    let points1 = MyPoint::from_iter(0..1);
    for i in 0..2 {
        test_context.log_entity(format!("/entity/{i}"), |mut builder| {
            for frame in [10, 11, 12, 15, 18, 100, 102, 104].map(|frame| frame + i) {
                builder = builder.with_sparse_component_batches(
                    RowId::new(),
                    [build_frame_nr(frame)],
                    [(MyPoints::descriptor_points(), Some(&points1 as _))],
                );
            }

            builder
        });
    }
}

#[test]
pub fn time_panel_two_sections() {
    TimePanel::ensure_registered_subscribers();
    let mut test_context = TestContext::new();

    test_context.send_time_commands(
        test_context.active_store_id(),
        [TimeControlCommand::SetActiveTimeline("frame_nr".into())],
    );

    add_sparse_data(&mut test_context);

    run_time_panel_and_save_snapshot(
        &test_context,
        TimePanel::default(),
        300.0,
        false,
        "time_panel_two_sections",
    );
}

#[test]
pub fn time_panel_two_sections_with_valid_range() {
    TimePanel::ensure_registered_subscribers();
    let mut test_context = TestContext::new();

    add_sparse_data(&mut test_context);

    test_context.send_time_commands(
        test_context.active_store_id(),
        [
            TimeControlCommand::SetActiveTimeline("frame_nr".into()),
            TimeControlCommand::AddValidTimeRange {
                timeline: Some("frame_nr".into()),
                time_range: AbsoluteTimeRange::new(
                    TimeInt::new_temporal(14),
                    TimeInt::new_temporal(102),
                ),
            },
        ],
    );

    run_time_panel_and_save_snapshot(
        &test_context,
        TimePanel::default(),
        300.0,
        false,
        "time_panel_two_sections_with_valid_range",
    );

    // Zoom out to check on "hidden" data.
    test_context.send_time_commands(
        test_context.active_store_id(),
        [TimeControlCommand::SetTimeView(TimeView {
            min: 8.into(),
            time_spanned: 20.0,
        })],
    );

    run_time_panel_and_save_snapshot(
        &test_context,
        TimePanel::default(),
        300.0,
        false,
        "time_panel_two_sections_with_valid_range_zoomed_out",
    );
}

#[test]
pub fn time_panel_two_sections_with_two_valid_ranges() {
    TimePanel::ensure_registered_subscribers();
    let mut test_context = TestContext::new();

    add_sparse_data(&mut test_context);

    test_context.send_time_commands(
        test_context.active_store_id(),
        [
            TimeControlCommand::SetActiveTimeline("frame_nr".into()),
            // Part of the first section.
            TimeControlCommand::AddValidTimeRange {
                timeline: Some("frame_nr".into()),
                time_range: AbsoluteTimeRange::new(
                    TimeInt::new_temporal(11),
                    TimeInt::new_temporal(13),
                ),
            },
            // Part of first + second section.
            TimeControlCommand::AddValidTimeRange {
                timeline: Some("frame_nr".into()),
                time_range: AbsoluteTimeRange::new(
                    TimeInt::new_temporal(15),
                    TimeInt::new_temporal(102),
                ),
            },
        ],
    );

    run_time_panel_and_save_snapshot(
        &test_context,
        TimePanel::default(),
        300.0,
        false,
        "time_panel_two_sections_with_two_valid_ranges",
    );

    // Zoom out to check on "hidden" data.
    test_context.send_time_commands(
        test_context.active_store_id(),
        [TimeControlCommand::SetTimeView(TimeView {
            min: 8.into(),
            time_spanned: 20.0,
        })],
    );

    run_time_panel_and_save_snapshot(
        &test_context,
        TimePanel::default(),
        300.0,
        false,
        "time_panel_two_sections_with_two_valid_ranges_zoomed_out",
    );
}

#[test]
pub fn time_panel_dense_data() {
    TimePanel::ensure_registered_subscribers();
    let mut test_context = TestContext::new();

    test_context.send_time_commands(
        test_context.active_store_id(),
        [TimeControlCommand::SetActiveTimeline("frame_nr".into())],
    );

    let points1 = MyPoint::from_iter(0..1);

    let mut rng_seed = 0b1010_1010_1010_1010_1010_1010_1010_1010u64;
    let mut rng = || {
        rng_seed ^= rng_seed >> 12;
        rng_seed ^= rng_seed << 25;
        rng_seed ^= rng_seed >> 27;
        rng_seed.wrapping_mul(0x2545_f491_4f6c_dd1d)
    };

    test_context.log_entity("/entity", |mut builder| {
        for frame in 0..1_000 {
            if rng() & 0b1 == 0 {
                continue;
            }

            builder = builder.with_sparse_component_batches(
                RowId::new(),
                [build_frame_nr(frame)],
                [(MyPoints::descriptor_points(), Some(&points1 as _))],
            );
        }

        builder
    });

    run_time_panel_and_save_snapshot(
        &test_context,
        TimePanel::default(),
        300.0,
        false,
        "time_panel_dense_data",
    );
}

// ---

#[test]
pub fn time_panel_filter_test_inactive() {
    run_time_panel_filter_tests(false, "", "time_panel_filter_test_inactive");
}

#[test]
pub fn time_panel_filter_test_active_no_query() {
    run_time_panel_filter_tests(true, "", "time_panel_filter_test_active_no_query");
}

#[test]
pub fn time_panel_filter_test_active_query() {
    run_time_panel_filter_tests(true, "ath", "time_panel_filter_test_active_query");
}

pub fn run_time_panel_filter_tests(filter_active: bool, query: &str, snapshot_name: &str) {
    TimePanel::ensure_registered_subscribers();
    let mut test_context = TestContext::new();

    test_context.send_time_commands(
        test_context.active_store_id(),
        [TimeControlCommand::SetActiveTimeline("frame_nr".into())],
    );

    let points1 = MyPoint::from_iter(0..1);
    for i in 0..2 {
        test_context.log_entity(format!("/entity/{i}"), |mut builder| {
            builder = builder.with_sparse_component_batches(
                RowId::new(),
                [build_frame_nr(1)],
                [(MyPoints::descriptor_points(), Some(&points1 as _))],
            );

            builder
        });
    }

    for i in 0..2 {
        test_context.log_entity(format!("/path/{i}"), |mut builder| {
            builder = builder.with_sparse_component_batches(
                RowId::new(),
                [build_frame_nr(1)],
                [(MyPoints::descriptor_points(), Some(&points1 as _))],
            );

            builder
        });
    }

    let mut time_panel = TimePanel::default();
    if filter_active {
        time_panel.activate_filter(query);
    }

    run_time_panel_and_save_snapshot(&test_context, time_panel, 300.0, false, snapshot_name);
}

// --

/// This test focuses on various kinds of entities and ensures their representation in the tree is
/// correct regardless of the selected timeline and current time.
//TODO(ab): we should also test what happens when GC kicks in.
#[test]
pub fn test_various_entity_kinds_in_time_panel() {
    TimePanel::ensure_registered_subscribers();

    for timeline in ["timeline_a", "timeline_b"] {
        for time in [0, 5, i64::MAX] {
            let mut test_context = TestContext::new();

            log_data_for_various_entity_kinds_tests(&mut test_context);

            test_context.send_time_commands(
                test_context.active_store_id(),
                [
                    TimeControlCommand::SetActiveTimeline(timeline.into()),
                    TimeControlCommand::SetTime(time.into()),
                    TimeControlCommand::SetTimeView(TimeView {
                        min: 0.into(),
                        time_spanned: 10.0,
                    }),
                ],
            );

            let time_panel = TimePanel::default();

            run_time_panel_and_save_snapshot(
                &test_context,
                time_panel,
                1200.0,
                true,
                &format!("various_entity_kinds_{timeline}_{time}"),
            );
        }
    }
}

#[test]
pub fn test_focused_item_is_focused() {
    TimePanel::ensure_registered_subscribers();

    let mut test_context = TestContext::new();

    test_context.send_time_commands(
        test_context.active_store_id(),
        [TimeControlCommand::SetActiveTimeline("timeline_a".into())],
    );

    log_data_for_various_entity_kinds_tests(&mut test_context);

    *test_context.focused_item.lock() =
        Some(EntityPath::from("/parent_with_data/of/entity").into());

    let time_panel = TimePanel::default();

    run_time_panel_and_save_snapshot(
        &test_context,
        time_panel,
        200.0,
        false,
        "focused_item_is_focused",
    );
}

pub fn log_data_for_various_entity_kinds_tests(test_context: &mut TestContext) {
    let timeline_a = "timeline_a";
    let timeline_b = "timeline_b";

    // just your average static entity
    log_static_data(test_context, "static_entity");

    // static data is over-logged multiple times
    for _ in 0..3 {
        log_static_data(test_context, "static_entity_multiple");
    }

    // static data overrides data logged on timeline a
    log_data(test_context, "static_overrides_temporal", timeline_a, 3);
    log_static_data(test_context, "static_overrides_temporal");

    // data in single timeline
    log_data(test_context, "timeline_a_only", timeline_a, 3);
    log_data(test_context, "timeline_b_only", timeline_b, 3);

    // data in both timelines
    log_data(test_context, "timeline_a_and_b", timeline_a, 2);
    log_data(test_context, "timeline_a_and_b", timeline_b, 5);

    // nested entity with parent empty
    log_data(test_context, "/empty/parent/of/entity", timeline_a, 3);

    // nested entity with data in a parent
    log_data(test_context, "/parent_with_data/of/entity", timeline_a, 3);
    log_data(test_context, "/parent_with_data", timeline_a, 1);

    // some entity with data logged "late" on the timeline
    log_data(test_context, "/late_data", timeline_a, 9);
    log_data(test_context, "/late_data", timeline_a, 10);
}

pub fn log_data(
    test_context: &mut TestContext,
    entity_path: impl Into<EntityPath>,
    timeline: &str,
    time: i64,
) {
    test_context.log_entity(entity_path.into(), |builder| {
        builder.with_archetype(
            RowId::new(),
            [(
                Timeline::new(timeline, TimeType::Sequence),
                TimeInt::try_from(time).expect("time must be valid"),
            )],
            &Points2D::new([[0.0, 0.0]]),
        )
    });
}

pub fn log_static_data(test_context: &mut TestContext, entity_path: impl Into<EntityPath>) {
    test_context.log_entity(entity_path.into(), |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &Points2D::new([[0.0, 0.0]]),
        )
    });
}

fn run_time_panel_and_save_snapshot(
    test_context: &TestContext,
    mut time_panel: TimePanel,
    height: f32,
    expand_all: bool,
    snapshot_name: &str,
) {
    let mut harness = test_context
        .setup_kittest_for_rendering()
        .with_size(Vec2::new(700.0, height))
        .build_ui(|ui| {
            test_context.run(&ui.ctx().clone(), |viewer_ctx| {
                if expand_all {
                    re_context_menu::collapse_expand::collapse_expand_instance_path(
                        viewer_ctx,
                        viewer_ctx.recording(),
                        &InstancePath::entity_all("/"),
                        CollapseScope::StreamsTree,
                        true,
                    );
                }

                let blueprint = ViewportBlueprint::from_db(
                    viewer_ctx.store_context.blueprint,
                    &LatestAtQuery::latest(blueprint_timeline()),
                );

                let mut time_commands = Vec::new();

                time_panel.show_expanded_with_header(
                    viewer_ctx,
                    viewer_ctx.time_ctrl,
                    &blueprint,
                    viewer_ctx.recording(),
                    ui,
                    &mut time_commands,
                );

                test_context.send_time_commands(viewer_ctx.store_id().clone(), time_commands);
            });

            test_context.handle_system_commands(ui.ctx());
        });

    harness.run();
    harness.snapshot_options(
        snapshot_name,
        &SnapshotOptions::default().failed_pixel_count_threshold(4),
    );
}
