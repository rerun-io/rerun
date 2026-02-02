//! Snapshot test suite dedicated to snapshot the way we present various kinds of blueprint tree structures
//! with a focus on various view contents and filter configuration.

#![cfg(feature = "testing")]

use egui_kittest::SnapshotError;
use itertools::Itertools as _;
use re_blueprint_tree::BlueprintTree;
use re_blueprint_tree::data::BlueprintTreeData;
use re_chunk_store::RowId;
use re_chunk_store::external::re_chunk::ChunkBuilder;
use re_log_types::{EntityPath, build_frame_nr};
use re_sdk_types::archetypes::Points3D;
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_ui::filter_widget::FilterState;
use re_viewer_context::{RecommendedView, TimeControlCommand, ViewClass as _, ViewId};
use re_viewport_blueprint::{ViewBlueprint, ViewportBlueprint};

const VIEW_ID: &str = "this-is-a-view-id";

#[derive(Debug, Clone, Copy)]
enum RecordingKind {
    /// No entities are logged.
    Empty,

    /// Some placeholder entity hierarchy is logged.
    Regular,
}

struct TestCase {
    name: &'static str,

    origin: EntityPath,
    entity_filter: &'static str,

    recording_kind: RecordingKind,
}

fn base_test_cases() -> impl Iterator<Item = TestCase> {
    [
        TestCase {
            name: "empty",
            origin: EntityPath::root(),
            entity_filter: "$origin/**",
            recording_kind: RecordingKind::Empty,
        },
        TestCase {
            name: "root_origin",
            origin: EntityPath::root(),
            entity_filter: "$origin/**",
            recording_kind: RecordingKind::Regular,
        },
        TestCase {
            name: "non_root_origin",
            origin: EntityPath::from("/path/to"),
            entity_filter: "$origin/**",

            recording_kind: RecordingKind::Regular,
        },
        TestCase {
            name: "unknown_origin",
            origin: EntityPath::from("/wrong/path"),
            entity_filter: "$origin/**",
            recording_kind: RecordingKind::Regular,
        },
        TestCase {
            name: "single_proj",
            origin: EntityPath::from("/center/way"),
            entity_filter: "$origin/**\n/path/to/**",
            recording_kind: RecordingKind::Regular,
        },
        TestCase {
            name: "proj_with_placeholder",
            origin: EntityPath::from("/path/to"),
            entity_filter: "/**",
            recording_kind: RecordingKind::Regular,
        },
        TestCase {
            name: "multiple_proj",
            origin: EntityPath::from("/center/way"),
            entity_filter: "$origin/**\n/path/to/right\n/path/to/the/**",
            recording_kind: RecordingKind::Regular,
        },
    ]
    .into_iter()
}

fn filter_queries() -> impl Iterator<Item = Option<&'static str>> {
    [
        None,
        Some(""),
        Some("t"),
        Some("void"),
        Some("path"),
        Some("ath t"),
        Some("ath left"),
        Some("to/the"),
        Some("/to/the/"),
        Some("to/the oid"),
        Some("/path/to /rig"),
        Some("/path/to/th"),
    ]
    .into_iter()
}

fn test_context(test_case: &TestCase) -> TestContext {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    match test_case.recording_kind {
        RecordingKind::Empty => {}
        RecordingKind::Regular => {
            test_context.log_entity("/path/to/left", add_point_to_chunk_builder);
            test_context.log_entity("/path/to/right", add_point_to_chunk_builder);
            test_context.log_entity("/path/to/the/void", add_point_to_chunk_builder);
            test_context.log_entity("/path/onto/their/coils", add_point_to_chunk_builder);
            test_context.log_entity("/center/way", add_point_to_chunk_builder);
        }
    }

    test_context.setup_viewport_blueprint(|_, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_id(
            re_view_spatial::SpatialView3D::identifier(),
            RecommendedView {
                origin: test_case.origin.clone(),
                query_filter: test_case
                    .entity_filter
                    .parse()
                    .expect("invalid entity filter"),
            },
            ViewId::hashed_from_str(VIEW_ID),
        ));
    });

    test_context
}

