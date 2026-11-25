//! Messages that the in-process server hands to the viewer must not keep tonic's receive buffer alive.
//!
//! This is its own test binary because it installs a global allocator to measure live heap bytes.

use std::time::Duration;

use re_grpc_server::{MemoryLimit, PlaybackBehavior, ServerListener, ServerOptions};
use re_log_channel::DataSourceMessage;
use re_log_encoding::ToTransport as _;
use re_log_encoding::rrd::Compression;
use re_log_msg::{LogMsg, SetStoreInfo, StoreInfo, StoreSource};
use re_log_types::{StoreId, StoreKind};
use re_protos::sdk_comms::v1alpha1::WriteMessagesRequest;
use re_protos::sdk_comms::v1alpha1::message_proxy_service_client::MessageProxyServiceClient;

#[global_allocator]
static GLOBAL: re_memory::AccountingAllocator<std::alloc::System> =
    re_memory::AccountingAllocator::new(std::alloc::System);

const LARGE_BLOB_LEN: usize = 32 * 1024 * 1024;
const SMALL_BLOB_LEN: usize = 4 * 1024;
const NUM_SMALL_MESSAGES: usize = 16;

fn live_bytes() -> usize {
    re_memory::accounting_allocator::global_allocs()
        .expect("the accounting allocator is installed")
        .size
}

fn blob_msg(store_id: &StoreId, len: usize) -> LogMsg {
    let chunk = re_chunk::Chunk::builder("blob")
        .with_archetype(
            re_chunk::RowId::new(),
            re_log_types::TimePoint::STATIC,
            &re_sdk_types::archetypes::EncodedImage::from_file_contents(vec![0xAB; len]),
        )
        .build()
        .expect("a single-row chunk is valid");
    let arrow_msg = chunk.to_arrow_msg().expect("the chunk encodes to Arrow");
    LogMsg::ArrowMsg(store_id.clone(), arrow_msg)
}

/// A large uncompressed message followed by small ones over a single stream: the small ones land
/// in the receive buffer that tonic grew for the large one.
/// After the large message is dropped, the small ones the viewer keeps must not hold on to it.
#[tokio::test(flavor = "multi_thread")]
async fn small_messages_do_not_pin_a_large_receive_buffer() {
    let listener = ServerListener::bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = listener.local_addr();
    let (receiver, _proxy) = re_grpc_server::spawn_with_recv(
        listener,
        ServerOptions {
            playback_behavior: PlaybackBehavior::OldestFirst,
            memory_limit: MemoryLimit::ZERO,
            cors_allowed_origins: vec![],
        },
        re_grpc_server::shutdown::never(),
    );

    let baseline = live_bytes();

    {
        let store_id = StoreId::random(StoreKind::Recording, "test_app");
        let set_store_info = LogMsg::SetStoreInfo(SetStoreInfo {
            row_id: *re_chunk::RowId::new(),
            info: StoreInfo::new(store_id.clone(), StoreSource::Unknown),
        });

        let mut messages = vec![
            set_store_info.to_transport(Compression::Off).unwrap(),
            blob_msg(&store_id, LARGE_BLOB_LEN)
                .to_transport(Compression::Off)
                .unwrap(),
        ];
        for _ in 0..NUM_SMALL_MESSAGES {
            messages.push(
                blob_msg(&store_id, SMALL_BLOB_LEN)
                    .to_transport(Compression::Off)
                    .unwrap(),
            );
        }

        let mut client = MessageProxyServiceClient::new(
            tonic::transport::Endpoint::from_shared(format!("http://{addr}"))
                .unwrap()
                .connect()
                .await
                .unwrap(),
        )
        .max_encoding_message_size(usize::MAX);
        // tonic's encoder packs messages that are ready together into one frame. Pausing between
        // them sends each small message in a frame of its own, as an SDK logging at intervals does,
        // and such a frame fits in the space left over after the large message.
        let requests = futures::StreamExt::then(futures::stream::iter(messages), |msg| async {
            tokio::time::sleep(Duration::from_millis(5)).await;
            WriteMessagesRequest {
                log_msg: Some(msg.into()),
            }
        });
        client.write_messages(requests).await.unwrap();
    }

    let kept_small_messages = tokio::task::spawn_blocking(move || {
        let mut kept = Vec::new();
        let mut num_arrow_msgs = 0;
        while num_arrow_msgs < 1 + NUM_SMALL_MESSAGES {
            let msg = receiver
                .recv_timeout(Duration::from_secs(30))
                .expect("the server forwards every message");
            if let Some(DataSourceMessage::LogMsg(LogMsg::ArrowMsg(_, arrow_msg))) = msg.into_data()
            {
                num_arrow_msgs += 1;
                if num_arrow_msgs > 1 {
                    kept.push(arrow_msg);
                }
            }
        }
        kept
    })
    .await
    .unwrap();
    assert_eq!(kept_small_messages.len(), NUM_SMALL_MESSAGES);

    let retained = live_bytes().saturating_sub(baseline);
    assert!(
        retained < LARGE_BLOB_LEN / 2,
        "{NUM_SMALL_MESSAGES} messages of {SMALL_BLOB_LEN} bytes keep {retained} bytes alive"
    );

    drop(kept_small_messages);
}
