use std::time::Duration;

use re_sdk::RecordingStreamBuilder;

/// Test that we don't block forever when dropping
/// a broken gRPC sink.
#[test]
fn test_drop_grpc_sink() {
    re_log::setup_logging();
    let url_to_nowhere = "rerun+http://not.real:1234/proxy";

    re_log::info!("Connecting…");
    // TODO(emilk): it would be nice to be able to configure `connect_timeout_on_flush` here to speed up this test.
    let rec = RecordingStreamBuilder::new("rerun_example_grpc_drop_test")
        .connect_grpc_opts(url_to_nowhere)
        .unwrap();

    re_log::info!("Flushing with timeout…");
    assert!(rec.flush_with_timeout(Duration::from_secs(2)).is_err());

    re_log::info!("Dropping recording…");
    drop(rec); // If the test hangs here, we have a bug!

    re_log::info!("Done.");
}
