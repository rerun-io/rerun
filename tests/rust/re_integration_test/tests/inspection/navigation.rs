use std::io::Write as _;
use std::net::TcpListener;
use std::sync::mpsc;

use egui_kittest::SnapshotResults;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use re_integration_test::{HarnessConfig, InspectionHarness, ViewerHarnessExt as _};

struct DelayedFailingHttpUrl {
    url: String,
    fail_request: mpsc::SyncSender<()>,
}

fn delayed_failing_http_url() -> DelayedFailingHttpUrl {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind test HTTP server");
    let address = listener
        .local_addr()
        .expect("Failed to read test HTTP server address");
    let (fail_tx, fail_rx) = mpsc::sync_channel(1);

    std::thread::Builder::new()
        .name("delayed_failing_http_server".to_owned())
        .spawn(move || {
            let (mut stream, _) = listener.accept().expect("Failed to accept HTTP request");
            fail_rx
                .recv()
                .expect("Test ended before releasing the HTTP response");
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\n\
                      Access-Control-Allow-Origin: *\r\n\
                      Content-Length: 3\r\n\
                      Connection: close\r\n\r\n\
                      bad",
                )
                .expect("Failed to write HTTP response");
        })
        .expect("Failed to spawn test HTTP server");

    DelayedFailingHttpUrl {
        url: format!("http://{address}/recording.rrd"),
        fail_request: fail_tx,
    }
}

#[tokio::test(flavor = "multi_thread")]
pub async fn rejected_startup_url_does_not_create_history() {
    let mut harness = InspectionHarness::spawn(HarnessConfig {
        startup_url: Some("does not parse".to_owned()),
        ..Default::default()
    });

    harness.step_until("Welcome screen appears", |harness| {
        harness
            .query_by_label("The data layer for physical AI")
            .is_some()
    });
    harness.assert_browser_url_parameter("");

    assert!(
        harness.query_by_label("Loading data source:").is_none(),
        "An unsupported S3 URI should not be treated as a local file"
    );
}

#[tokio::test(flavor = "multi_thread")]
pub async fn failed_loading_returns_to_the_previous_route_without_history() {
    let DelayedFailingHttpUrl { url, fail_request } = delayed_failing_http_url();
    let mut harness = InspectionHarness::spawn(HarnessConfig {
        startup_url: Some(url.clone()),
        browser_wait_until_navigated: false, // We don't want to wait until we're beyond the loading screen.
        ..Default::default()
    });

    fn assert_back_is_disabled(harness: &InspectionHarness) {
        let back_button = harness
            .query_by_label("go back")
            .expect("The navigation bar should contain a back button");
        assert!(
            back_button.accesskit_node().is_disabled(),
            "The transient or rejected URL should not create a history entry"
        );
    }

    harness.step_until("Loading screen appears", |harness| {
        harness.query_by_label("Loading data source:").is_some()
    });
    assert_back_is_disabled(&harness);
    harness.assert_browser_url_parameter(&url);

    fail_request
        .send(())
        .expect("Failed to release the HTTP response");

    harness.step_until("Loading error appears", |harness| {
        harness.query_by_label("Go Back").is_some()
    });
    assert!(harness.query_by_label("Loading data source:").is_none());
    assert_back_is_disabled(&harness);

    let mut snapshot_results = SnapshotResults::new();
    snapshot_results.add(harness.try_snapshot("failed_loading_error"));
    harness.assert_browser_url_parameter(&url);

    harness
        .query_by_label("Go Back")
        .expect("The loading error should provide a way back")
        .click();
    harness.step_until("Welcome screen reappears", |harness| {
        harness
            .query_by_label("The data layer for physical AI")
            .is_some()
    });
    assert_back_is_disabled(&harness);
    harness.assert_browser_url_parameter("");
}
