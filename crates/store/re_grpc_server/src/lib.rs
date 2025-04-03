//! Server implementation of an in-memory Storage Node.

pub mod shutdown;

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::pin::Pin;

use re_protos::sdk_comms::v1alpha1::ReadTablesRequest;
use re_protos::sdk_comms::v1alpha1::ReadTablesResponse;
use re_protos::sdk_comms::v1alpha1::WriteMessagesRequest;
use re_protos::sdk_comms::v1alpha1::WriteTableRequest;
use re_protos::sdk_comms::v1alpha1::WriteTableResponse;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::Stream;
use tokio_stream::StreamExt as _;
use tonic::transport::server::TcpIncoming;
use tonic::transport::Server;
use tower_http::cors::CorsLayer;

use re_byte_size::SizeBytes as _;
use re_memory::MemoryLimit;
use re_protos::{
    common::v1alpha1::{DataframePart as DataframePartProto, StoreKind as StoreKindProto},
    log_msg::v1alpha1::LogMsg as LogMsgProto,
    sdk_comms::v1alpha1::{
        message_proxy_service_server, ReadMessagesRequest, ReadMessagesResponse,
        TableId as TableIdProto, WriteMessagesResponse,
    },
};

pub const DEFAULT_SERVER_PORT: u16 = 9876;
pub const DEFAULT_MEMORY_LIMIT: MemoryLimit = MemoryLimit::UNLIMITED;

const MAX_DECODING_MESSAGE_SIZE: usize = u32::MAX as usize;
const MAX_ENCODING_MESSAGE_SIZE: usize = MAX_DECODING_MESSAGE_SIZE;

// TODO(jan): Refactor `serve`/`spawn` variants into a builder?

/// Start a Rerun server, listening on `addr`.
///
/// A Rerun server is an in-memory implementation of a Storage Node.
///
/// The returned future must be polled for the server to make progress.
///
/// Currently, the only RPCs supported by the server are `WriteMessages` and `ReadMessages`.
///
/// Clients send data to the server via `WriteMessages`. Any sent messages will be stored
/// in the server's message queue. Messages are only removed if the server hits its configured
/// memory limit.
///
/// Clients receive data from the server via `ReadMessages`. Upon establishing the stream,
/// the server sends all messages stored in its message queue, and subscribes the client
/// to the queue. Any messages sent to the server through `WriteMessages` will be proxied
/// to the open `ReadMessages` stream.
pub async fn serve(
    addr: SocketAddr,
    memory_limit: MemoryLimit,
    shutdown: shutdown::Shutdown,
) -> Result<(), tonic::transport::Error> {
    serve_impl(addr, MessageProxy::new(memory_limit), shutdown).await
}

async fn serve_impl(
    addr: SocketAddr,
    message_proxy: MessageProxy,
    shutdown: shutdown::Shutdown,
) -> Result<(), tonic::transport::Error> {
    let tcp_listener = TcpListener::bind(addr)
        .await
        .unwrap_or_else(|err| panic!("failed to bind listener on {addr}: {err}"));

    let incoming =
        TcpIncoming::from_listener(tcp_listener, true, None).expect("failed to init listener");

    let connect_addr = if addr.ip().is_loopback() || addr.ip().is_unspecified() {
        format!("rerun+http://127.0.0.1:{}/proxy", addr.port())
    } else {
        format!("rerun+http://{addr}/proxy")
    };
    re_log::info!("Listening for gRPC connections on {addr}. Connect by running `rerun --connect {connect_addr}`");

    let cors = CorsLayer::very_permissive();
    let grpc_web = tonic_web::GrpcWebLayer::new();

    let routes = {
        let mut routes_builder = tonic::service::Routes::builder();
        routes_builder.add_service(
            re_protos::sdk_comms::v1alpha1::message_proxy_service_server::MessageProxyServiceServer::new(
                message_proxy,
            )
            .max_decoding_message_size(MAX_DECODING_MESSAGE_SIZE)
            .max_encoding_message_size(MAX_ENCODING_MESSAGE_SIZE),
        );
        routes_builder.routes()
    };

    Server::builder()
        .accept_http1(true) // Support `grpc-web` clients
        .layer(cors) // Allow CORS requests from web clients
        .layer(grpc_web) // Support `grpc-web` clients
        .add_routes(routes)
        .serve_with_incoming_shutdown(incoming, shutdown.wait())
        .await
}

