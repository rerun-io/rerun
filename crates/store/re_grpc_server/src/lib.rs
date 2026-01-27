//! Server for the legacy `StoreHub` API.

pub mod shutdown;

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::pin::Pin;

use re_byte_size::SizeBytes;
use re_log_channel::{DataSourceMessage, DataSourceUiCommand};
use re_log_encoding::{ToApplication as _, ToTransport as _};
use re_log_types::TableMsg;
use re_protos::common::v1alpha1::{
    DataframePart as DataframePartProto, StoreKind as StoreKindProto, TableId as TableIdProto,
};
use re_protos::log_msg::v1alpha1::LogMsg as LogMsgProto;
use re_protos::sdk_comms::v1alpha1::{
    ReadMessagesRequest, ReadMessagesResponse, ReadTablesRequest, ReadTablesResponse,
    SaveScreenshotRequest, SaveScreenshotResponse, WriteMessagesRequest, WriteMessagesResponse,
    WriteTableRequest, WriteTableResponse, message_proxy_service_server,
};
use re_quota_channel::{async_broadcast_channel, async_mpsc_channel};
use std::task::{Context, Poll};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_stream::{Stream, StreamExt as _};
use tonic::transport::Server;
use tonic::transport::server::TcpIncoming;
use tower_http::cors::CorsLayer;

use crate::priority_stream::PriorityMerge;

mod priority_stream;

pub use re_memory::MemoryLimit;

/// Default port of the OSS /proxy server.
pub const DEFAULT_SERVER_PORT: u16 = 9876;

pub const MAX_DECODING_MESSAGE_SIZE: usize = u32::MAX as usize;
pub const MAX_ENCODING_MESSAGE_SIZE: usize = MAX_DECODING_MESSAGE_SIZE;

/// Options for the gRPC Proxy Server
#[derive(Clone, Copy, Debug)]
pub struct ServerOptions {
    /// When a client connect, should they be sent the oldest data first, or the newest?
    pub playback_behavior: PlaybackBehavior,

    /// Start garbage collecting old data when we reach this.
    pub memory_limit: MemoryLimit,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            playback_behavior: PlaybackBehavior::OldestFirst,
            memory_limit: MemoryLimit::from_bytes(1024 * 1024 * 1024), // Be very conservative by default
        }
    }
}

/// What happens when a client connects to a gRPC server?
#[derive(Clone, Copy, Debug)]
pub enum PlaybackBehavior {
    /// Start playing back all the old data first,
    /// and only after start sending anything that happened since.
    OldestFirst,

    /// Prioritize the newest arriving messages,
    /// replaying the history later, starting with the newest.
    NewestFirst,
}

impl PlaybackBehavior {
    pub fn from_newest_first(newest_first: bool) -> Self {
        if newest_first {
            Self::NewestFirst
        } else {
            Self::OldestFirst
        }
    }
}

/// Wrapper with a nicer error message
#[derive(Debug)]
pub struct TonicStatusError(pub tonic::Status);

impl std::fmt::Display for TonicStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO(emilk): duplicated in `re_grpc_client`
        let status = &self.0;

        write!(f, "gRPC error")?;

        if status.code() != tonic::Code::Unknown {
            write!(f, ", code: '{}'", status.code())?;
        }
        if !status.message().is_empty() {
            write!(f, ", message: {:?}", status.message())?;
        }
        // Binary data - not useful.
        // if !status.details().is_empty() {
        //     write!(f, ", details: {:?}", status.details())?;
        // }
        if !status.metadata().is_empty() {
            write!(f, ", metadata: {:?}", status.metadata().as_ref())?;
        }
        Ok(())
    }
}

impl From<tonic::Status> for TonicStatusError {
    fn from(value: tonic::Status) -> Self {
        Self(value)
    }
}

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
    options: ServerOptions,
    shutdown: shutdown::Shutdown,
) -> anyhow::Result<()> {
    serve_impl(addr, options, MessageProxy::new(options), shutdown).await
}

