use std::time::Duration;

use re_sdk::{
    RecordingStream, RecordingStreamBuilder, RecordingStreamResult,
    external::re_grpc_client::write::Options, log::ChunkBatcherConfig, sink::GrpcSink,
};

/// Creates a recording with a saturated gRPC sink that cannot establish a connection.
///
/// Uses a shorter connection timeout than the default to keep the tests fast.
fn saturated_unconnected_grpc_recording(
    application_id: &'static str,
) -> RecordingStreamResult<RecordingStream> {
    let rec = RecordingStreamBuilder::new(application_id)
        .batcher_config(ChunkBatcherConfig::ALWAYS_TEST_ONLY)
        .buffered()?;
    let uri = "rerun+http://127.0.0.1:0/proxy".parse()?;
    let options = Options {
        connect_timeout_on_flush: Duration::from_millis(50),
        ..Default::default()
    };
    rec.set_sink(Box::new(GrpcSink::new_with_options(uri, options)));

    for i in 0..1_000 {
        rec.log(
            "scalar",
            &re_sdk_types::archetypes::Scalars::single(f64::from(i)),
        )?;
    }

    Ok(rec)
}

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

/// Test that shutdown releases a saturated gRPC sink that has not established a connection.
#[test]
fn test_drop_saturated_unconnected_grpc_sink() {
    re_log::setup_logging();
    let rec =
        saturated_unconnected_grpc_recording("rerun_example_saturated_grpc_drop_test").unwrap();

    drop(rec); // If the test hangs here, we have a bug!
}

/// Test that disconnect releases a saturated gRPC sink that has not established a connection.
#[test]
fn test_disconnect_saturated_unconnected_grpc_sink() {
    re_log::setup_logging();
    let rec = saturated_unconnected_grpc_recording("rerun_example_saturated_grpc_disconnect_test")
        .unwrap();

    rec.disconnect(); // If the test hangs here, we have a bug!
}
