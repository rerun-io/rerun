//! Shared fixtures for the v2 pipeline tests.
//!
//! The centrepiece is [`MockFetchClient`]: a [`DataframeClientAPI`] that
//! answers `FetchChunks` with *real* chunks, encoded through the same
//! transport path the server uses (`Chunk::to_arrow_msg` →
//! `ToTransport::to_transport` → `FetchChunksResponse`). That lets the
//! executor/driver/bridge tests run the production decode path
//! (`fetch_batch_group_via_grpc`) rather than a stubbed shortcut, which is
//! what makes them worth writing: the routing, cursor, and ledger logic all
//! key off `Chunk::id()`, so a fake that skips encoding would not exercise
//! the lookups that matter.
//!
//! Chunk IDs are derived deterministically from `(segment_id, row)` by
//! [`chunk_id_for`], so a chunk-info batch built by [`chunk_info_batch`] and
//! a chunk built by [`scalar_chunk`] agree on identity without any plumbing.

use std::collections::VecDeque;
use std::sync::Arc;

use parking_lot::Mutex;

use arrow::array::{
    Array as _, ArrayRef, BooleanArray, FixedSizeBinaryBuilder, Int64Array, RecordBatch,
    RecordBatchOptions, StringArray, UInt64Array,
};
use arrow::datatypes::{DataType, Field, Schema};
use re_dataframe::external::re_chunk::{Chunk, ChunkId, RowId};
use re_log_types::{EntityPath, TimeInt, TimePoint, Timeline};
use re_protos::cloud::v1alpha1::ext::QueryDatasetDataframe;
use re_protos::cloud::v1alpha1::{
    FetchChunksRequest, FetchChunksResponse, GetDatasetSchemaRequest, GetDatasetSchemaResponse,
    QueryDatasetRequest, QueryDatasetResponse,
};
use tonic::codec::DecodeBuf;
use tonic::{Request, Response, Status};

use crate::dataframe_query_common::DataframeClientAPI;

/// The chunk id [`chunk_info_batch`] writes for row `i` of `segment_id`, and
/// the one [`scalar_chunk`] stamps onto the chunk it builds.
pub fn chunk_id_for(segment_id: &str, i: usize) -> ChunkId {
    let mut bytes = [0u8; 16];
    bytes[0..8].copy_from_slice(&(i as u64 + 1).to_le_bytes());
    // Up to 8 id bytes, zero-padded: production `ChunkId`s are globally
    // unique Tuids, so ids must differ across segments too — the fixture
    // segment ids are short and distinct within their first 8 bytes.
    let seg = segment_id.as_bytes();
    let n = seg.len().min(8);
    bytes[8..8 + n].copy_from_slice(&seg[..n]);
    ChunkId::from_tuid(re_log_types::external::re_tuid::Tuid::from_bytes(bytes))
}

/// Chunk-info batch with `:start`, `chunk_is_static`, and uncompressed sizes.
///
/// `starts[i] = None` → null `:start`. Passing `timeline: None` omits the
/// `:start` column entirely (the "old server" shape that forces `BufferAll`).
pub fn chunk_info_batch(
    segment_id: &str,
    timeline: Option<&str>,
    starts: &[Option<i64>],
    statics: &[bool],
    uncompressed: &[Option<u64>],
) -> RecordBatch {
    chunk_info_batch_sized(
        segment_id,
        timeline,
        starts,
        statics,
        uncompressed,
        &vec![10u64; starts.len()],
    )
}