/// Start a Rerun server, listening on `addr`.
///
/// The returned future must be polled for the server to make progress.
///
/// This function additionally accepts a smart channel, through which messages
/// can be sent to the server directly. It is similar to creating a client
/// and sending messages through `WriteMessages`, but without the overhead
/// of a localhost connection.
///
/// See [`serve`] for more information about what a Rerun server is.
pub async fn serve_from_channel(
    addr: SocketAddr,
    memory_limit: MemoryLimit,
    shutdown: shutdown::Shutdown,
    channel_rx: re_smart_channel::Receiver<re_log_types::LogMsg>,
) {
    let message_proxy = MessageProxy::new(memory_limit);
    let event_tx = message_proxy.event_tx.clone();

    tokio::spawn(async move {
        use re_smart_channel::SmartMessagePayload;

        loop {
            let msg = match channel_rx.try_recv() {
                Ok(msg) => match msg.payload {
                    SmartMessagePayload::Msg(msg) => msg,
                    SmartMessagePayload::Flush { on_flush_done } => {
                        on_flush_done(); // we don't buffer
                        continue;
                    }
                    SmartMessagePayload::Quit(err) => {
                        if let Some(err) = err {
                            re_log::debug!("smart channel sender quit: {err}");
                        } else {
                            re_log::debug!("smart channel sender quit");
                        }
                        break;
                    }
                },
                Err(re_smart_channel::TryRecvError::Disconnected) => {
                    re_log::debug!("smart channel sender closed, closing receiver");
                    break;
                }
                Err(re_smart_channel::TryRecvError::Empty) => {
                    // Let other tokio tasks run:
                    tokio::task::yield_now().await;
                    continue;
                }
            };

            let msg = match re_log_encoding::protobuf_conversions::log_msg_to_proto(
                msg,
                re_log_encoding::Compression::LZ4,
            ) {
                Ok(msg) => msg,
                Err(err) => {
                    re_log::error!("failed to encode message: {err}");
                    continue;
                }
            };

            if event_tx.send(Event::Message(msg)).await.is_err() {
                re_log::debug!("shut down, closing sender");
                break;
            }
        }
    });

    if let Err(err) = serve_impl(addr, message_proxy, shutdown).await {
        re_log::error!("message proxy server crashed: {err}");
    }
}

/// Start a Rerun server, listening on `addr`.
///
/// This function additionally accepts a `ReceiveSet`, from which the
/// server will read all messages. It is similar to creating a client
/// and sending messages through `WriteMessages`, but without the overhead
/// of a localhost connection.
///
/// See [`serve`] for more information about what a Rerun server is.
pub fn spawn_from_rx_set(
    addr: SocketAddr,
    memory_limit: MemoryLimit,
    shutdown: shutdown::Shutdown,
    rxs: re_smart_channel::ReceiveSet<re_log_types::LogMsg>,
) {
    let message_proxy = MessageProxy::new(memory_limit);
    let event_tx = message_proxy.event_tx.clone();

    tokio::spawn(async move {
        if let Err(err) = serve_impl(addr, message_proxy, shutdown).await {
            re_log::error!("message proxy server crashed: {err}");
        }
    });

    tokio::spawn(async move {
        loop {
            let Some(msg) = rxs.try_recv().and_then(|(_, m)| m.into_data()) else {
                if rxs.is_empty() {
                    // We won't ever receive more data:
                    break;
                }
                // Because `try_recv` is blocking, we should give other tasks
                // a chance to run before we continue
                tokio::task::yield_now().await;
                continue;
            };

            let msg = match re_log_encoding::protobuf_conversions::log_msg_to_proto(
                msg,
                re_log_encoding::Compression::LZ4,
            ) {
                Ok(msg) => msg,
                Err(err) => {
                    re_log::error!("failed to encode message: {err}");
                    continue;
                }
            };

            if event_tx.send(Event::Message(msg)).await.is_err() {
                re_log::debug!("shut down, closing sender");
                break;
            }
        }
    });
}

