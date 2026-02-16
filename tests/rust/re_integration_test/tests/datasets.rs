use std::{str::FromStr as _, time::Duration};

use egui_kittest::SnapshotResults;
use egui_kittest::kittest::Queryable as _;
use re_integration_test::{HarnessExt as _, TestServer};
use re_sdk::{
    TimeCell, Timeline,
    external::{re_log_types::AbsoluteTimeRange, re_tuid},
};
use re_viewer::{
    external::{re_chunk::TimelineName, re_viewer_context::open_url::ViewerOpenUrl},
    viewer_test_utils::{self, HarnessOptions},
};

#[tokio::test(flavor = "multi_thread")]
pub async fn dataset_ui_test() {
    let (server, _) = TestServer::spawn().await.with_test_data().await;

    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions::default());
    let mut snapshot_results = SnapshotResults::new();

    harness.set_blueprint_panel_opened(true);
    harness.set_selection_panel_opened(false);
    harness.set_time_panel_opened(false);

    harness.get_by_label("Add…").click();
    harness.run_ok();
    harness.get_by_label_contains("Connect to a server").click();
    harness.run_ok();

    snapshot_results.add(harness.try_snapshot("dataset_ui_empty_form"));

    harness
        .get_by_role_and_label(egui::accesskit::Role::TextInput, "Address:")
        .click();
    harness.run_ok();
    harness
        .get_by_role_and_label(egui::accesskit::Role::TextInput, "Address:")
        .type_text(&format!("rerun+http://localhost:{}", server.port()));
    harness.run_ok();

    harness.get_by_label("No authentication").click();
    harness.run_ok();

    harness.get_by_label("Add").click();
    harness.run_ok();

    viewer_test_utils::step_until(
        "Redap server dataset appears",
        &mut harness,
        // The label eventually appears twice: first in the left panel, and in the entries table
        // when it refreshes. Here we wait for both to appear. Later we pick the first one (in the
        // left panel).
        |harness| harness.query_all_by_label_contains("my_dataset").count() == 2,
        Duration::from_millis(100),
        Duration::from_secs(5),
    );

    // We pick the first one.
    harness
        .get_all_by_label("my_dataset")
        .next()
        .unwrap()
        .click();

    viewer_test_utils::step_until(
        "Redap recording id appears",
        &mut harness,
        |harness| {
            harness
                .query_by_label_contains("new_recording_id")
                .is_some()
        },
        Duration::from_millis(100),
        Duration::from_secs(5),
    );
    snapshot_results.add(harness.try_snapshot("dataset_ui_table"));
}

#[tokio::test(flavor = "multi_thread")]
pub async fn start_with_dataset_url() {
    let (server, _) = TestServer::spawn().await.with_test_data().await;

    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        startup_url: Some(format!(
            "rerun+http://localhost:{}/entry/187b552b95a5c2f73f37894708825ba5",
            server.port()
        )),
        ..Default::default()
    });

    viewer_test_utils::step_until(
        "Redap recording id appears",
        &mut harness,
        |harness| {
            harness
                .query_by_label_contains("new_recording_id")
                .is_some()
        },
        Duration::from_millis(100),
        Duration::from_secs(5),
    );
    harness.snapshot("start_with_dataset_url");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn start_with_segment_fragment_url() {
    let (server, segment_id) = TestServer::spawn().await.with_test_data().await;

    let dataset_id =
        re_tuid::Tuid::from_str("187b552b95a5c2f73f37894708825ba5").expect("Failed to parse TUID");
    let url = ViewerOpenUrl::RedapDatasetSegment(re_uri::DatasetSegmentUri {
        origin: re_uri::Origin {
            scheme: re_uri::Scheme::RerunHttp,
            host: re_uri::external::url::Host::Domain("localhost".to_owned()),
            port: server.port(),
        },
        dataset_id,
        segment_id: segment_id.id().to_owned(),
        fragment: re_uri::Fragment {
            selection: None,
            when: Some((
                TimelineName::new("test_time"),
                TimeCell::new(re_sdk::time::TimeType::Sequence, 10),
            )),
            time_selection: Some(re_uri::TimeSelection {
                timeline: Timeline::new_sequence("test_time"),
                range: AbsoluteTimeRange::new(2, 8),
            }),
        },
    });

    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        startup_url: Some(url.sharable_url(None).expect("Should be a sharable url")),
        ..Default::default()
    });

    viewer_test_utils::step_until(
        "Redap recording id appears",
        &mut harness,
        |harness| {
            harness.query_by_label_contains("Streams").is_some()
                && harness.query_by_label("Loading etries…").is_none()
        },
        Duration::from_millis(100),
        Duration::from_secs(5),
    );

    harness.set_selection_panel_opened(false);

    // Redact the loading bar, since it is racy
    harness.mask(egui::Rect::from_x_y_ranges(
        egui::Rangef::new(190.0, 1024.0),
        egui::Rangef::new(604.0, 606.0),
    ));

    // Redact timeline data because we have no way to consistently wait for it to arrive
    harness.mask(egui::Rect::from_x_y_ranges(
        egui::Rangef::new(190.0, 1024.0),
        egui::Rangef::new(650.0, 690.0),
    ));

    harness.snapshot("start_with_segment_fragment_url");
}