async fn serve_impl(
    addr: SocketAddr,
    options: ServerOptions,
    message_proxy: MessageProxy,
    shutdown: shutdown::Shutdown,
) -> anyhow::Result<()> {
    // TODO(rust-lang/rust#130668): When listening on `::` we want to listen to both ipv6 `::` and ipv4 `0.0.0.0`
    // On Mac & Linux this happens automatically since all sockets are dual-stack by default.
    // On Windows, the dual stack behavior is opt-in, but `TcpListener::bind` does not expose the option.
    // To work around this, we explicitly listen on both ipv4 & ipv6 if an unspecified ipv6 address is used.
    let dual_stack_windows = cfg!(target_os = "windows")
        && matches!(addr.ip(), std::net::IpAddr::V6(ipv6) if ipv6.is_unspecified());

    let incoming: Pin<Box<dyn Stream<Item = _> + Send>> = if dual_stack_windows {
        let ipv6_addr = addr;
        let ipv4_addr = SocketAddr::V4(std::net::SocketAddrV4::new(
            std::net::Ipv4Addr::UNSPECIFIED,
            addr.port(),
        ));

        let tcp_listener_ipv6 = TcpListener::bind(ipv6_addr).await?;
        let tcp_listener_ipv4 = TcpListener::bind(ipv4_addr).await?;

        let incoming_ipv6 = TcpIncoming::from(tcp_listener_ipv6).with_nodelay(Some(true));
        let incoming_ipv4 = TcpIncoming::from(tcp_listener_ipv4).with_nodelay(Some(true));

        // Merge both streams into a single stream
        let merged = tokio_stream::StreamExt::merge(incoming_ipv6, incoming_ipv4);

        let connect_addr = format!("rerun+http://127.0.0.1:{}/proxy", addr.port());

        re_log::info!(
            "Listening for gRPC connections on {ipv6_addr} and {ipv4_addr}. Connect by running `rerun --connect {connect_addr}`",
        );

        Box::pin(merged)
    } else {
        let tcp_listener = TcpListener::bind(addr).await?;
        let incoming = TcpIncoming::from(tcp_listener).with_nodelay(Some(true));

        let connect_addr = if addr.ip().is_loopback() || addr.ip().is_unspecified() {
            format!("rerun+http://127.0.0.1:{}/proxy", addr.port())
        } else {
            format!("rerun+http://{addr}/proxy")
        };

        re_log::info!(
            "Listening for gRPC connections on {addr}. Connect by running `rerun --connect {connect_addr}`",
        );

        Box::pin(incoming)
    };

    re_log::info!("Server memory limit set at {}", options.memory_limit);

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
        .await?;

    Ok(())
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
    options: ServerOptions,
    shutdown: shutdown::Shutdown,
    channel_rx: re_log_channel::LogReceiver,
) {
    let message_proxy = MessageProxy::new(options);
    let event_tx = message_proxy.event_tx.clone();

    tokio::task::spawn_blocking(move || {
        use re_log_channel::SmartMessagePayload;

        loop {
            let msg = if let Ok(msg) = channel_rx.recv() {
                match msg.payload {
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
                }
            } else {
                re_log::debug!("smart channel sender closed, closing receiver");
                break;
            };

            match msg {
                DataSourceMessage::LogMsg(msg) => {
                    let msg = match msg.to_transport(re_log_encoding::rrd::Compression::LZ4) {
                        Ok(msg) => msg,
                        Err(err) => {
                            re_log::error!("failed to encode message: {err}");
                            continue;
                        }
                    };

                    if event_tx
                        .blocking_send(Event::Message(LogOrTableMsgProto::LogMsg(msg.into())))
                        .is_err()
                    {
                        re_log::debug!("shut down, closing sender");
                        break;
                    }
                }
                unsupported => {
                    re_log::error_once!(
                        "Not implemented: re_grpc_server support for {}",
                        unsupported.variant_name()
                    );
                }
            }
        }
    });

    if let Err(err) = serve_impl(addr, options, message_proxy, shutdown).await {
        re_log::error!("message proxy server crashed: {err}");
    }
}