/// Start a Rerun server, listening on `addr`.
///
/// This function additionally creates a smart channel, and returns its receiving end.
/// Any messages received by the server are sent through the channel. This is similar
/// to creating a client and calling `ReadMessages`, but without the overhead of a
/// localhost connection.
///
/// The server is spawned as a task on a `tokio` runtime. This function panics if the
/// runtime is not available.
///
/// See [`serve`] for more information about what a Rerun server is.
pub fn spawn_with_recv(
    addr: SocketAddr,
    memory_limit: MemoryLimit,
    shutdown: shutdown::Shutdown,
) -> (
    re_smart_channel::Receiver<re_log_types::LogMsg>,
    re_smart_channel::Receiver<re_log_types::TableMsg>,
) {
    let (channel_log_tx, channel_log_rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::MessageProxy {
            url: format!("rerun+http://{addr}/proxy"),
        },
        re_smart_channel::SmartChannelSource::MessageProxy {
            url: format!("rerun+http://{addr}/proxy"),
        },
    );
    let (channel_table_tx, channel_table_rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::MessageProxy {
            url: format!("rerun+http://{addr}/proxy"),
        },
        re_smart_channel::SmartChannelSource::MessageProxy {
            url: format!("rerun+http://{addr}/proxy"),
        },
    );
    let (message_proxy, mut broadcast_log_rx, mut broadcast_table_rx) =
        MessageProxy::new_with_recv(memory_limit);
    tokio::spawn(async move {
        if let Err(err) = serve_impl(addr, message_proxy, shutdown).await {
            re_log::error!("message proxy server crashed: {err}");
        }
    });
    tokio::spawn(async move {
        loop {
            let msg = match broadcast_log_rx.recv().await {
                Ok(msg) => re_log_encoding::protobuf_conversions::log_msg_from_proto(msg),
                Err(broadcast::error::RecvError::Closed) => {
                    re_log::debug!("message proxy server shut down, closing receiver");
                    channel_log_tx.quit(None).ok();
                    break;
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    re_log::debug!(
                        "message proxy receiver dropped {n} messages due to backpressure"
                    );
                    continue;
                }
            };
            match msg {
                Ok(msg) => {
                    if channel_log_tx.send(msg).is_err() {
                        re_log::debug!(
                            "message proxy smart channel receiver closed, closing sender"
                        );
                        break;
                    }
                }
                Err(err) => {
                    re_log::error!("dropping LogMsg due to failed decode: {err}");
                    continue;
                }
            }
        }
    });
    todo!("tables");
    (channel_log_rx, channel_table_rx)
}

enum Event {
    /// New client connected, requesting full history and subscribing to new messages.
    NewClient(
        oneshot::Sender<(
            Vec<Msg>,
            broadcast::Receiver<LogMsgProto>,
            broadcast::Receiver<TableMsgProto>,
        )>,
    ),

    /// A client sent a message.
    Message(LogMsgProto),

    /// A client sent a table.
    Table(TableMsgProto),
}

// TODO: just use `WriteTableRequest` directly?
#[derive(Clone)]
struct TableMsgProto {
    id: TableIdProto,
    data: DataframePartProto,
}

#[derive(Clone)]
enum Msg {
    LogMsg(LogMsgProto),
    Table(TableMsgProto),
}

impl Msg {
    fn total_size_bytes(&self) -> u64 {
        match self {
            Msg::LogMsg(log_msg) => message_size(log_msg),
            Msg::Table(table) => table_size(table),
        }
    }
}

impl From<LogMsgProto> for Msg {
    fn from(value: LogMsgProto) -> Self {
        Self::LogMsg(value)
    }
}

impl From<TableMsgProto> for Msg {
    fn from(value: TableMsgProto) -> Self {
        Self::Table(value)
    }
}

/// Main event loop for the server, which runs in its own task.
///
/// Handles message history, and broadcasts messages to clients.
struct EventLoop {
    server_memory_limit: MemoryLimit,

    /// New log messages are broadcast to all clients.
    broadcast_log_tx: broadcast::Sender<LogMsgProto>,

    /// New table messages are broadcast to all clients.
    broadcast_table_tx: broadcast::Sender<TableMsgProto>,

    /// Channel for incoming events.
    event_rx: mpsc::Receiver<Event>,

    /// Messages stored in order of arrival, and garbage collected if the server hits the memory limit.
    ordered_message_queue: VecDeque<Msg>,

    /// Total size of `ordered_message_queue` in bytes.
    ordered_message_bytes: u64,

    /// Messages potentially out of order with the rest of the message stream. These are never garbage collected.
    persistent_message_queue: VecDeque<LogMsgProto>,
}

impl EventLoop {
    fn new(
        server_memory_limit: MemoryLimit,
        event_rx: mpsc::Receiver<Event>,
        broadcast_log_tx: broadcast::Sender<LogMsgProto>,
        broadcast_table_tx: broadcast::Sender<TableMsgProto>,
    ) -> Self {
        Self {
            server_memory_limit,
            broadcast_log_tx,
            broadcast_table_tx,
            event_rx,
            ordered_message_queue: Default::default(),
            ordered_message_bytes: 0,
            persistent_message_queue: Default::default(),
        }
    }

    async fn run_in_place(mut self) {
        loop {
            let Some(event) = self.event_rx.recv().await else {
                break;
            };

            match event {
                Event::NewClient(channel) => self.handle_new_client(channel),
                Event::Message(msg) => self.handle_msg(msg),
                Event::Table(table) => self.handle_table(table),
            }
        }
    }