/// [`chunk_info_batch`] with the *compressed* `chunk_byte_len` spelled out.
///
/// That column — not the uncompressed estimate — is what the request packer
/// bin-packs against, so it is the only way a fixture can control how many
/// fetch batches (and therefore how many mock responses) a plan produces.
pub fn chunk_info_batch_sized(
    segment_id: &str,
    timeline: Option<&str>,
    starts: &[Option<i64>],
    statics: &[bool],
    uncompressed: &[Option<u64>],
    compressed: &[u64],
) -> RecordBatch {
    assert_eq!(starts.len(), statics.len());
    assert_eq!(starts.len(), uncompressed.len());
    assert_eq!(starts.len(), compressed.len());
    let n = starts.len();

    let segment_ids = StringArray::from(vec![segment_id; n]);
    let sizes = UInt64Array::from(compressed.to_vec());
    let mut chunk_id_builder = FixedSizeBinaryBuilder::with_capacity(n, 16);
    for i in 0..n {
        chunk_id_builder
            .append_value(chunk_id_for(segment_id, i).as_bytes())
            .unwrap();
    }
    let chunk_ids = chunk_id_builder.finish();
    let static_col = BooleanArray::from(statics.to_vec());
    let uncompressed_col = UInt64Array::from(uncompressed.to_vec());

    let mut fields = vec![
        QueryDatasetDataframe::COLUMN_CHUNK_SEGMENT_ID.arrow_field(),
        Field::new(
            QueryDatasetDataframe::COLUMN_CHUNK_BYTE_LEN_NAME,
            DataType::UInt64,
            false,
        ),
        QueryDatasetDataframe::COLUMN_CHUNK_ID.arrow_field(),
        QueryDatasetDataframe::COLUMN_CHUNK_IS_STATIC.arrow_field(),
        Field::new(
            QueryDatasetDataframe::COLUMN_CHUNK_BYTE_SIZE_UNCOMPRESSED_NAME,
            DataType::UInt64,
            true,
        ),
    ];
    let mut columns: Vec<ArrayRef> = vec![
        Arc::new(segment_ids),
        Arc::new(sizes),
        Arc::new(chunk_ids),
        Arc::new(static_col),
        Arc::new(uncompressed_col),
    ];
    if let Some(timeline) = timeline {
        fields.push(Field::new(
            format!("{timeline}:start"),
            DataType::Int64,
            true,
        ));
        columns.push(Arc::new(Int64Array::from(starts.to_vec())));
    }

    let schema = Arc::new(Schema::new_with_metadata(fields, Default::default()));
    RecordBatch::try_new_with_options(
        schema,
        columns,
        &RecordBatchOptions::new().with_row_count(Some(n)),
    )
    .unwrap()
}

/// The entity every fixture chunk logs to.
pub const TEST_ENTITY: &str = "/points";

/// The component every fixture chunk carries.
pub fn test_component() -> re_types_core::ComponentDescriptor {
    re_types_core::ComponentDescriptor::partial("value")
}

/// One real chunk holding a single scalar row at `time` on `timeline`,
/// stamped with the id [`chunk_info_batch`] announced for `(segment_id, i)`.
///
/// `time: None` builds a static chunk (no time column) — the shape that maps
/// to `CursorKey::PreTime`.
pub fn scalar_chunk(
    segment_id: &str,
    i: usize,
    timeline: &'static str,
    time: Option<i64>,
    value: i64,
) -> Chunk {
    scalar_chunk_on_component(segment_id, i, timeline, time, value, test_component())
}

/// [`scalar_chunk`] with the component spelled out.
///
/// A fixture mixing a static chunk with temporal ones must put them on
/// *different* components: static data takes precedence over temporal data for
/// the same component descriptor, so a static `value` would shadow every
/// temporal `value` and leave a temporal query with no rows at all.
pub fn scalar_chunk_on_component(
    segment_id: &str,
    i: usize,
    timeline: &'static str,
    time: Option<i64>,
    value: i64,
    component: re_types_core::ComponentDescriptor,
) -> Chunk {
    let timepoint: TimePoint = match time {
        Some(t) => TimePoint::from([(Timeline::new_sequence(timeline), TimeInt::new_temporal(t))]),
        None => TimePoint::default(),
    };

    let array: ArrayRef = Arc::new(Int64Array::from(vec![value]));

    Chunk::builder_with_id(chunk_id_for(segment_id, i), EntityPath::from(TEST_ENTITY))
        .with_row(RowId::new(), timepoint, [(component, array)])
        .build()
        .expect("fixture chunk is well-formed")
}

