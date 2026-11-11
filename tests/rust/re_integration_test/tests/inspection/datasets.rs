use std::str::FromStr as _;

use egui_kittest::SnapshotResults;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use re_integration_test::{
    HarnessConfig, InspectionHarness, TEST_DATASET_ID, TestServer, ViewerHarnessExt as _,
};
use re_sdk::{
    TimeCell, Timeline,
    external::{re_log_types::AbsoluteTimeRange, re_tuid},
};
use re_viewer::external::{re_chunk::TimelineName, re_viewer_context::viewer_url::ViewerUrl};

#[tokio::test(flavor = "multi_thread")]
pub async fn dataset_ui_test() {
    let (server, _) = TestServer::spawn().await.with_test_data().await;

    let mut harness = InspectionHarness::spawn(HarnessConfig::default());
    let mut snapshot_results = SnapshotResults::new();

    harness.set_blueprint_panel_opened(true);
    harness.set_selection_panel_opened(false);
    harness.set_time_panel_opened(false);

    harness.get_by_label("Add…").click();
    harness.run();
    harness.get_by_label_contains("Connect to a server").click();
    harness.run();

    snapshot_results.add(harness.try_snapshot("dataset_ui_empty_form"));

    harness
        .get_by_role_and_label(egui::Role::TextInput, "Address")
        .click();
    harness.run();
    harness
        .get_by_role_and_label(egui::Role::TextInput, "Address")
        .type_text(&format!("rerun+http://localhost:{}", server.port()));
    harness.run();

    harness.get_by_label("No authentication").click();
    harness.run();

    harness.get_by_label("Add").click();
    harness.run_ok();

    // Wait for both datasets to appear.
    harness.step_until("Redap server datasets appear", |harness| {
        harness.query_all_by_label_contains("my_dataset").count() == 2
    });

    // Click the dataset (pick the first match, which is in the left panel).
    harness
        .get_all_by_label("my_dataset")
        .next()
        .expect("my_dataset label should be present")
        .click();

    harness.step_until("Dataset recording loaded", |harness| {
        harness
            .query_all_by_label_contains("new_recording_id")
            .next()
            .is_some()
            && !harness.is_loading()
    });

    snapshot_results.add(harness.try_snapshot("dataset_ui_table"));
}

#[tokio::test(flavor = "multi_thread")]
pub async fn start_with_dataset_url() {
    let (server, _) = TestServer::spawn().await.with_test_data().await;

    let mut harness = InspectionHarness::spawn(HarnessConfig {
        startup_url: Some(format!(
            "rerun+http://localhost:{}/entry/{TEST_DATASET_ID}",
            server.port()
        )),
        ..Default::default()
    });

    harness.step_until("Dataset recording loaded", |harness| {
        harness
            .query_all_by_label_contains("new_recording_id")
            .next()
            .is_some()
            && !harness.is_loading()
    });

    let back_button = harness
        .query_by_label("go back")
        .expect("The navigation bar should contain a back button");
    assert!(
        back_button.accesskit_node().is_disabled(),
        "A startup URL should be the first history entry"
    );

    let mut snapshot_results = SnapshotResults::new();
    snapshot_results.add(harness.try_snapshot("start_with_dataset_url"));
}

#[tokio::test(flavor = "multi_thread")]
async fn startup_segment_url_expands_ancestor_folders() {
    let (server, segment_id) = TestServer::spawn()
        .await
        .with_named_test_data(
            "parent.child.my_dataset",
            TEST_DATASET_ID,
            "new_recording_id",
        )
        .await;

    let mut harness = InspectionHarness::spawn(HarnessConfig {
        startup_url: Some(format!(
            "rerun+http://localhost:{}/dataset/{TEST_DATASET_ID}?segment_id={segment_id}",
            server.port()
        )),
        ..Default::default()
    });

    assert_segment_revealed_in_sources(&mut harness);
}