    fn handle_new_client(
        &self,
        channel: oneshot::Sender<(
            Vec<Msg>,
            broadcast::Receiver<LogMsgProto>,
            broadcast::Receiver<TableMsgProto>,
        )>,
    ) {
        channel
            .send((
                // static messages come first
                self.persistent_message_queue
                    .iter()
                    .cloned()
                    .map(Msg::from)
                    .chain(self.ordered_message_queue.iter().cloned())
                    .collect(),
                self.broadcast_log_tx.subscribe(),
                self.broadcast_table_tx.subscribe(),
            ))
            .ok();
    }

    fn handle_msg(&mut self, msg: LogMsgProto) {
        self.broadcast_log_tx.send(msg.clone()).ok();

        self.gc_if_using_too_much_ram();

        let Some(inner) = &msg.msg else {
            re_log::error!(
                "{}",
                re_protos::missing_field!(re_protos::log_msg::v1alpha1::LogMsg, "msg")
            );
            return;
        };

        // We put store info, blueprint data, and blueprint activation commands
        // in a separate queue that does *not* get garbage collected.
        use re_protos::log_msg::v1alpha1::log_msg::Msg;
        match inner {
            // Store info, blueprint activation commands
            Msg::SetStoreInfo(..) | Msg::BlueprintActivationCommand(..) => {
                self.persistent_message_queue.push_back(msg);
            }

            // Blueprint data
            Msg::ArrowMsg(ref inner)
                if inner
                    .store_id
                    .as_ref()
                    .is_some_and(|id| id.kind() == StoreKindProto::Blueprint) =>
            {
                self.persistent_message_queue.push_back(msg);
            }

            // Recording data
            Msg::ArrowMsg(..) => {
                let approx_size_bytes = message_size(&msg);
                self.ordered_message_bytes += approx_size_bytes;
                self.ordered_message_queue.push_back(msg.into());
            }
        }
    }

    fn handle_table(&mut self, table: TableMsgProto) {
        self.broadcast_table_tx.send(table.clone()).ok();

        self.gc_if_using_too_much_ram();

        let approx_size_bytes = table_size(&table);
        self.ordered_message_bytes += approx_size_bytes;
        self.ordered_message_queue.push_back(Msg::Table(table));
    }

    fn gc_if_using_too_much_ram(&mut self) {
        re_tracing::profile_function!();

        let Some(max_bytes) = self.server_memory_limit.max_bytes else {
            // Unlimited memory!
            return;
        };

        let max_bytes = max_bytes as u64;
        if max_bytes >= self.ordered_message_bytes {
            // We're not using too much memory.
            return;
        };

        {
            re_tracing::profile_scope!("Drop messages");
            re_log::info_once!(
                "Memory limit ({}) exceeded. Dropping old log messages from the server. Clients connecting after this will not see the full history.",
                re_format::format_bytes(max_bytes as _)
            );

            let bytes_to_free = self.ordered_message_bytes - max_bytes;

            let mut bytes_dropped = 0;
            let mut messages_dropped = 0;

            while bytes_dropped < bytes_to_free {
                // only drop messages from temporal queue
                if let Some(msg) = self.ordered_message_queue.pop_front() {
                    bytes_dropped += msg.total_size_bytes();
                    messages_dropped += 1;
                } else {
                    break;
                }
            }

            re_log::trace!(
                "Dropped {} bytes in {messages_dropped} message(s)",
                re_format::format_bytes(bytes_dropped as _)
            );
        }
    }
}

fn message_size(msg: &LogMsgProto) -> u64 {
    msg.total_size_bytes()
}

fn table_size(table: &TableMsgProto) -> u64 {
    let TableMsgProto { id, data } = table;
    id.total_size_bytes() + data.total_size_bytes()
}

pub struct MessageProxy {
    _queue_task_handle: tokio::task::JoinHandle<()>,
    event_tx: mpsc::Sender<Event>,
}

impl MessageProxy {
    pub fn new(server_memory_limit: MemoryLimit) -> Self {
        Self::new_with_recv(server_memory_limit).0
    }

    fn new_with_recv(
        server_memory_limit: MemoryLimit,
    ) -> (
        Self,
        broadcast::Receiver<LogMsgProto>,
        broadcast::Receiver<TableMsgProto>,
    ) {
        // Channel capacity is completely arbitrary.
        // We just want something large enough to handle bursts of messages.
        let (event_tx, event_rx) = mpsc::channel(1024);
        let (broadcast_log_tx, broadcast_log_rx) = broadcast::channel(1024);
        let (broadcast_table_tx, broadcast_table_rx) = broadcast::channel(1024);

        let task_handle = tokio::spawn(async move {
            EventLoop::new(
                server_memory_limit,
                event_rx,
                broadcast_log_tx,
                broadcast_table_tx,
            )
            .run_in_place()
            .await;
        });

        (
            Self {
                _queue_task_handle: task_handle,
                event_tx,
            },
            broadcast_log_rx,
            broadcast_table_rx,
        )
    }

