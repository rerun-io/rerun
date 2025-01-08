use std::collections::VecDeque;
use std::pin::Pin;

use prost::Message as _;
use re_memory::MemoryLimit;
use re_protos::{
    log_msg::v0::LogMsg as LogMsgProto,
    sdk_comms::v0::{message_proxy_server, Empty},
};
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::Stream;
use tokio_stream::StreamExt as _;

enum Event {
    /// New client connected, requesting full history and subscribing to new messages.
    NewClient(oneshot::Sender<(Vec<LogMsgProto>, broadcast::Receiver<LogMsgProto>)>),

    /// A client sent a message.
    Message(LogMsgProto),
}

struct QueueState {
    server_memory_limit: MemoryLimit,

    /// New messages are broadcast to all clients.
    broadcast_tx: broadcast::Sender<LogMsgProto>,

    /// Channel for incoming events.
    event_rx: mpsc::Receiver<Event>,

    /// Messages stored in order of arrival, and garbage collected if the server hits the memory limit.
    temporal_message_queue: VecDeque<LogMsgProto>,

    /// Total size of buffered messages in bytes.
    ///
    /// Excludes size of `commands`.
    message_bytes: u64,

    /// Messages potentially out of order with the rest of the message stream. These are never garbage collected.
    static_message_queue: VecDeque<LogMsgProto>,
}

