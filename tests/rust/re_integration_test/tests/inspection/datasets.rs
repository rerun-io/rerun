use std::str::FromStr as _;

use egui_kittest::SnapshotResults;
use egui_kittest::kittest::Queryable as _;
use re_integration_test::{HarnessConfig, InspectionHarness, TestServer, ViewerHarnessExt as _};
use re_sdk::{
    TimeCell, Timeline,
    external::{re_log_types::AbsoluteTimeRange, re_tuid},
};
use re_viewer::external::{re_chunk::TimelineName, re_viewer_context::open_url::ViewerOpenUrl};

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
        .get_by_role_and_label(egui::accesskit::Role::TextInput, "Address:")
        .click();
    harness.run();
    harness
        .get_by_role_and_label(egui::accesskit::Role::TextInput, "Address:")
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

    harness.step_until("Redap recording id appears", |harness| {
        harness
            .query_all_by_label_contains("new_recording_id")
            .next()
            .is_some()
    });
    harness.step_until_no_loading_indicator();

    snapshot_results.add(harness.try_snapshot("dataset_ui_table"));
}

#[tokio::test(flavor = "multi_thread")]
pub async fn start_with_dataset_url() {
    let (server, _) = TestServer::spawn().await.with_test_data().await;

    let mut harness = InspectionHarness::spawn(HarnessConfig {
        startup_url: Some(format!(
            "rerun+http://localhost:{}/entry/187b552b95a5c2f73f37894708825ba5",
            server.port()
        )),
        ..Default::default()
    });

    harness.step_until("Redap recording id appears", |harness| {
        harness
            .query_all_by_label_contains("new_recording_id")
            .next()
            .is_some()
    });
    harness.step_until_no_loading_indicator();

    let mut snapshot_results = SnapshotResults::new();
    snapshot_results.add(harness.try_snapshot("start_with_dataset_url"));
}

#[tokio::test(flavor = "multi_thread")]
pub async fn start_with_segment_fragment_url() {
    let (server, segment_id) = TestServer::spawn().await.with_test_data().await;

    let dataset_id =
        re_tuid::Tuid::from_str("187b552b95a5c2f73f37894708825ba5").expect("Failed to parse TUID");
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
    let url = ViewerOpenUrl::RedapDataset(segment_uri);

    let mut harness = InspectionHarness::spawn(HarnessConfig {
        startup_url: Some(url.sharable_url(None).expect("Should be a sharable url")),
        ..Default::default()
    });

    harness.step_until("Recording opened and source tree populated", |harness| {
        harness.query_by_label_contains("Streams").is_some()
            && !harness.is_loading()
            && harness.query_by_label_contains("my_dataset").is_some()
            && harness.query_all_by_label("new_recording_id").count() == 3
    });

    harness.set_selection_panel_opened(false);

    let mut snapshot_results = SnapshotResults::new();
    snapshot_results.add(harness.try_snapshot("start_with_segment_fragment_url"));
}