    async fn push_msg(&self, msg: LogMsgProto) {
        self.event_tx.send(Event::Message(msg)).await.ok();
    }

    async fn push_table(&self, table: TableMsgProto) {
        self.event_tx.send(Event::Table(table)).await.ok();
    }

    async fn new_client_stream(&self) -> ReadMessagesStream {
        let (sender, receiver) = oneshot::channel();
        if let Err(err) = self.event_tx.send(Event::NewClient(sender)).await {
            re_log::error!("Error initializing new client: {err}");
            return Box::pin(tokio_stream::empty());
        };
        let (history, log_channel, _) = match receiver.await {
            Ok(v) => v,
            Err(err) => {
                re_log::error!("Error initializing new client: {err}");
                return Box::pin(tokio_stream::empty());
            }
        };

        let history = tokio_stream::iter(
            history
                .into_iter()
                // TODO:
                .filter_map(|log_msg| {
                    if let Msg::LogMsg(log_msg) = log_msg {
                        Some(ReadMessagesResponse {
                            log_msg: Some(log_msg),
                        })
                    } else {
                        None
                    }
                })
                .map(Ok),
        );
        let channel = BroadcastStream::new(log_channel).map(|result| {
            result
                .map(|log_msg| ReadMessagesResponse {
                    log_msg: Some(log_msg),
                })
                .map_err(|err| {
                    re_log::error!("Error reading message from broadcast channel: {err}");
                    tonic::Status::internal("internal channel error")
                })
        });

        Box::pin(history.chain(channel))
    }
}

type ReadMessagesStream = Pin<Box<dyn Stream<Item = tonic::Result<ReadMessagesResponse>> + Send>>;
type ReadTablesStream = Pin<Box<dyn Stream<Item = tonic::Result<ReadTablesResponse>> + Send>>;

#[tonic::async_trait]
impl message_proxy_service_server::MessageProxyService for MessageProxy {
    async fn write_messages(
        &self,
        request: tonic::Request<tonic::Streaming<WriteMessagesRequest>>,
    ) -> tonic::Result<tonic::Response<WriteMessagesResponse>> {
        let mut stream = request.into_inner();
        loop {
            match stream.message().await {
                Ok(Some(WriteMessagesRequest {
                    log_msg: Some(log_msg),
                })) => {
                    self.push_msg(log_msg).await;
                }

                Ok(Some(WriteMessagesRequest { log_msg: None })) => {
                    re_log::warn!("missing log_msg in WriteMessagesRequest");
                }

                Ok(None) => {
                    // Connection was closed
                    break;
                }

                Err(err) => {
                    re_log::error!("Error while receiving messages: {err}");
                    break;
                }
            }
        }

        Ok(tonic::Response::new(WriteMessagesResponse {}))
    }

    type ReadMessagesStream = ReadMessagesStream;

    async fn read_messages(
        &self,
        _: tonic::Request<ReadMessagesRequest>,
    ) -> tonic::Result<tonic::Response<Self::ReadMessagesStream>> {
        Ok(tonic::Response::new(self.new_client_stream().await))
    }

    type ReadTablesStream = ReadTablesStream;

    async fn write_table(
        &self,
        request: tonic::Request<WriteTableRequest>,
    ) -> tonic::Result<tonic::Response<WriteTableResponse>> {
        if let WriteTableRequest {
            id: Some(id),
            data: Some(data),
        } = request.into_inner()
        {
            self.push_table(TableMsgProto { id, data }).await;
        } else {
            re_log::warn!("malformed `WriteTableRequest`");
        }

        Ok(tonic::Response::new(WriteTableResponse {}))
    }

    async fn read_tables(
        &self,
        _: tonic::Request<ReadTablesRequest>,
    ) -> tonic::Result<tonic::Response<Self::ReadTablesStream>> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use re_build_info::CrateVersion;
    use re_chunk::RowId;
    use re_log_encoding::protobuf_conversions::{log_msg_from_proto, log_msg_to_proto};
    use re_log_encoding::Compression;
    use re_log_types::{
        ApplicationId, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource,
    };
    use re_protos::sdk_comms::v1alpha1::{
        message_proxy_service_client::MessageProxyServiceClient,
        message_proxy_service_server::MessageProxyServiceServer,
    };
    use similar_asserts::assert_eq;
    use std::net::SocketAddr;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::net::TcpListener;
    use tokio_util::sync::CancellationToken;
    use tonic::transport::server::TcpIncoming;
    use tonic::transport::Channel;
    use tonic::transport::Endpoint;