impl QueueState {
    fn new(server_memory_limit: MemoryLimit, event_rx: mpsc::Receiver<Event>) -> Self {
        Self {
            server_memory_limit,
            broadcast_tx: broadcast::channel(1024).0,
            event_rx,
            temporal_message_queue: Default::default(),
            message_bytes: 0,
            static_message_queue: Default::default(),
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
            }
        }
    }

    fn handle_new_client(
        &self,
        channel: oneshot::Sender<(Vec<LogMsgProto>, broadcast::Receiver<LogMsgProto>)>,
    ) {
        channel
            .send((
                // static messages come first
                self.static_message_queue
                    .iter()
                    .cloned()
                    .chain(self.temporal_message_queue.iter().cloned())
                    .collect(),
                self.broadcast_tx.subscribe(),
            ))
            .ok();
    }

    fn handle_msg(&mut self, msg: LogMsgProto) {
        self.broadcast_tx.send(msg.clone()).ok();

        self.gc_if_using_too_much_ram();

        let Some(inner) = &msg.msg else {
            re_log::error!(
                "{}",
                re_protos::missing_field!(re_protos::log_msg::v0::LogMsg, "msg")
            );
            return;
        };

        use re_protos::log_msg::v0::log_msg::Msg;
        match inner {
            // We consider `BlueprintActivationCommand` a temporal message,
            // because it is sensitive to order, and it is safe to garbage collect
            // if all the messages that came before it were also garbage collected,
            // as it's the last message sent by the SDK when submitting a blueprint.
            Msg::ArrowMsg(..) | Msg::BlueprintActivationCommand(..) => {
                let approx_size_bytes = message_size(&msg);
                self.message_bytes += approx_size_bytes;
                self.temporal_message_queue.push_back(msg);
            }
            Msg::SetStoreInfo(..) => {
                self.static_message_queue.push_back(msg);
            }
        }
    }

    fn gc_if_using_too_much_ram(&mut self) {
        re_tracing::profile_function!();

        if let Some(max_bytes) = self.server_memory_limit.max_bytes {
            let max_bytes = max_bytes as u64;
            if max_bytes < self.message_bytes {
                re_tracing::profile_scope!("Drop messages");
                re_log::info_once!(
                    "Memory limit ({}) exceeded. Dropping old log messages from the server. Clients connecting after this will not see the full history.",
                    re_format::format_bytes(max_bytes as _)
                );

                let bytes_to_free = self.message_bytes - max_bytes;

                let mut bytes_dropped = 0;
                let mut messages_dropped = 0;

                while bytes_dropped < bytes_to_free {
                    // only drop messages from temporal queue
                    if let Some(msg) = self.temporal_message_queue.pop_front() {
                        bytes_dropped += message_size(&msg);
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
}

fn message_size(msg: &LogMsgProto) -> u64 {
    // TODO(jan): don't use encoded len
    msg.encoded_len() as u64
}

struct Queue {
    _task_handle: tokio::task::JoinHandle<()>,
    event_tx: mpsc::Sender<Event>,
}

impl Queue {
    fn spawn(server_memory_limit: MemoryLimit) -> Self {
        let (event_tx, event_rx) = mpsc::channel(1024);

        let task_handle = tokio::spawn(async move {
            QueueState::new(server_memory_limit, event_rx)
                .run_in_place()
                .await;
        });

        Self {
            _task_handle: task_handle,
            event_tx,
        }
    }

    async fn push(&self, msg: LogMsgProto) {
        self.event_tx.send(Event::Message(msg)).await.ok();
    }

    async fn new_client_stream(&self) -> MessageStream {
        let (sender, receiver) = oneshot::channel();
        if let Err(err) = self.event_tx.send(Event::NewClient(sender)).await {
            re_log::error!("Error initializing new client: {err}");
            return Box::pin(tokio_stream::empty());
        };
        let (history, channel) = match receiver.await {
            Ok(v) => v,
            Err(err) => {
                re_log::error!("Error initializing new client: {err}");
                return Box::pin(tokio_stream::empty());
            }
        };

        let history = tokio_stream::iter(history.into_iter().map(Ok));
        let channel = BroadcastStream::new(channel).map(|result| {
            result.map_err(|err| {
                re_log::error!("Error reading message from broadcast channel: {err}");
                tonic::Status::internal("internal channel error")
            })
        });

        Box::pin(history.merge(channel))
    }
}

pub struct MessageProxy {
    queue: Queue,
}

impl MessageProxy {
    pub fn new(server_memory_limit: MemoryLimit) -> Self {
        Self {
            queue: Queue::spawn(server_memory_limit),
        }
    }
}

type MessageStream = Pin<Box<dyn Stream<Item = tonic::Result<LogMsgProto>> + Send>>;

#[tonic::async_trait]
impl message_proxy_server::MessageProxy for MessageProxy {
    async fn write_messages(
        &self,
        request: tonic::Request<tonic::Streaming<LogMsgProto>>,
    ) -> tonic::Result<tonic::Response<Empty>> {
        let mut stream = request.into_inner();
        loop {
            match stream.message().await {
                Ok(Some(msg)) => {
                    self.queue.push(msg).await;
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

        Ok(tonic::Response::new(Empty {}))
    }

    type ReadMessagesStream = MessageStream;

    async fn read_messages(
        &self,
        _: tonic::Request<Empty>,
    ) -> tonic::Result<tonic::Response<Self::ReadMessagesStream>> {
        Ok(tonic::Response::new(self.queue.new_client_stream().await))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use message_proxy_server::MessageProxy;
    use re_build_info::CrateVersion;
    use re_chunk::RowId;
    use re_log_encoding::protobuf_conversions::{log_msg_from_proto, log_msg_to_proto};
    use re_log_encoding::Compression;
    use re_log_types::{
        ApplicationId, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource, Time,
    };
    use re_protos::log_msg;
    use re_protos::sdk_comms::v0::{
        message_proxy_client::MessageProxyClient, message_proxy_server::MessageProxyServer,
    };
    use std::net::SocketAddr;
    use std::sync::Arc;
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
    fn fake_log_stream(n: usize) -> Vec<LogMsg> {
        let store_id = StoreId::random(StoreKind::Blueprint);

        let mut messages = Vec::new();
        messages.push(LogMsg::SetStoreInfo(SetStoreInfo {
            row_id: *RowId::new(),
            info: StoreInfo {
                application_id: ApplicationId("test".to_owned()),
                store_id: store_id.clone(),
                cloned_from: None,
                is_official_example: true,
                started: Time::now(),
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
                            re_log_types::TimeInt::from_milliseconds(re_log_types::NonMinI64::MIN),
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

    async fn setup() -> (Completion, SocketAddr) {
        let completion = Completion::new();

        let tcp_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = tcp_listener.local_addr().unwrap();

        tokio::spawn({
            let completion = completion.clone();
            async move {
                tonic::transport::Server::builder()
                    .add_service(MessageProxyServer::new(super::MessageProxy::new(
                        MemoryLimit::UNLIMITED,
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

    async fn make_client(addr: SocketAddr) -> MessageProxyClient<Channel> {
        MessageProxyClient::new(
            Endpoint::from_shared(format!("http://{}", addr))
                .unwrap()
                .connect()
                .await
                .unwrap(),
        )
    }

    async fn read_log_stream(
        log_stream: &mut tonic::Response<tonic::Streaming<LogMsgProto>>,
        n: usize,
    ) -> Vec<LogMsg> {
        let mut stream_ref = log_stream
            .get_mut()
            .map(|result| log_msg_from_proto(result.unwrap()).unwrap());

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
        let messages = fake_log_stream(3);

        // start reading
        let mut log_stream = client.read_messages(Empty {}).await.unwrap();

        // write a few messages
        client
            .write_messages(tokio_stream::iter(
                messages
                    .clone()
                    .into_iter()
                    .map(|msg| log_msg_to_proto(msg, Compression::Off).unwrap()),
            ))
            .await
            .unwrap();

        // the messages should be echoed to us
        let actual = read_log_stream(&mut log_stream, messages.len()).await;
        assert_eq!(messages, actual);

        completion.finish();
    }

    #[tokio::test]
    async fn pubsub_history() {
        let (completion, addr) = setup().await;
        let mut client = make_client(addr).await; // We use the same client for both producing and consuming
        let messages = fake_log_stream(3);

        // don't read anything yet - these messages should be sent to us as part of history when we call `read_messages` later

        // Write a few messages:
        client
            .write_messages(tokio_stream::iter(
                messages
                    .clone()
                    .into_iter()
                    .map(|msg| log_msg_to_proto(msg, Compression::Off).unwrap()),
            ))
            .await
            .unwrap();

        // Start reading now - we should receive full history at this point:
        let mut log_stream = client.read_messages(Empty {}).await.unwrap();

        let actual = read_log_stream(&mut log_stream, messages.len()).await;
        assert_eq!(messages, actual);

        completion.finish();
    }

    #[tokio::test]
    async fn one_producer_many_consumers() {
        let (completion, addr) = setup().await;
        let mut producer = make_client(addr).await; // We use separate clients for producing and consuming
        let mut consumers = vec![make_client(addr).await, make_client(addr).await];
        let messages = fake_log_stream(3);

        // Initialize multiple read streams:
        let mut log_streams = vec![];
        for consumer in &mut consumers {
            log_streams.push(consumer.read_messages(Empty {}).await.unwrap());
        }

        // Write a few messages using our single producer:
        producer
            .write_messages(tokio_stream::iter(
                messages
                    .clone()
                    .into_iter()
                    .map(|msg| log_msg_to_proto(msg, Compression::Off).unwrap()),
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
        let messages = fake_log_stream(3);

        // Initialize multiple read streams:
        let mut log_streams = vec![];
        for consumer in &mut consumers {
            log_streams.push(consumer.read_messages(Empty {}).await.unwrap());
        }

        // Write a few messages using each producer:
        for producer in &mut producers {
            producer
                .write_messages(tokio_stream::iter(
                    messages
                        .clone()
                        .into_iter()
                        .map(|msg| log_msg_to_proto(msg, Compression::Off).unwrap()),
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
            assert_eq!(expected, actual);
        }

        completion.finish();
    }
}