/// Start a Rerun server, listening on `addr`.
///
/// This function additionally accepts a [`re_log_channel::LogReceiverSet`], from which the
/// server will read all messages. It is similar to creating a client
/// and sending messages through `WriteMessages`, but without the overhead
/// of a localhost connection.
///
/// See [`serve`] for more information about what a Rerun server is.
pub fn spawn_from_rx_set(
    addr: SocketAddr,
    options: ServerOptions,
    shutdown: shutdown::Shutdown,
    rxs: re_log_channel::LogReceiverSet,
) {
    let message_proxy = MessageProxy::new(options);
    let event_tx = message_proxy.event_tx.clone();

    tokio::spawn(async move {
        if let Err(err) = serve_impl(addr, options, message_proxy, shutdown).await {
            re_log::error!("message proxy server crashed: {err}");
        }
    });

    tokio::task::spawn_blocking(move || {
        use re_log_channel::SmartMessagePayload;

        loop {
            let msg = if let Ok(msg) = rxs.recv() {
                match msg.payload {
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
                        if rxs.is_empty() {
                            // We won't ever receive more data:
                            break;
                        }
                        continue;
                    }
                }
            } else {
                if rxs.is_empty() {
                    // We won't ever receive more data:
                    break;
                }
                continue;
            };

            match msg {
                DataSourceMessage::LogMsg(msg) => {
                    let msg = match msg.to_transport(re_log_encoding::rrd::Compression::LZ4) {
                        Ok(msg) => msg,
                        Err(err) => {
                            re_log::error!("failed to encode message: {err}");
                            continue;
                        }
                    };

                    if event_tx
                        .blocking_send(Event::Message(LogOrTableMsgProto::LogMsg(msg.into())))
                        .is_err()
                    {
                        re_log::debug!("shut down, closing sender");
                        break;
                    }
                }
                unsupported => {
                    re_log::error_once!(
                        "gRPC proxy server cannot forward {}",
                        unsupported.variant_name()
                    );
                }
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
    options: ServerOptions,
    shutdown: shutdown::Shutdown,
) -> re_log_channel::LogReceiver {
    let uri = re_uri::ProxyUri::new(re_uri::Origin::from_scheme_and_socket_addr(
        re_uri::Scheme::RerunHttp,
        addr,
    ));

    let (channel_log_tx, channel_log_rx) =
        re_log_channel::log_channel(re_log_channel::LogSource::MessageProxy(uri));

    let (message_proxy, mut broadcast_log_rx) = MessageProxy::new_with_recv(options);

    tokio::spawn(async move {
        if let Err(err) = serve_impl(addr, options, message_proxy, shutdown).await {
            re_log::error!("message proxy server crashed: {err}");
        }
    });

    tokio::spawn(async move {
        let mut app_id_cache = re_log_encoding::CachingApplicationIdInjector::default();

        loop {
            let msg: anyhow::Result<DataSourceMessage> = match broadcast_log_rx.recv().await {
                Ok(inner) => match inner {
                    LogOrTableMsgProto::LogMsg(msg) => match msg.msg {
                        Some(msg) => msg
                            .to_application((&mut app_id_cache, None))
                            .map(DataSourceMessage::LogMsg)
                            .map_err(|err| err.into()),
                        None => Err(re_protos::missing_field!(
                            re_protos::log_msg::v1alpha1::LogMsg,
                            "msg"
                        )
                        .into()),
                    },

                    LogOrTableMsgProto::Table(msg) => match msg.data.try_into() {
                        Ok(data) => Ok(DataSourceMessage::TableMsg(TableMsg {
                            id: msg.id.into(),
                            data,
                        })),
                        Err(err) => {
                            re_log::error!("Dropping LogMsg::Table due to failed decode: {err}");
                            continue;
                        }
                    },

                    LogOrTableMsgProto::UiCommand(cmd) => Ok(DataSourceMessage::UiCommand(cmd)),
                },

                Err(async_broadcast_channel::RecvError::Closed) => {
                    re_log::debug!("message proxy server shut down, closing receiver");
                    channel_log_tx.quit(None).ok();
                    break;
                }
            };
            match msg {
                Ok(mut log_msg) => {
                    if let Some(metadata_key) =
                        re_sorbet::TimestampLocation::IPCDecode.metadata_key()
                    {
                        // Insert the timestamp metadata into the Arrow message for accurate e2e latency measurements.
                        // Note that this function is only called by the viewer
                        // (that's what the message-receiver is connected to).
                        log_msg.insert_arrow_record_batch_metadata(
                            metadata_key.to_owned(),
                            re_sorbet::timestamp_metadata::now_timestamp(),
                        );
                    }

                    if channel_log_tx.send(log_msg).is_err() {
                        re_log::debug!(
                            "message proxy smart channel receiver closed, closing sender"
                        );
                        break;
                    }
                }
                Err(err) => {
                    re_log::error!("dropping LogMsg due to failed decode: {err}");
                }
            }
        }
    });

    channel_log_rx
}

enum Event {
    /// New client connected, requesting full history and subscribing to new messages.
    NewClient(
        oneshot::Sender<(
            Vec<LogOrTableMsgProto>,
            async_broadcast_channel::Receiver<LogOrTableMsgProto>,
        )>,
    ),

    /// A client sent a message.
    Message(LogOrTableMsgProto),
}

#[derive(Clone)]
struct TableMsgProto {
    id: TableIdProto,
    data: DataframePartProto,
}
// -----------------------------------------------------------------------------------

#[derive(Clone)]
enum LogOrTableMsgProto {
    LogMsg(LogMsgProto),
    Table(TableMsgProto),
    UiCommand(DataSourceUiCommand),
}

impl SizeBytes for LogOrTableMsgProto {
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Self::LogMsg(log_msg) => log_msg.heap_size_bytes(),
            Self::Table(table) => table.heap_size_bytes(),
            Self::UiCommand(cmd) => cmd.heap_size_bytes(),
        }
    }
}

impl From<LogMsgProto> for LogOrTableMsgProto {
    fn from(value: LogMsgProto) -> Self {
        Self::LogMsg(value)
    }
}

impl From<TableMsgProto> for LogOrTableMsgProto {
    fn from(value: TableMsgProto) -> Self {
        Self::Table(value)
    }
}

impl From<DataSourceUiCommand> for LogOrTableMsgProto {
    fn from(value: DataSourceUiCommand) -> Self {
        Self::UiCommand(value)
    }
}

// -----------------------------------------------------------------------------------

#[derive(Default)]
struct MsgQueue {
    /// Messages stored in order of arrival, and garbage collected if the server hits the memory limit.
    queue: VecDeque<LogOrTableMsgProto>,

    /// Total size of [`Self::queue`] in bytes.
    size_bytes: u64,
}

impl MsgQueue {
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &LogOrTableMsgProto> {
        self.queue.iter()
    }

    pub fn push_back(&mut self, msg: LogOrTableMsgProto) {
        self.size_bytes += msg.total_size_bytes();
        self.queue.push_back(msg);
    }

    pub fn pop_front(&mut self) -> Option<LogOrTableMsgProto> {
        if let Some(msg) = self.queue.pop_front() {
            self.size_bytes -= msg.total_size_bytes();
            Some(msg)
        } else {
            None
        }
    }
}

// -----------------------------------------------------------------------------------

/// Contains all messages received so far,
/// minus some that are garbage collected when needed.
#[derive(Default)]
struct MessageBuffer {
    /// Normal data messages.
    ///
    /// First to be garbage collected if we run into the memory limit.
    disposable: MsgQueue,

    /// "Static" (non-temporal) data messages.
    ///
    /// Our chunk-store already keeps static messages forever,
    /// and it makes sense: you usually log them once,
    /// and then expect them to stay around.
    ///
    /// We keep the static messages for as long as we can, but if [`Self::disposable`]
    /// is empty and we're still over our memory budget, we start throwing
    /// away the oldest messages from here too.
    /// This is because some users use static logging for camera images,
    /// which adds up very quickly.
    ///
    /// Ideally we would keep exactly one static message per entity/component stream
    /// (like the `ChunkStore` does), but we'll save that for:
    /// TODO(#5531): replace this with `ChunkStore`
    static_: MsgQueue,

    /// These are never garbage collected.
    persistent: MsgQueue,
}

impl MessageBuffer {
    fn size_bytes(&self) -> u64 {
        let Self {
            disposable,
            static_,
            persistent,
        } = self;
        disposable.size_bytes + static_.size_bytes + persistent.size_bytes
    }

    fn all(&self, playback_behavior: PlaybackBehavior) -> Vec<LogOrTableMsgProto> {
        re_tracing::profile_function!();

        let Self {
            disposable,
            static_,
            persistent,
        } = self;

        // Note: we ALWAYS send the persistent and static data before the disposable,
        // regardless of PlaybackBehavior!

        match playback_behavior {
            PlaybackBehavior::OldestFirst => {
                itertools::chain!(persistent.iter(), static_.iter(), disposable.iter())
                    .cloned()
                    .collect()
            }
            PlaybackBehavior::NewestFirst => itertools::chain!(
                persistent.iter().rev(),
                static_.iter().rev(),
                disposable.iter().rev()
            )
            .cloned()
            .collect(),
        }
    }

    fn add_msg(&mut self, msg: LogOrTableMsgProto) {
        match msg {
            LogOrTableMsgProto::LogMsg(msg) => self.add_log_msg(msg),
            LogOrTableMsgProto::Table(msg) => {
                self.disposable.push_back(msg.into());
            }
            LogOrTableMsgProto::UiCommand(msg) => {
                self.disposable.push_back(msg.into());
            }
        }
    }

    fn add_log_msg(&mut self, msg: LogMsgProto) {
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
                self.persistent.push_back(msg.into());
            }

            Msg::ArrowMsg(inner) => {
                let is_blueprint = inner
                    .store_id
                    .as_ref()
                    .is_some_and(|id| id.kind() == StoreKindProto::Blueprint);

                if is_blueprint {
                    // Persist blueprint messages forever.
                    self.persistent.push_back(msg.into());
                } else if inner.is_static == Some(true) {
                    self.static_.push_back(msg.into());
                } else {
                    // Recording data
                    self.disposable.push_back(msg.into());
                }
            }
        }
    }

    pub fn gc(&mut self, max_bytes: u64) {
        if self.size_bytes() <= max_bytes {
            // We're not using too much memory.
            return;
        }

        re_tracing::profile_scope!("Drop messages");
        re_log::info_once!(
            "Exceeded gRPC proxy server memory limit ({}). Dropping the olddest log messages. Clients connecting after this will not see the full history.",
            re_format::format_bytes(max_bytes as _)
        );

        let start_size = self.size_bytes();
        let mut messages_dropped = 0;

        while self.disposable.pop_front().is_some() {
            messages_dropped += 1;
            if self.size_bytes() < max_bytes {
                break;
            }
        }

        if max_bytes < self.size_bytes() {
            re_log::info_once!(
                "Exceeded gRPC proxy server memory limit ({}). Dropping old *static* log messages as well. Clients connecting after this will no longer see the complete set of static data.",
                re_format::format_bytes(max_bytes as _)
            );
            while self.static_.pop_front().is_some() {
                messages_dropped += 1;
                if self.size_bytes() < max_bytes {
                    break;
                }
            }
        }

        let bytes_dropped = start_size - self.size_bytes();

        re_log::trace!(
            "Dropped {} bytes in {messages_dropped} message(s)",
            re_format::format_bytes(bytes_dropped as _)
        );

        if max_bytes < self.size_bytes() {
            re_log::warn_once!(
                "The gRPC server is using more memory than the given memory limit ({}), despite having garbage-collected all non-persistent messages.",
                re_format::format_bytes(max_bytes as _)
            );
        }
    }
}

