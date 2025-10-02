use std::sync::Arc;
use std::time::Duration;

use egui_kittest::Harness;
use egui_kittest::{SnapshotResults, kittest::Queryable as _};

use re_integration_test::TestServer;
use re_sdk::external::re_log_types::{SetStoreInfo, StoreInfo};
use re_sdk::external::re_tuid::Tuid;
use re_sdk::log::{Chunk, RowId};
use re_sdk::{
    Component, ComponentDescriptor, EntityPath, EntityPathPart, RecordingInfo, StoreId, StoreKind,
    TimePoint,
};
use re_view_text_document::TextDocumentView;
use re_viewer::external::re_chunk::{ChunkBuilder, LatestAtQuery};
use re_viewer::external::re_entity_db::EntityDb;
use re_viewer::external::re_viewer_context::{
    Item, RecommendedView, ViewClass, ViewId, ViewerContext, blueprint_timeline,
};
use re_viewer::external::{re_types, re_viewer_context};
use re_viewer::viewer_test_utils::AppTestingExt as _;
use re_viewer::{App, SystemCommand, SystemCommandSender as _, viewer_test_utils};
use re_viewport_blueprint::{ViewBlueprint, ViewportBlueprint};

#[tokio::test(flavor = "multi_thread")]
pub async fn xfoo_test() {
    let server = TestServer::spawn().await.with_test_data().await;

    let mut harness = viewer_test_utils::viewer_harness();
    let mut snapshot_results = SnapshotResults::new();

    harness.run_ok();
    snapshot_results.add(harness.try_snapshot("xfoo_0"));

    harness.get_by_label("Blueprint panel toggle").click();
    harness.run_ok();
    harness.get_by_label("Time panel toggle").click();
    harness.run_ok();
    harness.get_by_label("Selection panel toggle").click();
    harness.run_ok();

    harness.run_ok();
    snapshot_results.add(harness.try_snapshot("xfoo_1"));

    let app = harness.state_mut();
    let k = app.testonly_state_mut();

    let store_hub = app.testonly_get_store_hub();

    let store_info = StoreInfo::testing();
    let application_id = store_info.application_id().clone();
    let recording_store_id = store_info.store_id.clone();
    let mut recording_store = EntityDb::new(recording_store_id.clone());

    recording_store.set_store_info(SetStoreInfo {
        row_id: Tuid::new(),
        info: store_info,
    });
    {
        // Set RecordingInfo:
        recording_store
            .set_recording_property(
                EntityPath::properties(),
                RecordingInfo::descriptor_name(),
                &re_types::components::Name::from("Test recording"),
            )
            .unwrap();
        recording_store
            .set_recording_property(
                EntityPath::properties(),
                RecordingInfo::descriptor_start_time(),
                &re_types::components::Timestamp::now(),
            )
            .unwrap();
    }
    {
        // Set some custom recording properties:
        recording_store
            .set_recording_property(
                EntityPath::properties() / EntityPathPart::from("episode"),
                ComponentDescriptor {
                    archetype: None,
                    component: "location".into(),
                    component_type: Some(re_types::components::Text::name()),
                },
                &re_types::components::Text::from("Swallow Falls"),
            )
            .unwrap();
        recording_store
            .set_recording_property(
                EntityPath::properties() / EntityPathPart::from("episode"),
                ComponentDescriptor {
                    archetype: None,
                    component: "weather".into(),
                    component_type: Some(re_types::components::Text::name()),
                },
                &re_types::components::Text::from("Cloudy with meatballs"),
            )
            .unwrap();
    }

    let blueprint_id = StoreId::random(StoreKind::Blueprint, application_id);
    let blueprint_store = EntityDb::new(blueprint_id.clone());

    store_hub.insert_entity_db(recording_store);
    store_hub.insert_entity_db(blueprint_store);
    store_hub.set_active_recording_id(recording_store_id.clone());
    store_hub
        .set_cloned_blueprint_active_for_app(&blueprint_id)
        .expect("Failed to set blueprint as active");

    println!("Active recording: {:?}", store_hub.active_recording());
    println!("Active blueprint: {:?}", store_hub.active_blueprint());

    app.command_sender.send_system(SystemCommand::SetSelection(
        re_viewer_context::Item::StoreId(recording_store_id.clone()).into(),
    ));
    harness.run_ok();

    setup_viewport_blueprint(&mut harness, |_viewer_context, blueprint| {
        println!("Blueprint view count: {}", blueprint.views.len());
        for id in blueprint.view_ids() {
            println!("View id: {id}");
        }
        println!(
            "Display mode: {:?}",
            _viewer_context.global_context.display_mode
        );
    });

    // app.testonly_state_mut()
    //     .navigation
    //     .replace(DisplayMode::LocalRecordings(recording_store_id));

    // store_hub.set_active_recording_id(StoreId::new(
    //     StoreKind::Recording,
    //     "test_app",
    //     "test_recording",
    // ));
    // println!("Active recording: {:?}", store_hub.active_recording());

    // app.testonly_init_recording();

    // log_entity(&mut harness, "time_series", |builder| {
    //     builder.with_archetype(
    //         RowId::new(),
    //         TimePoint::STATIC,
    //         &re_types::archetypes::Scalars::single(1.0),
    //     )
    // });

    log_entity(&mut harness, "txt/one", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_types::archetypes::TextDocument::new("one"),
        )
    });

    setup_viewport_blueprint(&mut harness, |_viewer_context, blueprint| {
        println!("Blueprint view count: {}", blueprint.views.len());
        for id in blueprint.view_ids() {
            println!("View id: {id}");
        }
    });

    setup_viewport_blueprint(&mut harness, |_viewer_context, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            TextDocumentView::identifier(),
        ));
    });

    setup_viewport_blueprint(&mut harness, |_viewer_context, blueprint| {
        println!("Blueprint view count: {}", blueprint.views.len());
        for id in blueprint.view_ids() {
            println!("View id: {id}");
        }
        println!(
            "Display mode: {:?}",
            _viewer_context.global_context.display_mode
        );
    });

    println!(
        "Active blueprint: {:?}",
        harness
            .state_mut()
            .testonly_get_store_hub()
            .active_blueprint()
    );

    harness.run_ok();
    harness.run_ok();
    harness.run_ok();
    snapshot_results.add(harness.try_snapshot("xfoo"));
}