    #[derive(Clone)]
    struct Completion(Arc<CancellationToken>);

    impl Drop for Completion {
        fn drop(&mut self) {
            self.finish();
        }
    }

    impl Completion {
        fn new() -> Self {
            Self(Arc::new(CancellationToken::new()))
        }

        fn finish(&self) {
            self.0.cancel();
        }

        async fn wait(&self) {
            self.0.cancelled().await;
        }
    }

    /// Generates `n` log messages wrapped in a `SetStoreInfo` at the start and `BlueprintActivationCommand` at the end,
    /// to exercise message ordering.
    fn fake_log_stream_blueprint(n: usize) -> Vec<LogMsg> {
        let store_id = StoreId::random(StoreKind::Blueprint);

        let mut messages = Vec::new();
        messages.push(LogMsg::SetStoreInfo(SetStoreInfo {
            row_id: *RowId::new(),
            info: StoreInfo {
                application_id: ApplicationId("test".to_owned()),
                store_id: store_id.clone(),
                cloned_from: None,
                store_source: StoreSource::RustSdk {
                    rustc_version: String::new(),
                    llvm_version: String::new(),
                },
                store_version: Some(CrateVersion::LOCAL),
            },
        }));
        for _ in 0..n {
            messages.push(LogMsg::ArrowMsg(
                store_id.clone(),
                re_chunk::Chunk::builder("test_entity".into())
                    .with_archetype(
                        re_chunk::RowId::new(),
                        re_log_types::TimePoint::default().with(
                            re_log_types::Timeline::new_sequence("blueprint"),
                            re_log_types::TimeInt::from_millis(re_log_types::NonMinI64::MIN),
                        ),
                        &re_types::blueprint::archetypes::Background::new(
                            re_types::blueprint::components::BackgroundKind::SolidColor,
                        )
                        .with_color([255, 0, 0]),
                    )
                    .build()
                    .unwrap()
                    .to_arrow_msg()
                    .unwrap(),
            ));
        }
        messages.push(LogMsg::BlueprintActivationCommand(
            re_log_types::BlueprintActivationCommand {
                blueprint_id: store_id,
                make_active: true,
                make_default: true,
            },
        ));

        messages
    }

    fn fake_log_stream_recording(n: usize) -> Vec<LogMsg> {
        let store_id = StoreId::random(StoreKind::Recording);

        let mut messages = Vec::new();
        messages.push(LogMsg::SetStoreInfo(SetStoreInfo {
            row_id: *RowId::new(),
            info: StoreInfo {
                application_id: ApplicationId("test".to_owned()),
                store_id: store_id.clone(),
                cloned_from: None,
                store_source: StoreSource::RustSdk {
                    rustc_version: String::new(),
                    llvm_version: String::new(),
                },
                store_version: Some(CrateVersion::LOCAL),
            },
        }));
        for _ in 0..n {
            messages.push(LogMsg::ArrowMsg(
                store_id.clone(),
                re_chunk::Chunk::builder("test_entity".into())
                    .with_archetype(
                        re_chunk::RowId::new(),
                        re_log_types::TimePoint::default().with(
                            re_log_types::Timeline::new_sequence("log_time"),
                            re_log_types::TimeInt::from_millis(re_log_types::NonMinI64::MIN),
                        ),
                        &re_types::archetypes::Points2D::new([(0.0, 0.0), (1.0, 1.0), (2.0, 2.0)]),
                    )
                    .build()
                    .unwrap()
                    .to_arrow_msg()
                    .unwrap(),
            ));
        }

        messages
    }

    async fn setup() -> (Completion, SocketAddr) {
        setup_with_memory_limit(MemoryLimit::UNLIMITED).await
    }

    async fn setup_with_memory_limit(memory_limit: MemoryLimit) -> (Completion, SocketAddr) {
        let completion = Completion::new();

        let tcp_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = tcp_listener.local_addr().unwrap();

        tokio::spawn({
            let completion = completion.clone();
            async move {
                tonic::transport::Server::builder()
                    .add_service(MessageProxyServiceServer::new(super::MessageProxy::new(
                        memory_limit,
                    )))
                    .serve_with_incoming_shutdown(
                        TcpIncoming::from_listener(tcp_listener, true, None).unwrap(),
                        completion.wait(),
                    )
                    .await
                    .unwrap();
            }
        });

        (completion, addr)
    }

    async fn make_client(addr: SocketAddr) -> MessageProxyServiceClient<Channel> {
        MessageProxyServiceClient::new(
            Endpoint::from_shared(format!("http://{addr}"))
                .unwrap()
                .connect()
                .await
                .unwrap(),
        )
    }