/// What a mock `FetchChunks` call should do.
pub enum FetchBehavior {
    /// Return these chunks, each tagged with the given segment id.
    Chunks(Vec<(Chunk, String)>),

    /// Fail the RPC.
    Fail(Status),
}

/// A [`DataframeClientAPI`] whose `fetch_chunks` replays a scripted queue.
///
/// Calls pop the queue in order; once it is empty every further call returns
/// an empty response. `calls` counts every call, including the drained ones.
#[derive(Clone, Debug)]
pub struct MockFetchClient {
    script: Arc<Mutex<VecDeque<FetchBehavior>>>,
    calls: Arc<std::sync::atomic::AtomicU64>,
    origin: re_uri::Origin,
}

impl std::fmt::Debug for FetchBehavior {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Chunks(chunks) => write!(f, "Chunks({})", chunks.len()),
            Self::Fail(err) => write!(f, "Fail({err})"),
        }
    }
}

impl MockFetchClient {
    pub fn new(script: impl IntoIterator<Item = FetchBehavior>) -> Self {
        Self {
            script: Arc::new(Mutex::new(script.into_iter().collect())),
            calls: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            origin: re_uri::Origin::test(),
        }
    }

    /// A single scripted response carrying all of `chunks`.
    pub fn returning(chunks: impl IntoIterator<Item = (Chunk, String)>) -> Self {
        Self::new([FetchBehavior::Chunks(chunks.into_iter().collect())])
    }

