#![cfg(feature = "testing")]
#![expect(clippy::unwrap_used)] // Fine for tests

use re_chunk::{Chunk, ChunkId};
use re_chunk_store::{LatestAtQuery, RowId};
use re_entity_db::InstancePath;
use re_log_encoding::RrdManifestBuilder;
use re_log_types::example_components::{MyPoint, MyPoints};
use re_log_types::external::re_tuid::Tuid;
use re_log_types::{EntityPath, StoreId, TimeInt, TimePoint, TimeType, Timeline, build_frame_nr};
use re_sdk_types::archetypes::Points2D;
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotResults;
use re_time_panel::TimePanel;
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

    let mut snapshot_results = SnapshotResults::new();
    run_time_panel_and_save_snapshot(
        &test_context,
        TimePanel::default(),
        300.0,
        false,
        "time_panel_two_sections",
        &mut snapshot_results,
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

    let mut snapshot_results = SnapshotResults::new();
    run_time_panel_and_save_snapshot(
        &test_context,
        TimePanel::default(),
        300.0,
        false,
        "time_panel_dense_data",
        &mut snapshot_results,
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

    let mut snapshot_results = SnapshotResults::new();
    run_time_panel_and_save_snapshot(
        &test_context,
        time_panel,
        300.0,
        false,
        snapshot_name,
        &mut snapshot_results,
    );
}

// --

/// This test focuses on various kinds of entities and ensures their representation in the tree is
/// correct regardless of the selected timeline and current time.
//TODO(ab): we should also test what happens when GC kicks in.
#[test]
pub fn test_various_entity_kinds_in_time_panel() {
    TimePanel::ensure_registered_subscribers();

    let mut snapshot_results = SnapshotResults::new();
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
                &mut snapshot_results,
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

    let mut snapshot_results = SnapshotResults::new();
    run_time_panel_and_save_snapshot(
        &test_context,
        time_panel,
        200.0,
        false,
        "focused_item_is_focused",
        &mut snapshot_results,
    );
}

#[test]
fn with_unloaded_chunks() {
    TimePanel::ensure_registered_subscribers();

    let mut test_context = TestContext::new();

    test_context.send_time_commands(
        test_context.active_store_id(),
        [TimeControlCommand::SetActiveTimeline("timeline_a".into())],
    );

    // Add manifest with unloaded chunks (chunks that exist in manifest but not in the store)
    let rrd_manifest = build_manifest_with_unloaded_chunks(test_context.active_store_id());
    test_context.add_rrd_manifest(rrd_manifest);

    // Also log some loaded data for comparison
    log_data_for_various_entity_kinds_tests(&mut test_context);

    let time_panel = TimePanel::default();

    let mut snapshot_results = SnapshotResults::new();
    run_time_panel_and_save_snapshot(
        &test_context,
        time_panel,
        450.0,
        false,
        "time_panel_unloaded_chunks",
        &mut snapshot_results,
    );
}

fn build_manifest_with_unloaded_chunks(store_id: StoreId) -> re_log_encoding::RrdManifest {
    let mut builder = RrdManifestBuilder::default();
    let mut byte_offset = 0u64;

    // Helper to generate sequential chunk IDs
    let mut next_chunk_id = {
        let mut chunk_id = ChunkId::from_tuid(Tuid::from_nanos_and_inc(999, 0));
        move || {
            chunk_id = chunk_id.next();
            chunk_id
        }
    };

    let mut next_row_id = {
        let mut row_id = RowId::from_tuid(Tuid::from_nanos_and_inc(999, 0));
        move || {
            row_id = row_id.next();
            row_id
        }
    };

    let timeline_a = Timeline::new("timeline_a", TimeType::Sequence);
    let timeline_b = Timeline::new("timeline_b", TimeType::Sequence);

    // Create chunks that will be in the manifest but NOT loaded into the store
    let unloaded_chunks = [
        Chunk::builder_with_id(next_chunk_id(), "/parent_with_data/of/unloaded1")
            .with_archetype(
                next_row_id(),
                [(timeline_a, TimeInt::new_temporal(2))],
                &Points2D::new([[0.0, 1.0]]),
            )
            .with_archetype(
                next_row_id(),
                [(timeline_a, TimeInt::new_temporal(10))],
                &Points2D::new([[1.0, 1.0]]),
            )
            .build()
            .unwrap(),
        Chunk::builder_with_id(next_chunk_id(), "/parent_with_data/of/unloaded2")
            .with_archetype(
                next_row_id(),
                [(timeline_a, TimeInt::new_temporal(4))],
                &Points2D::new([[0.0, 1.0]]),
            )
            .with_archetype(
                next_row_id(),
                [(timeline_a, TimeInt::new_temporal(6))],
                &Points2D::new([[1.0, 1.0]]),
            )
            .build()
            .unwrap(),
        Chunk::builder_with_id(next_chunk_id(), "/parent_with_data/of/unloaded3")
            .with_archetype(
                next_row_id(),
                [(timeline_a, TimeInt::new_temporal(5))],
                &Points2D::new([[0.0, 1.0]]),
            )
            .build()
            .unwrap(),
        Chunk::builder_with_id(next_chunk_id(), "/timeline_a_only")
            .with_archetype(
                next_row_id(),
                [(timeline_a, TimeInt::new_temporal(5))],
                &Points2D::new([[0.0, 1.0]]),
            )
            .with_archetype(
                next_row_id(),
                [(timeline_a, TimeInt::new_temporal(8))],
                &Points2D::new([[1.0, 1.0]]),
            )
            .build()
            .unwrap(),
        Chunk::builder_with_id(next_chunk_id(), "/timeline_b_only")
            .with_archetype(
                next_row_id(),
                [(timeline_b, TimeInt::new_temporal(5))],
                &Points2D::new([[0.0, 1.0]]),
            )
            .with_archetype(
                next_row_id(),
                [(timeline_b, TimeInt::new_temporal(8))],
                &Points2D::new([[1.0, 1.0]]),
            )
            .build()
            .unwrap(),
        Chunk::builder_with_id(next_chunk_id(), "/unloaded_entity")
            .with_archetype(
                next_row_id(),
                [(timeline_a, TimeInt::new_temporal(2))],
                &Points2D::new([[0.0, 1.0]; 10]),
            )
            .with_archetype(
                next_row_id(),
                [(timeline_a, TimeInt::new_temporal(3))],
                &Points2D::new([[1.0, 0.0]; 10]),
            )
            .build()
            .unwrap(),
        Chunk::builder_with_id(next_chunk_id(), "/unloaded_entity")
            .with_archetype(
                next_row_id(),
                [(timeline_a, TimeInt::new_temporal(5))],
                &Points2D::new([[0.0, 1.0]]),
            )
            .with_archetype(
                next_row_id(),
                [(timeline_a, TimeInt::new_temporal(6))],
                &Points2D::new([[1.0, 1.0]]),
            )
            .build()
            .unwrap(),
        Chunk::builder_with_id(next_chunk_id(), "/unloaded_entity")
            .with_archetype(
                next_row_id(),
                [(timeline_a, TimeInt::new_temporal(9))],
                &Points2D::new([[1.0, 2.0]]),
            )
            .with_archetype(
                next_row_id(),
                [(timeline_a, TimeInt::new_temporal(10))],
                &Points2D::new([[2.0, 2.0]]),
            )
            .build()
            .unwrap(),
    ];

    for chunk in &unloaded_chunks {
        let arrow_msg = chunk.to_arrow_msg().unwrap();
        let chunk_batch = re_sorbet::ChunkBatch::try_from(&arrow_msg.batch).unwrap();

        // Use mock byte sizes for testing (actual values only matter for file-based loading)
        let chunk_byte_size = 1000u64;
        let chunk_byte_size_uncompressed = 2000u64;

        let byte_span = re_span::Span {
            start: byte_offset,
            len: chunk_byte_size,
        };

        builder
            .append(&chunk_batch, byte_span, chunk_byte_size_uncompressed)
            .unwrap();

        byte_offset += chunk_byte_size;
    }

    builder.build(store_id).unwrap()
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
    snapshot_results: &mut SnapshotResults,
) {
    let mut harness = test_context
        .setup_kittest_for_rendering_ui([700.0, height])
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
    harness.snapshot(snapshot_name);

    snapshot_results.extend_harness(&mut harness);
}
