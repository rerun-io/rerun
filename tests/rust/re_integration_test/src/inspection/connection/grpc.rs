//! Drive an out-of-process `rerun` viewer over the gRPC `Inspect` RPC.

use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use egui_inspection::protocol::{self, Request, Response};
use re_protos::sdk_comms::v1alpha1::{
    InspectRequest, viewer_control_service_client::ViewerControlServiceClient,
};
use tonic::transport::{Channel, Endpoint};

use crate::get_free_port;

use super::{CONNECT_TIMEOUT, HarnessConfig, TestEnv};

/// Per-request gRPC deadline.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// One inspection exchange handed to the gRPC worker thread: the request plus a channel to send
/// the (decoded) response — or an error message — back on.
struct InspectJob {
    request: Request,
    reply: mpsc::SyncSender<Result<Response, String>>,
}

/// A blocking connection to an out-of-process viewer's `ViewerControlService`, carrying one
/// `egui_inspection` request/response exchange per unary `Inspect` call.
///
/// `tonic` is async-only, so the actual gRPC work runs on a dedicated worker thread that owns a
/// small tokio runtime and the client. This keeps the transport blocking to callers regardless of
/// whether they run inside a tokio runtime (an ambient runtime would make `block_on`/`block_in_place`
/// awkward), at the cost of one channel round-trip per request.
pub(in crate::inspection) struct GrpcConnection {
    requests: mpsc::SyncSender<InspectJob>,
    _worker: std::thread::JoinHandle<()>,

    /// Kills the spawned viewer process when the connection is dropped.
    _child: ProcessChildGuard,
}

impl GrpcConnection {
    /// Launch a native `rerun --integration-test` process and connect to it over gRPC.
    ///
    /// Runs `--headless` unless [`TestEnv::windowed`] is set, in which case a real viewer window is
    /// opened so a developer can watch the test.
    pub(super) fn spawn_viewer(config: &HarnessConfig) -> Self {
        let binary = TestEnv::get().resolve_rerun_binary();
        let port = get_free_port();

        let mut command = Command::new(&binary);
        if !TestEnv::get().windowed {
            command.arg("--headless");
        }
        command
            .arg("--integration-test")
            .arg("--hide-welcome-screen")
            .arg("--port")
            .arg(port.to_string());
        if let Some(url) = &config.startup_url {
            command.arg(url);
        }
        // Inherit stdout/stderr so the viewer's logs show up in the test output.
        let child = command.stdin(Stdio::null()).spawn().unwrap_or_else(|err| {
            panic!(
                "Failed to spawn rerun viewer at {}: {err}",
                binary.display()
            )
        });

        Self::connect(
            &format!("http://127.0.0.1:{port}"),
            ProcessChildGuard(child),
        )
    }

    /// Spawn the worker thread and block until it has connected to `endpoint` (retrying, with a
    /// `GetInfo` liveness check, until [`CONNECT_TIMEOUT`] elapses).
    fn connect(endpoint: &str, child: ProcessChildGuard) -> Self {
        let endpoint = endpoint.to_owned();
        // Bounded channels (workspace lint disallows the unbounded `mpsc::channel`). Requests are
        // sent one at a time (each blocks on its reply), so a small buffer never fills.
        let (requests_tx, requests_rx) = mpsc::sync_channel::<InspectJob>(16);
        let (ready_tx, ready_rx) = mpsc::sync_channel::<()>(1);
        let worker = std::thread::Builder::new()
            .name("inspection-grpc".to_owned())
            .spawn(move || grpc_worker(&endpoint, &requests_rx, &ready_tx))
            .expect("Failed to spawn the gRPC worker thread");

        // Block until the worker signals it has connected. If it panics while connecting, `ready_tx`
        // is dropped and `recv` returns `Err` (the worker's own panic message goes to stderr).
        ready_rx
            .recv()
            .expect("gRPC worker failed to connect to the viewer");

        Self {
            requests: requests_tx,
            _worker: worker,
            _child: child,
        }
    }

    /// Send one request and return its response, panicking on transport failure or a
    /// [`Response::Error`] reply.
    pub(super) fn request(&self, request: Request) -> Response {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.requests
            .send(InspectJob {
                request,
                reply: reply_tx,
            })
            .expect("gRPC worker thread is gone");
        reply_rx
            .recv()
            .expect("gRPC worker dropped the reply")
            .unwrap_or_else(|message| panic!("{message}"))
    }
}

/// Spawn a tokio runtime on a thread so that the tests may stay async-free.
fn grpc_worker(
    endpoint: &str,
    requests: &mpsc::Receiver<InspectJob>,
    ready: &mpsc::SyncSender<()>,
) {
    // This thread is the runtime's owner. It exists so that the blocking test API can talk to a
    // `tonic` client without an ambient runtime, so it cannot take an `AsyncRuntimeHandle`.
    let runtime = tokio::runtime::Builder::new_multi_thread() // NOLINT: owned by this worker thread
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("Failed to build the gRPC worker runtime");

    let mut client = runtime.block_on(connect_grpc_client(endpoint));
    ready
        .send(())
        .expect("Connection was dropped before we finished connecting.");

    while let Ok(job) = requests.recv() {
        let response = runtime.block_on(do_inspect(&mut client, job.request));
        job.reply
            .send(response)
            .expect("We should always be able to send a response.");
    }
}

/// Connect to `endpoint`, retrying (with a `GetInfo` liveness check) until [`CONNECT_TIMEOUT`].
async fn connect_grpc_client(endpoint: &str) -> ViewerControlServiceClient<Channel> {
    let endpoint = Endpoint::from_shared(endpoint.to_owned())
        .unwrap_or_else(|err| panic!("Invalid viewer endpoint {endpoint:?}: {err}"))
        .timeout(REQUEST_TIMEOUT);

    let deadline = tokio::time::Instant::now() + CONNECT_TIMEOUT;
    loop {
        if let Ok(channel) = endpoint.connect().await {
            let mut client = ViewerControlServiceClient::new(channel);
            // A successful `GetInfo` confirms the viewer is up and serving inspection.
            if get_info(&mut client).await {
                return client;
            }
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Timed out connecting to the headless viewer at {:?} after {CONNECT_TIMEOUT:?}",
            endpoint.uri()
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn get_info(client: &mut ViewerControlServiceClient<Channel>) -> bool {
    let Ok(request) = protocol::encode_body(&Request::GetInfo) else {
        return false;
    };
    match client.inspect(InspectRequest { request }).await {
        Ok(response) => matches!(
            protocol::decode_body(&response.into_inner().response),
            Ok(Response::Info { .. })
        ),
        Err(_) => false,
    }
}

/// One unary `Inspect` exchange. Returns the decoded response, or an error message for a transport
/// failure or a [`Response::Error`] reply.
async fn do_inspect(
    client: &mut ViewerControlServiceClient<Channel>,
    request: Request,
) -> Result<Response, String> {
    let request = protocol::encode_body(&request).expect("Failed to encode inspection request");
    let response = client
        .inspect(InspectRequest { request })
        .await
        .map_err(|err| format!("inspect rpc failed: {err}"))?
        .into_inner();
    match protocol::decode_body(&response.response).expect("Failed to decode inspection response") {
        Response::Error { message } => Err(format!("Viewer returned an error: {message}")),
        response => Ok(response),
    }
}

/// Kills the spawned viewer process on drop.
struct ProcessChildGuard(Child);

impl Drop for ProcessChildGuard {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}