// -----------------------------------------------------------------------------------

/// A wrapper that converts an `async_broadcast_channel::Receiver` into a `Stream`.
///
/// This uses `async_stream` internally to bridge the async recv method to Stream.
/// The stream yields the inner value (unwrapped from `Tracked`).
struct BackPressureReceiverStream<T: Clone + SizeBytes + Send + Sync + 'static> {
    inner: Pin<Box<dyn Stream<Item = Result<T, async_broadcast_channel::RecvError>> + Send>>,
}

impl<T: Clone + SizeBytes + Send + Sync + 'static> BackPressureReceiverStream<T> {
    fn new(mut receiver: async_broadcast_channel::Receiver<T>) -> Self {
        let stream = async_stream::stream! {
            while let Ok(value) = receiver.recv().await {
                yield Ok(value);
            }
        };
        Self {
            inner: Box::pin(stream),
        }
    }
}

impl<T: Clone + SizeBytes + Send + Sync + 'static> Stream for BackPressureReceiverStream<T> {
    type Item = Result<T, async_broadcast_channel::RecvError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.as_mut().poll_next(cx)
    }
}

// -----------------------------------------------------------------------------------

/// Main event loop for the server, which runs in its own task.
///
/// Handles message history, and broadcasts messages to clients.
struct EventLoop {
    options: ServerOptions,