    pub fn call_count(&self) -> u64 {
        self.calls.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// Encodes a chunk the way the server does, so the client-side decode path
/// in `fetch_chunks_response_to_chunk_and_segment_id` runs for real.
fn encode_chunk(chunk: &Chunk, segment_id: &str) -> re_protos::log_msg::v1alpha1::ArrowMsg {
    use re_log_encoding::ToTransport as _;

    let arrow_msg = chunk.to_arrow_msg().expect("fixture chunk encodes");
    let store_id = re_log_types::StoreId::new(
        re_log_types::StoreKind::Recording,
        "test_app",
        segment_id.to_owned(),
    );
    arrow_msg
        .to_transport((store_id, re_log_encoding::Compression::Off))
        .expect("fixture chunk converts to transport")
}

/// Decoder that ignores the wire bytes and pops pre-built responses.
///
/// The mock hands tonic a body with one well-formed (but empty-payload)
/// gRPC frame per queued response, purely so tonic invokes us the right
/// number of times.
struct ScriptedDecoder {
    queue: VecDeque<FetchChunksResponse>,
}

impl tonic::codec::Decoder for ScriptedDecoder {
    type Item = FetchChunksResponse;
    type Error = Status;

    fn decode(&mut self, src: &mut DecodeBuf<'_>) -> Result<Option<Self::Item>, Self::Error> {
        // Consume the frame's payload. A decoder that leaves bytes behind
        // makes tonic re-invoke it on the same buffer forever.
        use bytes::Buf as _;
        src.advance(src.remaining());

        Ok(self.queue.pop_front())
    }
}

/// One gRPC length-prefixed frame per message, with a 1-byte payload.
///
/// tonic only calls the decoder once per frame it can parse, so the frame
/// count — not the payload — is what drives [`ScriptedDecoder`].
fn framed_body(n_messages: usize) -> String {
    let mut body = Vec::new();
    for _ in 0..n_messages {
        body.push(0u8); // uncompressed
        body.extend_from_slice(&1u32.to_be_bytes()); // payload length
        body.push(b'x');
    }
    String::from_utf8(body).expect("frame bytes are ASCII-safe")
}

#[async_trait::async_trait]
impl DataframeClientAPI for MockFetchClient {
    fn origin(&self) -> &re_uri::Origin {
        &self.origin
    }

    async fn get_dataset_schema(
        &mut self,
        _request: Request<GetDatasetSchemaRequest>,
    ) -> tonic::Result<Response<GetDatasetSchemaResponse>> {
        Err(Status::unimplemented("unused by pipeline_v2 tests"))
    }

    async fn query_dataset(
        &mut self,
        _request: Request<QueryDatasetRequest>,
    ) -> tonic::Result<Response<tonic::codec::Streaming<QueryDatasetResponse>>> {
        Err(Status::unimplemented("unused by pipeline_v2 tests"))
    }

    async fn fetch_chunks(
        &mut self,
        _request: Request<FetchChunksRequest>,
    ) -> tonic::Result<Response<tonic::codec::Streaming<FetchChunksResponse>>> {
        self.calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let behavior = self.script.lock().pop_front();
        let chunks = match behavior {
            Some(FetchBehavior::Fail(err)) => return Err(err),
            Some(FetchBehavior::Chunks(chunks)) => chunks,
            // Drained: the plan asked for more batches than the script
            // covers. An empty response is the honest answer.
            None => Vec::new(),
        };

        let response = FetchChunksResponse {
            chunks: chunks
                .iter()
                .map(|(chunk, segment_id)| encode_chunk(chunk, segment_id))
                .collect(),
        };

        // One `FetchChunksResponse` message carrying every chunk.
        let mut queue: VecDeque<_> = VecDeque::with_capacity(1);
        queue.push_back(response);
        let body = framed_body(queue.len());
        // `new_request`, not `new_response`: the response direction expects a
        // `grpc-status` trailer to close the stream, which a plain body never
        // sends — the stream would hang at end-of-body. The request direction
        // terminates at EOF, which is what we want.
        let streaming = Request::new(tonic::codec::Streaming::new_request(
            ScriptedDecoder { queue },
            body,
            None,
            None,
        ))
        .into_inner();
        Ok(Response::new(streaming))
    }
}

/// A `PendingQueryAnalytics` with no connection behind it: the OTLP send on
/// drop is gated internally on the connection being present, so nothing
/// escapes the test process.
pub fn test_pending_analytics() -> crate::PendingQueryAnalytics {
    crate::analytics::begin_query(
        None,
        crate::analytics::QueryInfo {
            dataset_id: "ds-test".to_owned(),
            query_chunks: 0,
            query_segments: 0,
            query_layers: 0,
            query_columns: 0,
            query_entities: 0,
            query_bytes: 0,
            query_chunks_per_segment_min: 0,
            query_chunks_per_segment_max: 0,
            query_chunks_per_segment_mean: 0.0,
            target_partitions: 0,
            query_type: crate::analytics::QueryType::Dataframe,
            primary_index_name: None,
            time_to_first_chunk_info: None,
            trace_id: None,
            filters_pushed_down: 0,
            filters_applied_client_side: 0,
            entity_path_narrowing_applied: false,
            filters_total: 0,
            filters_signatures: String::new(),
            filters_signatures_exact: String::new(),
            filters_signatures_inexact: String::new(),
            filters_signatures_unsupported: String::new(),
        },
        std::time::Instant::now(),
        std::time::SystemTime::now(),
    )
}

/// Projected schema carrying only the segment-id column.
///
/// `align_record_batch_to_schema` selects target fields by name, so this
/// narrows every emitted batch to one column of repeated segment ids —
/// exactly the signal the driver tests assert on (which segment emitted, in
/// what order, how many rows) with none of the payload noise.
pub fn segment_id_only_schema() -> Arc<Schema> {
    Arc::new(Schema::new_with_metadata(
        vec![Field::new(
            re_protos::cloud::v1alpha1::ext::ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME,
            DataType::Utf8,
            false,
        )],
        Default::default(),
    ))
}

/// Runs a whole partition driver over `chunk_infos` against a mock serving
/// `responses`, and returns the emitted batches in order.
///
/// This is the full v2 tree minus the DataFusion bridge: real planner, real
/// window, real executor (spawning onto the current runtime), real
/// per-segment stores.
///
/// Always asserts that the issuance window ends fully refunded: once the
/// driver has finished, every lease has died and every overdraft settled, on
/// completion, under-delivery, and error unwind alike — so any shortfall is a
/// leaked or burnt-and-never-minted-back permit.
///
/// The returned [`OverdraftAudit`] lets a fixture prove it actually exercised
/// the priority path, rather than passing for some unrelated reason.
pub async fn run_driver(
    chunk_infos: Vec<RecordBatch>,
    timeline: &'static str,
    responses: impl IntoIterator<Item = FetchBehavior>,
    window_bytes: u64,
    fetch_concurrency: usize,
    limit_rows: Option<usize>,
) -> (
    Result<(), re_redap_client::ApiError>,
    Vec<RecordBatch>,
    super::window::OverdraftAudit,
) {
    run_driver_projected(
        chunk_infos,
        timeline,
        responses,
        window_bytes,
        fetch_concurrency,
        limit_rows,
        segment_id_only_schema(),
    )
    .await
}

/// [`run_driver`] with the projected schema spelled out, for fixtures that
/// assert on emitted component *values* and not only on segment order.
///
/// A name absent from the emitted batch aligns to an all-null column rather
/// than an error (`align_record_batch_to_schema`), so a fixture that misnames
/// a column fails its own value assertion instead of passing vacuously.
pub async fn run_driver_projected(
    chunk_infos: Vec<RecordBatch>,
    timeline: &'static str,
    responses: impl IntoIterator<Item = FetchBehavior>,
    window_bytes: u64,
    fetch_concurrency: usize,
    limit_rows: Option<usize>,
    projected_schema: Arc<Schema>,
) -> (
    Result<(), re_redap_client::ApiError>,
    Vec<RecordBatch>,
    super::window::OverdraftAudit,
) {
    use re_dataframe::QueryExpression;

    use super::driver::PartitionDriver;
    use super::executor::FetchExecutor;
    use super::plan::build_fetch_plan;
    use super::window::IssuanceWindow;

    let query_expression = QueryExpression {
        filtered_index: Some(re_dataframe::TimelineName::try_new(timeline).unwrap()),
        ..Default::default()
    };

    let origin = re_uri::Origin::test();

    let plan = build_fetch_plan(
        &origin,
        chunk_infos,
        query_expression.filtered_index,
        &None,
        crate::dataframe_query_provider::fetch_plan::TARGET_BATCH_SIZE_BYTES as u64,
    )
    .expect("fixture plan builds");

    let window = Arc::new(IssuanceWindow::new(window_bytes));
    let pending_analytics = test_pending_analytics();
    let executor = FetchExecutor {
        client: MockFetchClient::new(responses),
        http_client: reqwest::Client::new(),
        object_store_auth: Arc::new(crate::chunk_fetcher::NoOpObjectStoreAuthenticator {}),
        io_handle: tokio::runtime::Handle::current(),
        window: Arc::clone(&window),
        front: Arc::new(super::executor::DeliveryFront::default()),
        metrics: Arc::clone(pending_analytics.metrics()),
        pending_analytics: pending_analytics.clone(),
    };

    let driver = PartitionDriver {
        plan: Arc::new(plan),
        executor,
        fetch_concurrency,
        origin,
        query_expression,
        index_values: None,
        projected_schema,
        limit_rows,
    };

    let (tx, mut rx) = tokio::sync::mpsc::channel::<RecordBatch>(64);
    let run = tokio::spawn(driver.run(tx));

    let mut batches = Vec::new();
    while let Some(batch) = rx.recv().await {
        batches.push(batch);
    }
    let result = run.await.expect("driver task does not panic");

    assert_eq!(
        window.available_bytes(),
        window_bytes,
        "the issuance window must be fully refunded once the driver is done",
    );

    (result, batches, window.overdraft_audit())
}

/// Runs a partition through the real DataFusion bridge — `execute_partition`
/// and the `SendableRecordBatchStream` it hands back — rather than the driver
/// alone, so the end-of-stream path is covered.
///
/// That path is worth its own fixture because the driver's row channel closes
/// strictly before its outcome is knowable, and the two are observed from
/// different runtimes here (driver on the shared CPU runtime, stream polled on
/// the test's) exactly as in production.
///
/// Returns the emitted batches (or the terminating error), the partition
/// countdown, and the recorded analytics error kind, so callers can assert
/// the analytics bookkeeping too.
///
/// `fetch_concurrency` is explicit because [`MockFetchClient`] hands out its
/// scripted responses to whichever in-flight call reaches the queue first:
/// above 1, a fixture cannot rely on a given response landing in a given
/// batch. Pass 1 when the test depends on that pairing.
pub async fn run_bridge(
    chunk_infos: Vec<RecordBatch>,
    timeline: &'static str,
    responses: impl IntoIterator<Item = FetchBehavior>,
    fetch_concurrency: usize,
) -> (
    Result<Vec<RecordBatch>, datafusion::error::DataFusionError>,
    Arc<std::sync::atomic::AtomicUsize>,
    Option<&'static str>,
) {
    use std::sync::atomic::{AtomicBool, AtomicUsize};

    use futures::StreamExt as _;
    use re_dataframe::QueryExpression;

    use super::FetchConfig;
    use super::bridge::{V2PartitionCtx, execute_partition};

    let partitions_remaining = Arc::new(AtomicUsize::new(1));
    let pending_analytics = test_pending_analytics();

    let setup = execute_partition(V2PartitionCtx {
        chunk_infos,
        query_expression: QueryExpression {
            filtered_index: Some(re_dataframe::TimelineName::try_new(timeline).unwrap()),
            ..Default::default()
        },
        index_values: None,
        projected_schema: segment_id_only_schema(),
        limit_rows: None,
        client: MockFetchClient::new(responses),
        object_store_auth: Arc::new(crate::chunk_fetcher::NoOpObjectStoreAuthenticator {}),
        io_handle: tokio::runtime::Handle::current(),
        config: FetchConfig {
            fetch_concurrency,
            window_bytes: 64 * 1024 * 1024,
        },
        trace_headers: None,
        server_trace_id: None,
        pending_analytics: pending_analytics.clone(),
        // A live collector: `emit_snapshot_if_last_partition` short-circuits
        // before touching the countdown when none is registered, so an empty
        // vec would make the partition bookkeeping untestable.
        captured_collectors: vec![crate::MetricsCollector::new()],
        partitions_remaining: Arc::clone(&partitions_remaining),
        snapshot_sent: Arc::new(AtomicBool::new(false)),
    });

    // Setup can fail before any stream exists (e.g. an unbuildable plan);
    // that is a terminal outcome for the partition, so report it as one.
    let mut stream = match setup {
        Ok(stream) => stream,
        Err(err) => {
            return (
                Err(err),
                partitions_remaining,
                pending_analytics.error_kind(),
            );
        }
    };

    let mut batches = Vec::new();
    let result = loop {
        match stream.next().await {
            None => break Ok(batches),
            Some(Ok(batch)) => batches.push(batch),
            Some(Err(err)) => break Err(err),
        }
    };

    // Terminating drops the stream's inner state, including the driver's
    // `JoinHandle` — which panics if polled again. Operators are not required
    // to stop at the first terminal item, so every terminal path has to be
    // idempotent.
    assert!(
        stream.next().await.is_none(),
        "polling past termination must keep yielding end-of-stream",
    );

    (result, partitions_remaining, pending_analytics.error_kind())
}

/// [`segment_id_only_schema`] plus one component column of [`TEST_ENTITY`].
///
/// Dataframe-batch component columns are named `{entity_path}:{component}` and
/// carry a list per row (`ComponentColumnDescriptor::column_name`), which is
/// what the emit path produces and therefore what alignment matches against.
pub fn schema_with_component(component: &str) -> Arc<Schema> {
    Arc::new(Schema::new_with_metadata(
        vec![
            Field::new(
                re_protos::cloud::v1alpha1::ext::ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME,
                DataType::Utf8,
                false,
            ),
            Field::new(
                format!("{TEST_ENTITY}:{component}"),
                DataType::List(Arc::new(Field::new("item", DataType::Int64, true))),
                true,
            ),
        ],
        Default::default(),
    ))
}

/// Per-row scalars of the component column built by [`schema_with_component`],
/// flattened across batches. `None` is a null at the row level — the shape a
/// row emitted before its static chunk arrived would have.
pub fn emitted_component_values(batches: &[RecordBatch], component: &str) -> Vec<Option<i64>> {
    let name = format!("{TEST_ENTITY}:{component}");
    batches
        .iter()
        .flat_map(|batch| {
            let (idx, _) = batch
                .schema()
                .column_with_name(&name)
                .expect("fixture projected this column");
            let col = batch
                .column(idx)
                .as_any()
                .downcast_ref::<arrow::array::ListArray>()
                .expect("component columns are lists");
            (0..col.len())
                .map(|i| {
                    if col.is_null(i) {
                        return None;
                    }
                    let values = col.value(i);
                    let values = values
                        .as_any()
                        .downcast_ref::<Int64Array>()
                        .expect("fixture components are Int64");
                    (values.len() == 1 && !values.is_null(0)).then(|| values.value(0))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The segment id each emitted row carries, flattened across batches.
pub fn emitted_segment_ids(batches: &[RecordBatch]) -> Vec<String> {
    batches
        .iter()
        .flat_map(|batch| {
            let col = batch
                .column(0)
                .as_any()
                .downcast_ref::<StringArray>()
                .expect("segment-id column is Utf8");
            (0..col.len())
                .map(|i| col.value(i).to_owned())
                .collect::<Vec<_>>()
        })
        .collect()
}

mod smoke {
    use std::sync::atomic::AtomicU64;

    use super::*;
    use crate::analytics::TaskFetchStats;
    use crate::chunk_fetcher::fetch_batch_group_via_grpc;

    /// Narrowest possible check: does the scripted `Streaming` yield its
    /// message at all? Isolates mock plumbing from the decode path above it.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn scripted_streaming_yields_one_message() {
        use futures::StreamExt as _;

        let chunk = scalar_chunk("seg-a", 0, "frame", Some(10), 42);
        let mut client = MockFetchClient::returning([(chunk, "seg-a".to_owned())]);
        let response = client
            .fetch_chunks(Request::new(FetchChunksRequest {
                chunk_infos: vec![],
            }))
            .await
            .expect("mock responds");
        let msgs: Vec<_> = response.into_inner().collect().await;
        assert_eq!(msgs.len(), 1, "expected exactly one FetchChunksResponse");
        assert_eq!(msgs[0].as_ref().unwrap().chunks.len(), 1);
    }

    /// The mock must survive a full round-trip through the real decode path,
    /// preserving both chunk identity and the segment tag — everything else
    /// in these tests keys off those two.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn mock_round_trips_chunks_through_the_real_decode_path() {
        let chunk = scalar_chunk("seg-a", 0, "frame", Some(10), 42);
        let client = MockFetchClient::returning([(chunk, "seg-a".to_owned())]);

        let request = chunk_info_batch("seg-a", Some("frame"), &[Some(10)], &[false], &[Some(100)]);
        let counter = AtomicU64::new(0);
        let mut stats = TaskFetchStats::default();

        let fetched = fetch_batch_group_via_grpc(&[request], &client, &counter, &mut stats)
            .await
            .expect("mock fetch succeeds");

        let chunks: Vec<_> = fetched.into_iter().flatten().collect();
        assert_eq!(chunks.len(), 1);
        let (chunk, segment_id) = &chunks[0];
        assert_eq!(chunk.id(), chunk_id_for("seg-a", 0));
        assert_eq!(
            segment_id.as_ref().map(ToString::to_string).as_deref(),
            Some("seg-a")
        );
        assert_eq!(client.call_count(), 1);
    }
}
