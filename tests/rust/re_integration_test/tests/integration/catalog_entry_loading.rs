//! Which catalog RPCs the viewer needs to list datasets and open an entry.

use egui_kittest::kittest::Queryable as _;
use re_integration_test::{TestServer, ViewerHarnessExt as _};
use re_viewer::viewer_test_utils::{self, HarnessOptions};

#[tokio::test(flavor = "multi_thread")]
async fn open_dataset_after_listing_without_detail_rpcs() {
    let (server, segment) = TestServer::spawn().await.with_test_data().await;
    server.injected_errors().inject("ReadDatasetEntry");
    server.injected_errors().inject("GetSegmentTableSchema");
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        startup_url: Some(format!("rerun+http://localhost:{}", server.port())),
        ..Default::default()
    });
    viewer_test_utils::step_until("the dataset is listed", &mut harness, |harness| {
        harness.has_label("my_dataset")
    });

    server.injected_errors().clear_all();
    harness
        .get_all_by_label("my_dataset")
        .next()
        .expect("listed dataset")
        .click();
    viewer_test_utils::step_until(
        "the selected dataset's segment table is loaded",
        &mut harness,
        |harness| harness.has_label(segment.as_str()),
    );
}
