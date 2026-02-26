#![cfg(feature = "testing")]
#![expect(clippy::unwrap_used)] // Fine for tests

use re_chunk::Chunk;
use re_chunk_store::{LatestAtQuery, RowId};
use re_entity_db::InstancePath;
use re_log_types::example_components::{MyPoint, MyPoints};
use re_log_types::{EntityPath, TimeInt, TimePoint, TimeReal, TimeType, Timeline, build_frame_nr};
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

    add_sparse_data(&mut test_context);

    test_context.set_active_timeline("frame_nr");

    let mut snapshot_results = SnapshotResults::new();
    run_time_panel_and_save_snapshot(
        &test_context,
        TimePanel::default(),
        "time_panel_two_sections",
        &mut snapshot_results,
        &RunOptions {
            height: 300.0,
            expand_all: false,
            mark_chunks_used_or_missing: vec![],
        },
    );
}

#[test]
pub fn time_panel_dense_data() {
    TimePanel::ensure_registered_subscribers();
    let mut test_context = TestContext::new();

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

    test_context.set_active_timeline("frame_nr");

    let mut snapshot_results = SnapshotResults::new();
    run_time_panel_and_save_snapshot(
        &test_context,
        TimePanel::default(),
        "time_panel_dense_data",
        &mut snapshot_results,
        &RunOptions {
            height: 300.0,
            expand_all: false,
            mark_chunks_used_or_missing: vec![],
        },
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

    test_context.set_active_timeline("frame_nr");

    let mut time_panel = TimePanel::default();
    if filter_active {
        time_panel.activate_filter(query);
    }

    let mut snapshot_results = SnapshotResults::new();
    run_time_panel_and_save_snapshot(
        &test_context,
        time_panel,
        snapshot_name,
        &mut snapshot_results,
        &RunOptions {
            height: 300.0,
            expand_all: false,
            mark_chunks_used_or_missing: vec![],
        },
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
                &format!("various_entity_kinds_{timeline}_{time}"),
                &mut snapshot_results,
                &RunOptions {
                    height: 1200.0,
                    expand_all: true,
                    mark_chunks_used_or_missing: vec![],
                },
            );
        }
    }
}

#[test]
pub fn test_focused_item_is_focused() {
    TimePanel::ensure_registered_subscribers();

    let mut test_context = TestContext::new();

    log_data_for_various_entity_kinds_tests(&mut test_context);

    test_context.set_active_timeline("timeline_a");

    *test_context.focused_item.lock() =
        Some(EntityPath::from("/parent_with_data/of/entity").into());

    let time_panel = TimePanel::default();

    let mut snapshot_results = SnapshotResults::new();
    run_time_panel_and_save_snapshot(
        &test_context,
        time_panel,
        "focused_item_is_focused",
        &mut snapshot_results,
        &RunOptions {
            height: 200.0,
            expand_all: false,
            mark_chunks_used_or_missing: vec![],
        },
    );
}

