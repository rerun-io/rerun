use re_chunk::Chunk;
use re_log_types::{TimeInt, Timeline};
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::TextLog;
use re_sdk_types::blueprint::archetypes::TextLogRows;
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotResults;
use re_test_viewport::TestContextExt as _;
use re_view_text_log::TextView;
use re_viewer_context::{BlueprintContext as _, TimeControlCommand, ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

fn setup_blueprint(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(TextView::identifier()))
    })
}

#[test]
fn temporal_anchor_between_sequence_steps() {
    let mut snapshot_results = SnapshotResults::new();
    let mut test_context = TestContext::new_with_view_class::<TextView>();

    let timeline = Timeline::log_tick();

    let chunks = &mut text_log_chunks(timeline);

    // Only add the first one (tick = 0)
    test_context.add_chunks(chunks.take(1));
    test_context.set_active_timeline(*timeline.name());

    // The temporal anchor is intentionally between two sequence steps.
    test_context.send_time_commands(
        test_context.active_store_id(),
        [TimeControlCommand::SetTime(
            TimeInt::new_temporal(100).into(),
        )],
    );
    test_context.handle_system_commands(&egui::Context::default());

    let view_id = setup_blueprint(&mut test_context);
    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        "text_log_temporal_anchor_between_steps_first_chunk",
        egui::vec2(500.0, 180.0),
        None,
    ));

    // Add the rest
    test_context.add_chunks(chunks);
    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        "text_log_temporal_anchor_between_steps_rest",
        egui::vec2(500.0, 180.0),
        None,
    ));
}

/// With a level filter active, only matching rows (and rows without a level) are shown,
/// laid out from cached per-chunk level counts.
#[test]
fn level_filter() {
    let mut snapshot_results = SnapshotResults::new();
    let mut test_context = TestContext::new_with_view_class::<TextView>();

    let timeline = Timeline::log_tick();

    let mut builder = Chunk::builder("logs");
    for tick in 0_i64..30 {
        let mut row = TextLog::new(format!("Log at tick {tick}"));
        // Every third row has no level: those always pass the filter.
        if tick % 3 != 2 {
            row = row.with_level(if tick % 2 == 0 { "INFO" } else { "WARN" });
        }
        builder = builder.with_archetype_auto_row([(timeline, tick)], &row);
    }
    test_context.add_chunks(std::iter::once(
        builder.build().expect("failed to build chunk"),
    ));

    test_context.set_active_timeline(*timeline.name());
    test_context.send_time_commands(
        test_context.active_store_id(),
        [TimeControlCommand::SetTime(
            TimeInt::new_temporal(15).into(),
        )],
    );
    test_context.handle_system_commands(&egui::Context::default());

    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view_id = blueprint
            .add_view_at_root(ViewBlueprint::new_with_root_wildcard(TextView::identifier()));

        let property_path = {
            let engine = ctx.store_context.blueprint.storage_engine();
            re_viewport_blueprint::entity_path_for_view_property(
                view_id,
                engine.store().entity_tree(),
                TextLogRows::name(),
            )
        };
        ctx.save_blueprint_archetype(
            property_path,
            &TextLogRows::new().with_filter_by_log_level(["WARN"]),
        );

        view_id
    });

    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        "text_log_level_filter",
        egui::vec2(500.0, 420.0),
        None,
    ));
}

fn text_log_chunks(timeline: Timeline) -> impl Iterator<Item = Chunk> {
    (0_i64..=200).step_by(10).map(move |tick| {
        Chunk::builder("logs")
            .with_archetype_auto_row(
                [(timeline, tick)],
                &TextLog::new(format!("Log at tick {tick}")),
            )
            .build()
            .expect("failed to build chunk")
    })
}