    async fn read_log_stream(
        log_stream: &mut tonic::Response<tonic::Streaming<ReadMessagesResponse>>,
        n: usize,
    ) -> Vec<LogMsg> {
        let mut stream_ref = log_stream
            .get_mut()
            .map(|result| log_msg_from_proto(result.unwrap().log_msg.unwrap()).unwrap());

        let mut messages = Vec::new();
        for _ in 0..n {
            messages.push(stream_ref.next().await.unwrap());
        }
        messages
    }

    #[tokio::test]
    async fn pubsub_basic() {
        let (completion, addr) = setup().await;
        let mut client = make_client(addr).await; // We use the same client for both producing and consuming
        let messages = fake_log_stream_blueprint(3);

        // start reading
        let mut log_stream = client.read_messages(ReadMessagesRequest {}).await.unwrap();

        // write a few messages
        client
            .write_messages(tokio_stream::iter(
                messages
                    .clone()
                    .into_iter()
                    .map(|msg| log_msg_to_proto(msg, Compression::Off).unwrap())
                    .map(|msg| WriteMessagesRequest { log_msg: Some(msg) }),
            ))
            .await
            .unwrap();

        // the messages should be echoed to us
        let actual = read_log_stream(&mut log_stream, messages.len()).await;

        assert_eq!(messages, actual);

        // While `SetStoreInfo` is sent first in `fake_log_stream`,
        // we can observe that it's also received first,
        // even though it is actually stored out of order in `persistent_message_queue`.
        assert!(matches!(messages[0], LogMsg::SetStoreInfo(..)));
        assert!(matches!(actual[0], LogMsg::SetStoreInfo(..)));

        completion.finish();
    }

    #[tokio::test]
    async fn pubsub_history() {
        let (completion, addr) = setup().await;
        let mut client = make_client(addr).await; // We use the same client for both producing and consuming
        let messages = fake_log_stream_blueprint(3);

        // don't read anything yet - these messages should be sent to us as part of history when we call `read_messages` later

        // Write a few messages:
        client
            .write_messages(tokio_stream::iter(
                messages
                    .clone()
                    .into_iter()
                    .map(|msg| log_msg_to_proto(msg, Compression::Off).unwrap())
                    .map(|msg| WriteMessagesRequest { log_msg: Some(msg) }),
            ))
            .await
            .unwrap();

        // Start reading now - we should receive full history at this point:
        let mut log_stream = client.read_messages(ReadMessagesRequest {}).await.unwrap();

        let actual = read_log_stream(&mut log_stream, messages.len()).await;
        assert_eq!(messages, actual);

        completion.finish();
    }

    #[tokio::test]
    async fn one_producer_many_consumers() {
        let (completion, addr) = setup().await;
        let mut producer = make_client(addr).await; // We use separate clients for producing and consuming
        let mut consumers = vec![make_client(addr).await, make_client(addr).await];
        let messages = fake_log_stream_blueprint(3);

        // Initialize multiple read streams:
        let mut log_streams = vec![];
        for consumer in &mut consumers {
            log_streams.push(
                consumer
                    .read_messages(ReadMessagesRequest {})
                    .await
                    .unwrap(),
            );
        }

        // Write a few messages using our single producer:
        producer
            .write_messages(tokio_stream::iter(
                messages
                    .clone()
                    .into_iter()
                    .map(|msg| log_msg_to_proto(msg, Compression::Off).unwrap())
                    .map(|msg| WriteMessagesRequest { log_msg: Some(msg) }),
            ))
            .await
            .unwrap();

        // Each consumer should've received them:
        for log_stream in &mut log_streams {
            let actual = read_log_stream(log_stream, messages.len()).await;
            assert_eq!(messages, actual);
        }

        completion.finish();
    }

    #[tokio::test]
    async fn many_producers_many_consumers() {
        let (completion, addr) = setup().await;
        let mut producers = vec![make_client(addr).await, make_client(addr).await];
        let mut consumers = vec![make_client(addr).await, make_client(addr).await];
        let messages = fake_log_stream_blueprint(3);

        // Initialize multiple read streams:
        let mut log_streams = vec![];
        for consumer in &mut consumers {
            log_streams.push(
                consumer
                    .read_messages(ReadMessagesRequest {})
                    .await
                    .unwrap(),
            );
        }

        // Write a few messages using each producer:
        for producer in &mut producers {
            producer
                .write_messages(tokio_stream::iter(
                    messages
                        .clone()
                        .into_iter()
                        .map(|msg| log_msg_to_proto(msg, Compression::Off).unwrap())
                        .map(|msg| WriteMessagesRequest { log_msg: Some(msg) }),
                ))
                .await
                .unwrap();
        }

        let expected = [messages.clone(), messages.clone()].concat();

        // Each consumer should've received one set of messages from each producer.
        // Note that in this test we also guarantee the order of messages across producers,
        // due to the `write_messages` calls being sequential.

        for log_stream in &mut log_streams {
            let actual = read_log_stream(log_stream, expected.len()).await;
            assert_eq!(actual, expected);
        }

        completion.finish();
    }