#[test]
fn with_unloaded_chunks() {
    TimePanel::ensure_registered_subscribers();

    // Disable compaction so chunk IDs remain stable after insertion.
    let mut test_context = TestContext::new_with_store_info_and_config(
        re_log_types::StoreInfo::testing(),
        re_chunk_store::ChunkStoreConfig::COMPACTION_DISABLED,
    );

    let mut chunks = create_chunks();

    let rrd_manifest = re_log_encoding::RrdManifest::build_in_memory_from_chunks(
        test_context.active_store_id(),
        chunks.iter(),
    )
    .unwrap();

    test_context.add_rrd_manifest(rrd_manifest);

    test_context.set_active_timeline("timeline_a");

    let mut snapshot_results = SnapshotResults::new();

    let mut used_ids = chunks[..6].iter().map(|c| c.id()).collect::<Vec<_>>();

    run_time_panel_and_save_snapshot(
        &test_context,
        TimePanel::default(),
        "time_panel_only_unloaded_chunks",
        &mut snapshot_results,
        &RunOptions {
            height: 250.0,
            expand_all: false,
            mark_chunks_used_or_missing: used_ids.clone(),
        },
    );

    // Load some chunks in the list.
    test_context.add_chunks(chunks.drain(..6));

    run_time_panel_and_save_snapshot(
        &test_context,
        TimePanel::default(),
        "time_panel_partially_unloaded_chunks",
        &mut snapshot_results,
        &RunOptions {
            height: 250.0,
            expand_all: false,
            // Should now be loaded.
            mark_chunks_used_or_missing: used_ids.clone(),
        },
    );

    test_context.send_time_commands(
        test_context.active_store_id(),
        [TimeControlCommand::SetTime(TimeReal::from(5))],
    );

    used_ids.push(chunks[0].id());

    run_time_panel_and_save_snapshot(
        &test_context,
        TimePanel::default(),
        "time_panel_loading_unloaded_chunks",
        &mut snapshot_results,
        &RunOptions {
            height: 250.0,
            expand_all: false,
            mark_chunks_used_or_missing: used_ids,
        },
    );
}

fn create_chunk(
    entity_path: impl Into<EntityPath>,
    timeline: Timeline,
    row_times: impl IntoIterator<Item = i64>,
) -> Chunk {
    let mut builder = Chunk::builder(entity_path);

    for time in row_times {
        builder = builder.with_archetype(
            RowId::new(),
            [(
                timeline,
                TimeInt::try_from(time).expect("time must be valid"),
            )],
            &Points2D::new([[0.0, 0.0]]),
        );
    }

    builder.build().unwrap()
}

fn create_chunks() -> Vec<Chunk> {
    let timeline_a = Timeline::new("timeline_a", TimeType::Sequence);
    let timeline_b = Timeline::new("timeline_b", TimeType::Sequence);

    vec![
        // will be loaded
        create_chunk("/parent_with_data/of/unloaded0", timeline_a, [0]),
        create_chunk("/some_entity", timeline_a, [1, 2]),
        create_chunk("/parent_with_data/of/unloaded1", timeline_a, 2..=10),
        create_chunk("/parent_with_data/of/unloaded2", timeline_a, [5]),
        create_chunk("/timeline_a_only", timeline_a, [5, 8]),
        create_chunk("/some_entity", timeline_a, [5, 6]),
        // will stay unloaded
        create_chunk("/parent_with_data/of/unloaded1", timeline_a, 4..=6),
        create_chunk("/parent_with_data/of/unloaded2", timeline_a, [10]),
        create_chunk("/timeline_b_only", timeline_b, [5, 8]),
        create_chunk("/some_entity", timeline_a, [9, 10]),
    ]
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
    let entity_path = entity_path.into();
    test_context.log_entity(entity_path, |builder| {
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

#[derive(Clone)]
struct RunOptions {
    height: f32,
    expand_all: bool,

    /// Marks the given chunks as missing, to cause the time-panel to
    /// indicate that something is loading. This has to be a chunk coming
    /// with a root in the rrd manifest.
    mark_chunks_used_or_missing: Vec<re_chunk::ChunkId>,
}

fn run_time_panel_and_save_snapshot(
    test_context: &TestContext,
    mut time_panel: TimePanel,
    snapshot_name: &str,
    snapshot_results: &mut SnapshotResults,
    options: &RunOptions,
) {
    let mut harness = test_context
        .setup_kittest_for_rendering_ui([700.0, options.height])
        .build_ui(|ui| {
            test_context.run(&ui.ctx().clone(), |viewer_ctx| {
                for chunk_id in &options.mark_chunks_used_or_missing {
                    viewer_ctx
                        .recording()
                        .storage_engine()
                        .store()
                        .use_physical_chunk_or_report_missing(chunk_id);
                }

                if options.expand_all {
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