    /// New log messages are broadcast to all clients.
    /// Uses a back-pressure channel that blocks senders when the byte limit is exceeded.
    broadcast_log_tx: async_broadcast_channel::Sender<LogOrTableMsgProto>,

    /// Channel for incoming events.
    event_rx: async_mpsc_channel::Receiver<Event>,

    /// All messages received so far, minus those that have been garbage collected.
    history: MessageBuffer,
}

impl EventLoop {
    fn new(
        options: ServerOptions,
        event_rx: async_mpsc_channel::Receiver<Event>,
        broadcast_log_tx: async_broadcast_channel::Sender<LogOrTableMsgProto>,
    ) -> Self {
        Self {
            options,
            broadcast_log_tx,
            event_rx,
            history: Default::default(),
        }
    }

    async fn run_in_place(mut self) {
        loop {
            let Some(event) = self.event_rx.recv().await else {
                break;
            };

            match event {
                Event::NewClient(channel) => {
                    channel
                        .send((
                            self.history.all(self.options.playback_behavior),
                            self.broadcast_log_tx.subscribe(),
                        ))
                        .ok();
                }
                Event::Message(msg) => self.handle_msg(msg).await,
            }
        }
    }

    async fn handle_msg(&mut self, msg: LogOrTableMsgProto) {
        // This will block if the broadcast channel is full, applying back-pressure
        self.broadcast_log_tx.send_async(msg.clone()).await.ok();

        if self.is_history_disabled() {
            // no need to gc or maintain history
            return;
        }

        self.gc_if_using_too_much_ram();

        self.history.add_msg(msg);
    }

    fn is_history_disabled(&self) -> bool {
        self.options.memory_limit.max_bytes.is_some_and(|b| b == 0)
    }

    fn gc_if_using_too_much_ram(&mut self) {
        let Some(max_bytes) = self.options.memory_limit.max_bytes else {
            // Unlimited memory!
            return;
        };

        self.history.gc(max_bytes);
    }
}

impl SizeBytes for TableMsgProto {
    fn heap_size_bytes(&self) -> u64 {
        let Self { id, data } = self;
        id.heap_size_bytes() + data.heap_size_bytes()
    }
}

pub struct MessageProxy {
    options: ServerOptions,
    _queue_task_handle: tokio::task::JoinHandle<()>,
    event_tx: async_mpsc_channel::Sender<Event>,
}

impl MessageProxy {
    pub fn new(options: ServerOptions) -> Self {
        Self::new_with_recv(options).0
    }

    fn new_with_recv(
        mut options: ServerOptions,
    ) -> (Self, async_broadcast_channel::Receiver<LogOrTableMsgProto>) {
        // Divide up the memory budget:
        let (broadcast_channel_memory_limit, rest_memory_limit) = options.memory_limit.split(0.25);
        options.memory_limit = rest_memory_limit;

        let (broadcast_log_tx, broadcast_log_rx) = async_broadcast_channel::channel(
            "re_grpc_server broadcast",
            4096,
            broadcast_channel_memory_limit.as_bytes(),
        );

        let (event_tx, event_rx) = {
            let message_queue_capacity = if options.memory_limit == MemoryLimit::ZERO {
                1
            } else {
                16 // Apply backpressure early
            };
            // TODO(emilk): this could also use a size-based backpressure mechanism.

            async_mpsc_channel::channel("re_grpc_server events", message_queue_capacity)
        };

        let task_handle = tokio::spawn(async move {
            EventLoop::new(options, event_rx, broadcast_log_tx)
                .run_in_place()
                .await;
        });

        (
            Self {
                options,
                _queue_task_handle: task_handle,
                event_tx,
            },
            broadcast_log_rx,
        )
    }

    async fn push_message(&self, message: impl Into<LogOrTableMsgProto>) {
        let message = message.into();
        self.event_tx.send(Event::Message(message)).await.ok();
    }

    async fn new_client_message_stream(&self) -> ReadMsgStream {
        let (sender, receiver) = oneshot::channel();
        if let Err(err) = self.event_tx.send(Event::NewClient(sender)).await {
            re_log::error!("Error accepting new client: {err}");
            return Box::pin(tokio_stream::empty());
        }
        let (history, msg_channel) = match receiver.await {
            Ok(v) => v,
            Err(err) => {
                re_log::error!("Error accepting new client: {err}");
                return Box::pin(tokio_stream::empty());
            }
        };

        let history = tokio_stream::iter(
            history
                .into_iter()
                .map(ReadLogOrTableMsgResponse::from)
                .map(Ok),
        );

        // Convert our backpressure receiver into a Stream
        let channel = BackPressureReceiverStream::new(msg_channel).map(|result| {
            result.map(ReadLogOrTableMsgResponse::from).map_err(|err| {
                re_log::error!("Error reading message from broadcast channel: {err}");
                tonic::Status::internal(format!("internal channel error: {err}"))
            })
        });

        match self.options.playback_behavior {
            PlaybackBehavior::OldestFirst => Box::pin(history.chain(channel)),
            PlaybackBehavior::NewestFirst => Box::pin(PriorityMerge::new(channel, history)),
        }
    }