fn run_with_viewer_context(
    harness: &mut Harness<'_, App>,
    func: impl FnOnce(&ViewerContext<'_>) + 'static,
) {
    harness
        .state_mut()
        .testonly_set_test_hook(Some(Box::new(func)));
    harness.run_ok();
    harness.state_mut().testonly_set_test_hook(None);
}

fn log_entity(
    harness: &mut Harness<'_, App>,
    entity_path: impl Into<EntityPath>,
    build_chunk: impl FnOnce(ChunkBuilder) -> ChunkBuilder,
) {
    let app = harness.state_mut();
    let builder = build_chunk(Chunk::builder(entity_path));
    let store_hub = app.testonly_get_store_hub();
    let active_recording = store_hub
        .active_recording_mut()
        .expect("active_recording should be initialized");
    active_recording
        .add_chunk(&Arc::new(
            builder.build().expect("chunk should be successfully built"),
        ))
        .expect("chunk should be successfully added");
}

fn setup_viewport_blueprint(
    harness: &mut Harness<'_, App>,
    setup_blueprint: impl FnOnce(&ViewerContext<'_>, &mut ViewportBlueprint) + 'static,
) {
    run_with_viewer_context(harness, |viewer_context| {
        let blueprint_query = LatestAtQuery::latest(blueprint_timeline());
        let mut viewport_blueprint =
            ViewportBlueprint::from_db(viewer_context.blueprint_db(), &blueprint_query);
        setup_blueprint(viewer_context, &mut viewport_blueprint);
        viewport_blueprint.save_to_blueprint_store(viewer_context);
    });
}