fn add_point_to_chunk_builder(builder: ChunkBuilder) -> ChunkBuilder {
    builder.with_archetype(
        RowId::new(),
        [build_frame_nr(0)],
        &Points3D::new([[0.0, 0.0, 0.0]]),
    )
}

#[test]
fn test_all_snapshot_test_cases() {
    let errors = filter_queries()
        .flat_map(|filter_query| {
            base_test_cases()
                .map(move |test_case| (filter_query, run_test_case(&test_case, filter_query)))
        })
        .filter_map(|(filter_query, result)| result.err().map(|err| (filter_query, err)))
        .collect_vec();

    for (filter_query, error) in &errors {
        eprintln!("ERR: filter '{filter_query:?}': {error:?}");
    }

    assert!(errors.is_empty(), "Some test cases failed");
}

fn run_test_case(test_case: &TestCase, filter_query: Option<&str>) -> Result<(), SnapshotError> {
    let test_context = test_context(test_case);
    let view_id = ViewId::hashed_from_str(VIEW_ID);

    let mut blueprint_tree = BlueprintTree::default();

    // This trick here is to run the blueprint panel for a frame, such that it registers the current
    // application id. This way, the blueprint panel will not discard the filter state we set up
    // when it's run for the snapshot.
    test_context.run_in_egui_central_panel(|ctx, ui| {
        let blueprint =
            ViewportBlueprint::from_db(ctx.store_context.blueprint, ctx.blueprint_query);

        blueprint_tree.show(ctx, &blueprint, ui, &test_context.view_states.lock());
    });

    if let Some(filter_query) = filter_query {
        blueprint_tree.activate_filter(filter_query);
    }

    // set the current timeline to the timeline where data was logged to
    test_context.send_time_commands(
        test_context.active_store_id(),
        [TimeControlCommand::SetActiveTimeline("frame_nr".into())],
    );

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([400.0, 800.0])
        .build_ui(|ui| {
            test_context.run(&ui.ctx().clone(), |viewer_ctx| {
                re_context_menu::collapse_expand::collapse_expand_view(
                    viewer_ctx,
                    &view_id,
                    blueprint_tree.collapse_scope(),
                    true,
                );

                let blueprint = ViewportBlueprint::from_db(
                    viewer_ctx.store_context.blueprint,
                    viewer_ctx.blueprint_query,
                );

                blueprint_tree.show(viewer_ctx, &blueprint, ui, &test_context.view_states.lock());
            });

            test_context.handle_system_commands(ui.ctx());
        });

    harness.run();

    let options = re_ui::testing::default_snapshot_options_for_ui().output_path(format!(
        "tests/snapshots/view_structure_test/{}",
        filter_query
            .map(|query| format!("query-{}", query.replace(' ', ",").replace('/', "_")))
            .unwrap_or_else(|| "no-query".to_owned())
    ));

    harness.try_snapshot_options(test_case.name, &options)
}

// ---

#[test]
fn test_all_insta_test_cases() {
    for test_case in base_test_cases() {
        for filter_query in filter_queries() {
            let test_context = test_context(&test_case);

            let blueprint_tree_data =
                test_context.run_once_in_egui_central_panel(|viewer_ctx, _| {
                    let blueprint = ViewportBlueprint::from_db(
                        viewer_ctx.store_context.blueprint,
                        viewer_ctx.blueprint_query,
                    );

                    let mut filter_state = FilterState::default();

                    if let Some(filter_query) = filter_query {
                        filter_state.activate(filter_query);
                    }

                    BlueprintTreeData::from_blueprint_and_filter(
                        viewer_ctx,
                        &blueprint,
                        &filter_state.filter(),
                    )
                });

            let mut settings = insta::Settings::clone_current();
            settings.set_prepend_module_to_snapshot(false);
            settings.set_snapshot_path(format!(
                "snapshots/view_structure_test/{}",
                filter_query
                    .map(|query| format!("query-{}", query.replace(' ', ",").replace('/', "_")))
                    .unwrap_or_else(|| "no-query".to_owned())
            ));

            settings.bind(|| {
                insta::assert_yaml_snapshot!(test_case.name, blueprint_tree_data);
            });
        }
    }
}
