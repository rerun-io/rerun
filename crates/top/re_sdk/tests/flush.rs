use std::time::Duration;

use re_sdk::RecordingStreamBuilder;

#[test]
fn test_drop_grpc_sink() {
    re_log::setup_logging();
    let url_to_nowhere = "rerun+http://not.real:1234/proxy";

    re_log::info!("Connecting…");
    let rec = RecordingStreamBuilder::new("rerun_example_grpc_drop_test")
        .connect_grpc_opts(url_to_nowhere)
        .unwrap();

    re_log::info!("Flushing with timeout…");
    assert!(rec.flush(Some(Duration::from_secs(2))).is_err());

    re_log::info!("Dropping recording…");
    drop(rec);

    re_log::info!("Done.");
}