    async fn new_client_log_stream(&self) -> ReadLogStream {
        Box::pin(
            self.new_client_message_stream()
                .await
                .filter_map(|msg| match msg {
                    Ok(ReadLogOrTableMsgResponse::LogMsg(msg)) => Some(Ok(msg)),
                    Ok(ReadLogOrTableMsgResponse::TableMsg(_)) => {
                        re_log::warn_once!("A log stream got a TableMsg");
                        None
                    }
                    Ok(ReadLogOrTableMsgResponse::UiCommand) => {
                        re_log::warn_once!("A log stream got a UiCommandMsg");
                        None
                    }
                    Err(err) => Some(Err(err)),
                }),
        )
    }

    async fn new_client_table_stream(&self) -> ReadTablesStream {
        Box::pin(
            self.new_client_message_stream()
                .await
                .filter_map(|msg| match msg {
                    Ok(ReadLogOrTableMsgResponse::LogMsg(_)) => {
                        re_log::warn_once!("A table stream got a LogMsg");
                        None
                    }
                    Ok(ReadLogOrTableMsgResponse::TableMsg(msg)) => Some(Ok(msg)),
                    Ok(ReadLogOrTableMsgResponse::UiCommand) => {
                        re_log::warn_once!("A log stream got a UiCommandMsg");
                        None
                    }
                    Err(err) => Some(Err(err)),
                }),
        )
    }
}

enum ReadLogOrTableMsgResponse {
    LogMsg(ReadMessagesResponse),
    TableMsg(ReadTablesResponse),
    UiCommand,
}

impl From<LogOrTableMsgProto> for ReadLogOrTableMsgResponse {
    fn from(proto: LogOrTableMsgProto) -> Self {
        match proto {
            LogOrTableMsgProto::LogMsg(log_msg) => Self::LogMsg(ReadMessagesResponse {
                log_msg: Some(log_msg),
            }),
            LogOrTableMsgProto::Table(table_msg) => Self::TableMsg(ReadTablesResponse {
                id: Some(table_msg.id),
                data: Some(table_msg.data),
            }),
            LogOrTableMsgProto::UiCommand(_ui_command) => Self::UiCommand,
        }
    }
}

type ReadLogStream = Pin<Box<dyn Stream<Item = tonic::Result<ReadMessagesResponse>> + Send>>;
type ReadTablesStream = Pin<Box<dyn Stream<Item = tonic::Result<ReadTablesResponse>> + Send>>;