    #[tokio::test]
    async fn memory_limit_drops_messages() {
        // Use an absurdly low memory limit to force all messages to be dropped immediately from history
        let (completion, addr) = setup_with_memory_limit(MemoryLimit::from_bytes(1)).await;
        let mut client = make_client(addr).await;
        let messages = fake_log_stream_recording(3);

        // Write some messages
        client
            .write_messages(tokio_stream::iter(
                messages
                    .clone()
                    .into_iter()
                    .map(|msg| log_msg_to_proto(msg, Compression::Off).unwrap())
                    .map(|msg| WriteMessagesRequest { log_msg: Some(msg) }),
            ))
            .await
            .unwrap();

        // Start reading
        let mut log_stream = client.read_messages(ReadMessagesRequest {}).await.unwrap();
        let mut actual = vec![];
        loop {
            let timeout_stream = log_stream.get_mut().timeout(Duration::from_millis(100));
            tokio::pin!(timeout_stream);
            let timeout_result = timeout_stream.try_next().await;
            match timeout_result {
                Ok(Some(value)) => {
                    actual.push(log_msg_from_proto(value.unwrap().log_msg.unwrap()).unwrap());
                }

                // Stream closed | Timed out
                Ok(None) | Err(_) => break,
            }
        }

        // The GC runs _before_ a message is stored, so we should see the static message, and the last message sent.
        assert_eq!(actual.len(), 2);
        assert_eq!(&actual[0], &messages[0]);
        assert_eq!(&actual[1], messages.last().unwrap());

        completion.finish();
    }

    #[tokio::test]
    async fn memory_limit_does_not_drop_blueprint() {
        // Use an absurdly low memory limit to force all messages to be dropped immediately from history
        let (completion, addr) = setup_with_memory_limit(MemoryLimit::from_bytes(1)).await;
        let mut client = make_client(addr).await;
        let messages = fake_log_stream_blueprint(3);

        // Write some messages
        client
            .write_messages(tokio_stream::iter(
                messages
                    .clone()
                    .into_iter()
                    .map(|msg| log_msg_to_proto(msg, Compression::Off).unwrap())
                    .map(|msg| WriteMessagesRequest { log_msg: Some(msg) }),
            ))
            .await
            .unwrap();

        // Start reading
        let mut log_stream = client.read_messages(ReadMessagesRequest {}).await.unwrap();
        let mut actual = vec![];
        loop {
            let timeout_stream = log_stream.get_mut().timeout(Duration::from_millis(100));
            tokio::pin!(timeout_stream);
            let timeout_result = timeout_stream.try_next().await;
            match timeout_result {
                Ok(Some(value)) => {
                    actual.push(log_msg_from_proto(value.unwrap().log_msg.unwrap()).unwrap());
                }

                // Stream closed | Timed out
                Ok(None) | Err(_) => break,
            }
        }

        // The stream in this case only contains SetStoreInfo, ArrowMsg with StoreKind::Blueprint,
        // and BlueprintActivationCommand. None of these things should be GC'd:
        assert_eq!(messages, actual);

        completion.finish();
    }

    #[tokio::test]
    async fn memory_limit_does_not_interrupt_stream() {
        // Use an absurdly low memory limit to force all messages to be dropped immediately from history
        let (completion, addr) = setup_with_memory_limit(MemoryLimit::from_bytes(1)).await;
        let mut client = make_client(addr).await; // We use the same client for both producing and consuming
        let messages = fake_log_stream_blueprint(3);

        // Start reading
        let mut log_stream = client.read_messages(ReadMessagesRequest {}).await.unwrap();

        // Write a few messages
        client
            .write_messages(tokio_stream::iter(
                messages
                    .clone()
                    .into_iter()
                    .map(|msg| log_msg_to_proto(msg, Compression::Off).unwrap())
                    .map(|msg| WriteMessagesRequest { log_msg: Some(msg) }),
            ))
            .await
            .unwrap();

        // The messages should be echoed to us, even though none of them will be stored in history
        let actual = read_log_stream(&mut log_stream, messages.len()).await;
        assert_eq!(messages, actual);

        completion.finish();
    }
}
