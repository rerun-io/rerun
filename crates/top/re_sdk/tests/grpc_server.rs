#![cfg(feature = "server")]

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::{Duration, Instant};

use re_chunk::Chunk;
use re_log_channel::DataSourceMessage;
use re_log_types::LogMsg;
use re_sdk::RecordingStreamBuilder;

const TIMEOUT: Duration = Duration::from_secs(10);

fn unused_local_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("failed to bind an ephemeral port")
        .local_addr()
        .expect("failed to read the ephemeral port")
        .port()
}

fn wait_for_server(addr: SocketAddr) {
    let start_time = Instant::now();
    while start_time.elapsed() < TIMEOUT {
        if TcpStream::connect_timeout(&addr, Duration::from_millis(100)).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("gRPC server did not start listening on {addr}");
}

fn wait_for_server_shutdown(addr: SocketAddr) {
    let start_time = Instant::now();
    while start_time.elapsed() < TIMEOUT {
        if TcpStream::connect_timeout(&addr, Duration::from_millis(100)).is_err() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("gRPC server did not stop listening on {addr}");
}

#[test]
fn grpc_server_sink_streams_recording_data_and_shuts_down() -> Result<(), Box<dyn std::error::Error>>
{
    let port = unused_local_port();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let sink = re_sdk::grpc_server::GrpcServerSink::new(
        "127.0.0.1",
        port,
        re_sdk::ServerOptions::default(),
    )?;
    let uri = sink.uri();

    let rec =
        RecordingStreamBuilder::new("rerun_example_grpc_server_sink_test").set_sinks((sink,))?;

    wait_for_server(addr);

    rec.log(
        "test/points",
        &re_sdk_types::archetypes::Points2D::new([[1.0, 2.0]]),
    )?;
    rec.flush_blocking()?;

    let runtime = tokio::runtime::Builder::new_multi_thread() // NOLINT: needed for testing.
        .worker_threads(1)
        .enable_all()
        .build()?;
    let async_runtime = re_async::AsyncRuntimeHandle::new_native(runtime.handle().clone());
    let receiver = re_grpc_client::read::stream(&async_runtime, uri);

    let start_time = Instant::now();
    loop {
        let remaining = TIMEOUT
            .checked_sub(start_time.elapsed())
            .expect("timed out waiting for logged data");

        let message = receiver.recv_timeout(remaining)?;
        if let Some(DataSourceMessage::LogMsg(LogMsg::ArrowMsg(received_store_id, arrow_msg))) =
            message.data()
        {
            let chunk = Chunk::from_arrow_msg(arrow_msg)?;
            if chunk.entity_path() == &"test/points".into() {
                assert_eq!(received_store_id, &rec.store_info().unwrap().store_id);
                assert_eq!(chunk.num_rows(), 1);
                break;
            }
        }
    }

    drop(rec);
    wait_for_server_shutdown(addr);

    Ok(())
}