type ReadMsgStream = Pin<Box<dyn Stream<Item = tonic::Result<ReadLogOrTableMsgResponse>> + Send>>;

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
                    self.push_message(log_msg).await;
                }

                Ok(Some(WriteMessagesRequest { log_msg: None })) => {
                    re_log::warn!("missing log_msg in WriteMessagesRequest");
                }

                Ok(None) => {
                    // Connection was closed
                    break;
                }

                Err(err) => {
                    re_log::error!("Error while receiving messages: {}", TonicStatusError(err));
                    break;
                }
            }
        }

        Ok(tonic::Response::new(WriteMessagesResponse {}))
    }

    type ReadMessagesStream = ReadLogStream;

    async fn read_messages(
        &self,
        _: tonic::Request<ReadMessagesRequest>,
    ) -> tonic::Result<tonic::Response<Self::ReadMessagesStream>> {
        Ok(tonic::Response::new(self.new_client_log_stream().await))
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
            self.push_message(TableMsgProto { id, data }).await;
        } else {
            re_log::warn!("malformed `WriteTableRequest`");
        }

        Ok(tonic::Response::new(WriteTableResponse {}))
    }

    async fn read_tables(
        &self,
        _: tonic::Request<ReadTablesRequest>,
    ) -> tonic::Result<tonic::Response<Self::ReadTablesStream>> {
        Ok(tonic::Response::new(self.new_client_table_stream().await))
    }

    async fn save_screenshot(
        &self,
        request: tonic::Request<SaveScreenshotRequest>,
    ) -> tonic::Result<tonic::Response<SaveScreenshotResponse>> {
        let SaveScreenshotRequest { view_id, file_path } = request.into_inner();
        self.push_message(DataSourceUiCommand::SaveScreenshot {
            file_path: file_path.into(),
            view_id,
        })
        .await;

        Ok(tonic::Response::new(SaveScreenshotResponse {}))
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::sync::Arc;
    use std::time::Duration;

    use itertools::{Itertools as _, chain};
    use re_chunk::RowId;
    use re_log_encoding::rrd::Compression;
    use re_log_types::{LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource};
    use re_protos::sdk_comms::v1alpha1::message_proxy_service_client::MessageProxyServiceClient;
    use re_protos::sdk_comms::v1alpha1::message_proxy_service_server::MessageProxyServiceServer;
    use similar_asserts::assert_eq;
    use tokio::net::TcpListener;
    use tokio_util::sync::CancellationToken;
    use tonic::transport::server::TcpIncoming;
    use tonic::transport::{Channel, Endpoint};

    use super::*;

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

    fn set_store_info_msg(store_id: &StoreId) -> LogMsg {
        LogMsg::SetStoreInfo(SetStoreInfo {
            row_id: *RowId::new(),
            info: StoreInfo::new(
                store_id.clone(),
                StoreSource::RustSdk {
                    rustc_version: String::new(),
                    llvm_version: String::new(),
                },
            ),
        })
    }

    /// Generates `n` log messages wrapped in a `SetStoreInfo` at the start and `BlueprintActivationCommand` at the end,
    /// to exercise message ordering.
    fn fake_log_stream_blueprint(n: usize) -> Vec<LogMsg> {
        let store_id = StoreId::random(StoreKind::Blueprint, "test_app");

        let mut messages = Vec::new();
        messages.push(set_store_info_msg(&store_id));
        for _ in 0..n {
            messages.push(LogMsg::ArrowMsg(
                store_id.clone(),
                re_chunk::Chunk::builder("test_entity")
                    .with_archetype(
                        re_chunk::RowId::new(),
                        re_log_types::TimePoint::default().with(
                            re_log_types::Timeline::new_sequence("blueprint"),
                            re_log_types::TimeInt::from_millis(re_log_types::NonMinI64::MIN),
                        ),
                        &re_sdk_types::blueprint::archetypes::Background::new(
                            re_sdk_types::blueprint::components::BackgroundKind::SolidColor,
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

    #[derive(Clone, Copy)]
    enum Temporalness {
        Static,
        Temporal,
    }

    fn fake_log_stream_recording(n: usize) -> Vec<LogMsg> {
        let store_id = StoreId::random(StoreKind::Recording, "test_app");

        chain!(
            [set_store_info_msg(&store_id)],
            generate_log_messages(&store_id, n, Temporalness::Temporal)
        )
        .collect()
    }

    fn generate_log_messages(
        store_id: &StoreId,
        n: usize,
        temporalness: Temporalness,
    ) -> Vec<LogMsg> {
        let mut messages = Vec::new();
        for _ in 0..n {
            let timepoint = match temporalness {
                Temporalness::Static => re_log_types::TimePoint::STATIC,
                Temporalness::Temporal => re_log_types::TimePoint::default().with(
                    re_log_types::Timeline::new_sequence("log_time"),
                    re_log_types::TimeInt::from_millis(re_log_types::NonMinI64::MIN),
                ),
            };

            messages.push(LogMsg::ArrowMsg(
                store_id.clone(),
                re_chunk::Chunk::builder("test_entity")
                    .with_archetype(
                        re_chunk::RowId::new(),
                        timepoint,
                        &re_sdk_types::archetypes::Points2D::new([
                            (0.0, 0.0),
                            (1.0, 1.0),
                            (2.0, 2.0),
                        ]),
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
        setup_opt(ServerOptions {
            playback_behavior: PlaybackBehavior::OldestFirst,
            memory_limit: MemoryLimit::UNLIMITED,
        })
        .await
    }

    async fn setup_with_memory_limit(memory_limit: MemoryLimit) -> (Completion, SocketAddr) {
        setup_opt(ServerOptions {
            playback_behavior: PlaybackBehavior::OldestFirst,
            memory_limit,
        })
        .await
    }

    async fn setup_opt(options: ServerOptions) -> (Completion, SocketAddr) {
        let completion = Completion::new();

        let tcp_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = tcp_listener.local_addr().unwrap();

        tokio::spawn({
            let completion = completion.clone();
            async move {
                tonic::transport::Server::builder()
                    // NOTE: This NODELAY very likely does nothing because of the call to
                    // `serve_with_incoming_shutdown` below, but we better be on the defensive here so
                    // we don't get surprised when things inevitably change.
                    .tcp_nodelay(true)
                    .accept_http1(true)
                    .http2_adaptive_window(Some(true)) // Optimize for throughput
                    .add_service(
                        MessageProxyServiceServer::new(super::MessageProxy::new(options))
                            .max_decoding_message_size(MAX_DECODING_MESSAGE_SIZE)
                            .max_encoding_message_size(MAX_ENCODING_MESSAGE_SIZE),
                    )
                    .serve_with_incoming_shutdown(
                        TcpIncoming::from(tcp_listener).with_nodelay(Some(true)),
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
        .max_decoding_message_size(crate::MAX_DECODING_MESSAGE_SIZE)
    }

    async fn write_messages(
        client: &mut MessageProxyServiceClient<Channel>,
        messages: Vec<LogMsg>,
    ) {
        client
            .write_messages(tokio_stream::iter(
                messages
                    .clone()
                    .into_iter()
                    .map(|msg| msg.to_transport(Compression::Off).unwrap())
                    .map(|msg| WriteMessagesRequest {
                        log_msg: Some(msg.into()),
                    }),
            ))
            .await
            .unwrap();
    }

    async fn read_log_stream(
        log_stream: &mut tonic::Response<tonic::Streaming<ReadMessagesResponse>>,
        n: usize,
    ) -> Vec<LogMsg> {
        let mut app_id_cache = re_log_encoding::CachingApplicationIdInjector::default();

        let mut stream_ref = log_stream.get_mut().map(|result| {
            let msg = result.unwrap().log_msg.unwrap().msg.unwrap();
            msg.to_application((&mut app_id_cache, None)).unwrap()
        });

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

        write_messages(&mut client, messages.clone()).await;

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

        write_messages(&mut client, messages.clone()).await;

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

        write_messages(&mut producer, messages.clone()).await;

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
            write_messages(producer, messages.clone()).await;
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

        write_messages(&mut client, messages.clone()).await;

        // Start reading
        let mut log_stream = client.read_messages(ReadMessagesRequest {}).await.unwrap();
        let mut actual = vec![];
        loop {
            let timeout_stream = log_stream.get_mut().timeout(Duration::from_millis(100));
            tokio::pin!(timeout_stream);
            let timeout_result = timeout_stream.try_next().await;
            let mut app_id_cache = re_log_encoding::CachingApplicationIdInjector::default();
            match timeout_result {
                Ok(Some(value)) => {
                    let msg = value.unwrap().log_msg.unwrap().msg.unwrap();
                    actual.push(msg.to_application((&mut app_id_cache, None)).unwrap());
                }

                // Stream closed | Timed out
                Ok(None) | Err(_) => break,
            }
        }

        // The GC runs _before_ a message is stored, so we should see the persistent message, and the last message sent.
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
        write_messages(&mut client, messages.clone()).await;

        // Start reading
        let mut log_stream = client.read_messages(ReadMessagesRequest {}).await.unwrap();
        let mut actual = vec![];
        loop {
            let timeout_stream = log_stream.get_mut().timeout(Duration::from_millis(100));
            tokio::pin!(timeout_stream);
            let timeout_result = timeout_stream.try_next().await;
            let mut app_id_cache = re_log_encoding::CachingApplicationIdInjector::default();
            match timeout_result {
                Ok(Some(value)) => {
                    let msg = value.unwrap().log_msg.unwrap().msg.unwrap();
                    actual.push(msg.to_application((&mut app_id_cache, None)).unwrap());
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
        let memory_limits = [
            0, // Will actually disable the message buffer and GC logic. Good to test that!
            1, // An absurdly low memory limit to force all messages to be dropped immediately from history
        ];

        for memory_limit in memory_limits {
            let (completion, addr) =
                setup_with_memory_limit(MemoryLimit::from_bytes(memory_limit)).await;
            let mut client = make_client(addr).await; // We use the same client for both producing and consuming
            let messages = fake_log_stream_blueprint(3);

            // Start reading
            let mut log_stream = client.read_messages(ReadMessagesRequest {}).await.unwrap();

            write_messages(&mut client, messages.clone()).await;

            // The messages should be echoed to us, even though none of them will be stored in history
            let actual = read_log_stream(&mut log_stream, messages.len()).await;
            assert_eq!(messages, actual);

            completion.finish();
        }
    }

    #[tokio::test]
    async fn static_data_is_returned_first() {
        let (completion, addr) = setup_with_memory_limit(MemoryLimit::UNLIMITED).await;
        let mut client = make_client(addr).await;

        let store_id = StoreId::random(StoreKind::Recording, "test_app");

        let set_store_info = vec![set_store_info_msg(&store_id)];
        let first_static = generate_log_messages(&store_id, 3, Temporalness::Static);
        let first_temporal = generate_log_messages(&store_id, 3, Temporalness::Temporal);
        let second_static = generate_log_messages(&store_id, 3, Temporalness::Static);

        write_messages(&mut client, set_store_info.clone()).await;
        write_messages(&mut client, first_static.clone()).await;
        write_messages(&mut client, first_temporal.clone()).await;
        write_messages(&mut client, second_static.clone()).await;

        // All static data should always come before temporal data:
        let expected =
            itertools::chain!(set_store_info, first_static, second_static, first_temporal)
                .collect_vec();

        let mut log_stream = client.read_messages(ReadMessagesRequest {}).await.unwrap();
        let actual = read_log_stream(&mut log_stream, expected.len()).await;

        assert_eq!(actual, expected);

        completion.finish();
    }

    #[tokio::test]
    async fn playback_newest_first() {
        let (completion, addr) = setup_opt(ServerOptions {
            playback_behavior: PlaybackBehavior::NewestFirst, // this is what we want to test
            memory_limit: MemoryLimit::UNLIMITED,
        })
        .await;
        let mut client = make_client(addr).await;

        let store_id = StoreId::random(StoreKind::Recording, "test_app");

        let set_store_info = vec![set_store_info_msg(&store_id)];
        let first_statics = generate_log_messages(&store_id, 3, Temporalness::Static);
        let temporals = generate_log_messages(&store_id, 3, Temporalness::Temporal);
        let second_statics = generate_log_messages(&store_id, 3, Temporalness::Static);

        write_messages(&mut client, set_store_info.clone()).await;
        write_messages(&mut client, first_statics.clone()).await;
        write_messages(&mut client, temporals.clone()).await;
        write_messages(&mut client, second_statics.clone()).await;

        // All static data should always come before temporal data:
        let expected = itertools::chain!(
            set_store_info.into_iter().rev(),
            second_statics.into_iter().rev(),
            first_statics.into_iter().rev(),
            temporals.into_iter().rev()
        )
        .collect_vec();

        let mut log_stream = client.read_messages(ReadMessagesRequest {}).await.unwrap();
        let actual = read_log_stream(&mut log_stream, expected.len()).await;

        assert_eq!(actual, expected);

        completion.finish();
    }
}