#[tokio::test(flavor = "multi_thread")]
async fn command_palette_segment_url_expands_ancestor_folders() {
    // TODO(RR-6358): in the browser, the segment never downloads its chunks.
    if InspectionHarness::is_browser() {
        return;
    }

    let (server, segment_id) = TestServer::spawn()
        .await
        .with_named_test_data(
            "parent.child.my_dataset",
            TEST_DATASET_ID,
            "new_recording_id",
        )
        .await;
    let mut harness = InspectionHarness::spawn(HarnessConfig::default());

    harness.get_by_label_contains("Search commands…").click();
    harness.run();
    harness
        .get_by_role(egui::Role::TextInput)
        .type_text(&format!(
            "rerun+http://localhost:{}/dataset/{TEST_DATASET_ID}?segment_id={segment_id}",
            server.port()
        ));
    harness.run();
    for pressed in [true, false] {
        harness.queue_event(egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
    }

    assert_segment_revealed_in_sources(&mut harness);
}

/// Chevron collapses must persist while the selection remains unchanged.
#[tokio::test(flavor = "multi_thread")]
async fn source_chevrons_preserve_manual_collapse() {
    let (server, segment_id) = TestServer::spawn()
        .await
        .with_named_test_data(
            "parent.child.my_dataset",
            TEST_DATASET_ID,
            "new_recording_id",
        )
        .await;
    let mut harness = InspectionHarness::spawn(HarnessConfig {
        startup_url: Some(format!(
            "rerun+http://localhost:{}/dataset/{TEST_DATASET_ID}?segment_id={segment_id}",
            server.port()
        )),
        ..Default::default()
    });
    assert_segment_revealed_in_sources(&mut harness);
    // Finishing loads change the selection, which re-expands collapsed ancestors.
    harness.step_until("Segment finishes loading", |harness| !harness.is_loading());

    for (ancestor, child) in [
        ("my_dataset", "new_recording_id"),
        ("child", "my_dataset"),
        ("parent", "child"),
        ("localhost:", "parent"),
    ] {
        collapse_source_and_check(&mut harness, ancestor, child);
    }

    harness
        .section_node(Some("_recording_panel"))
        .get_by_label_contains("localhost:")
        .click();
    harness.step_until("Selecting the server reveals its entries", |harness| {
        harness
            .section_node(Some("_recording_panel"))
            .query_by_label("parent")
            .is_some()
    });
    assert_eq!(
        harness
            .section_node(Some("_recording_panel"))
            .get_by_label_contains("localhost:")
            .accesskit_node()
            .toggled(),
        Some(egui::accesskit::Toggled::True)
    );
    collapse_source_and_check(&mut harness, "localhost:", "parent");
}

fn collapse_source_and_check(harness: &mut InspectionHarness, ancestor: &str, child: &str) {
    harness
        .section_node(Some("_recording_panel"))
        .get_by_label_contains(ancestor)
        .parent()
        .expect("Source items have a row containing their chevron")
        .get_by_role(egui::Role::DisclosureTriangle)
        .click();
    harness.step_until("Chevron collapses the source item", |harness| {
        harness
            .section_node(Some("_recording_panel"))
            .query_by_label_contains(child)
            .is_none()
    });

    harness.run_ok();
    assert!(
        harness
            .section_node(Some("_recording_panel"))
            .query_by_label_contains(child)
            .is_none(),
        "{ancestor} must stay collapsed after the chevron click"
    );
}

fn assert_segment_revealed_in_sources(harness: &mut InspectionHarness) {
    harness.step_until(
        "Segment and its ancestor folders appear in sources",
        |harness| {
            if harness.query_by_label_contains("STREAMS").is_none() {
                return false;
            }
            // Make sure all expected segments and folders are present in the sources:
            let sources = harness.section_node(Some("_recording_panel"));
            sources.query_by_label("parent").is_some()
                && sources.query_by_label("child").is_some()
                && sources.query_by_label("my_dataset").is_some()
                && sources
                    .query_by_label_contains("new_recording_id")
                    .is_some()
        },
    );
    harness.run_ok();
}

#[tokio::test(flavor = "multi_thread")]
pub async fn start_with_segment_fragment_url() {
    let (server, segment_id) = TestServer::spawn().await.with_test_data().await;

    let dataset_id = re_tuid::Tuid::from_str(TEST_DATASET_ID).expect("Failed to parse TUID");
    let segment_uri = re_uri::DatasetUri {
        origin: re_uri::Origin {
            scheme: re_uri::Scheme::RerunHttp,
            host: re_uri::external::url::Host::Domain("localhost".to_owned()),
            port: server.port(),
        },
        dataset_id,
        resource: re_uri::DatasetResource::Segments,
        segment_id: Some(segment_id),
        fragment: re_uri::Fragment {
            selection: None,
            when: Some((
                TimelineName::from("test_time"),
                TimeCell::new(re_sdk::time::TimeType::Sequence, 10),
            )),
            time_selection: Some(re_uri::TimeSelection {
                timeline: Timeline::new_sequence("test_time"),
                range: AbsoluteTimeRange::new(2, 8),
            }),
        },
    };
    let url = ViewerUrl::RedapDataset(segment_uri);

    let mut harness = InspectionHarness::spawn(HarnessConfig {
        startup_url: Some(url.sharable_url(None).expect("Should be a sharable url")),
        ..Default::default()
    });

    harness.step_until("Recording opened and source tree populated", |harness| {
        harness.query_by_label_contains("STREAMS").is_some()
            && !harness.is_loading()
            && harness.query_by_label_contains("my_dataset").is_some()
            && harness.query_all_by_label("new_recording_id").count() == 3
    });

    harness.set_selection_panel_opened(false);

    let mut snapshot_results = SnapshotResults::new();
    snapshot_results.add(harness.try_snapshot("start_with_segment_fragment_url"));
}
