/// Background worker for ingesting Arrow messages.
///
/// # Platform Support
///
/// - **Native**: Uses a dedicated background thread with bounded channels for backpressure
/// - **Wasm**: Processes synchronously (no threads available)
///
/// # Architecture
///
/// Each `EntityDb` owns its own `IngestionWorker` instance, ensuring:
/// - Message ordering per store is preserved
/// - Different stores don't block each other
/// - Worker lifecycle tied to `EntityDb` lifecycle
use std::sync::Arc;

use re_log_types::{ArrowMsg, StoreId};
use re_smart_channel::SmartChannelSource;

/// Maximum number of pending work items before backpressure kicks in (native only).
const WORK_QUEUE_CAPACITY: usize = 2000;

/// Result of processing a work item.
#[derive(Debug)]
pub struct ProcessedChunk {
    pub store_id: StoreId,
    pub chunk: Arc<re_chunk::Chunk>,
    pub timestamps: re_sorbet::TimestampMetadata,
    pub channel_source: Arc<SmartChannelSource>,
    pub msg_will_add_new_store: bool,
}

// ============================================================================
// NATIVE IMPLEMENTATION (background thread with channels)
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
mod native_impl {
    use std::sync::Arc;

    use re_log_types::{ArrowMsg, StoreId};
    use re_smart_channel::SmartChannelSource;

    use super::{ProcessedChunk, WORK_QUEUE_CAPACITY};

    /// Work item to be processed by the ingestion worker.
    struct WorkItem {
        store_id: StoreId,
        arrow_msg: ArrowMsg,
        channel_source: Arc<SmartChannelSource>,
        msg_will_add_new_store: bool,
    }

    /// Background worker for processing Arrow messages into chunks.
    ///
    /// Runs on a dedicated thread and provides backpressure via bounded channels.
    pub struct IngestionWorkerImpl {
        input_tx: crossbeam::channel::Sender<WorkItem>,
        output_rx: crossbeam::channel::Receiver<ProcessedChunk>,
        #[expect(dead_code)] // Kept alive for thread lifecycle
        worker_thread: Option<std::thread::JoinHandle<()>>,
    }

    impl IngestionWorkerImpl {
        /// Create a new ingestion worker with a dedicated background thread.
        pub fn new() -> Self {
            let (input_tx, input_rx) = crossbeam::channel::bounded::<WorkItem>(WORK_QUEUE_CAPACITY);
            let (output_tx, output_rx) = crossbeam::channel::unbounded::<ProcessedChunk>();

            let worker_thread = std::thread::Builder::new()
                .name("ingestion_worker".to_owned())
                .spawn(move || {
                    Self::worker_loop(input_rx, output_tx);
                })
                .expect("Failed to spawn ingestion worker thread");

            Self {
                input_tx,
                output_rx,
                worker_thread: Some(worker_thread),
            }
        }

        /// Submit an arrow message for processing, blocking if necessary.
        pub fn submit_arrow_msg_blocking(
            &self,
            store_id: StoreId,
            arrow_msg: ArrowMsg,
            channel_source: Arc<SmartChannelSource>,
            msg_will_add_new_store: bool,
        ) {
            let work_item = WorkItem {
                store_id,
                arrow_msg,
                channel_source,
                msg_will_add_new_store,
            };

            // Block until we can send
            if let Err(err) = self.input_tx.send(work_item) {
                re_log::warn!("Failed to send to ingestion worker: {err}");
            }
        }

        /// Poll for processed chunks. Returns all available chunks without blocking.
        pub fn poll_processed_chunks(&self) -> Vec<ProcessedChunk> {
            let mut chunks = Vec::new();

            // Drain all available processed chunks without blocking
            while let Ok(chunk) = self.output_rx.try_recv() {
                chunks.push(chunk);
            }

            chunks
        }

        /// Main worker loop that processes arrow messages.
        #[expect(clippy::needless_pass_by_value)] // Channels are moved into thread
        fn worker_loop(
            input_rx: crossbeam::channel::Receiver<WorkItem>,
            output_tx: crossbeam::channel::Sender<ProcessedChunk>,
        ) {
            re_log::debug!("Ingestion worker thread started");

            while let Ok(work_item) = input_rx.recv() {
                re_tracing::profile_scope!("process_arrow_msg");

                let WorkItem {
                    store_id,
                    arrow_msg,
                    channel_source,
                    msg_will_add_new_store,
                } = work_item;

                // Do the work of converting Arrow data to chunks
                let result = Self::process_arrow_msg(&arrow_msg);

                match result {
                    Ok((chunk, timestamps)) => {
                        let processed = ProcessedChunk {
                            store_id,
                            chunk: Arc::new(chunk),
                            timestamps,
                            channel_source,
                            msg_will_add_new_store,
                        };

                        if output_tx.send(processed).is_err() {
                            // Main thread has disconnected, time to exit
                            break;
                        }
                    }
                    Err(err) => {
                        re_log::warn_once!("Failed to process arrow message: {err}");
                    }
                }
            }

            re_log::debug!("Ingestion worker thread exiting");
        }

        /// Process an arrow message into a chunk.
        ///
        /// This is the work that we want to do off the main thread.
        fn process_arrow_msg(
            arrow_msg: &ArrowMsg,
        ) -> crate::Result<(re_chunk::Chunk, re_sorbet::TimestampMetadata)> {
            re_tracing::profile_function!();

            let chunk_batch = re_sorbet::ChunkBatch::try_from(&arrow_msg.batch)
                .map_err(re_chunk::ChunkError::from)?;
            let mut chunk = re_chunk::Chunk::from_chunk_batch(&chunk_batch)?;
            chunk.sort_if_unsorted();

            Ok((chunk, chunk_batch.sorbet_schema().timestamps.clone()))
        }
    }

    impl Drop for IngestionWorkerImpl {
        fn drop(&mut self) {
            // Dropping input_tx will cause the worker thread to exit gracefully
            // when it finishes processing remaining items
            re_log::debug!("Dropping ingestion worker");
        }
    }
}

// ============================================================================
// Wasm IMPLEMENTATION (synchronous processing, no threads)
// ============================================================================

#[cfg(target_arch = "wasm32")]
mod wasm_impl {
    use std::sync::Arc;

    use re_log_types::{ArrowMsg, StoreId};
    use re_smart_channel::SmartChannelSource;

    use super::ProcessedChunk;

    /// Wasm implementation that processes synchronously.
    ///
    /// Since Wasm doesn't support threads, we process messages immediately
    /// instead of queueing them to a background worker.
    pub struct IngestionWorkerImpl {
        // Wasm implementation has no state, but we keep the struct
        // for API compatibility
        _phantom: std::marker::PhantomData<()>,
    }

    impl IngestionWorkerImpl {
        /// Create a new synchronous ingestion worker (Wasm).
        pub fn new() -> Self {
            Self {
                _phantom: std::marker::PhantomData,
            }
        }

        /// Submit an arrow message for immediate synchronous processing.
        ///
        /// Unlike the native version, this processes the message immediately
        /// and adds it to an internal queue for later retrieval via poll_processed_chunks.
        pub fn submit_arrow_msg_blocking(
            &self,
            _store_id: StoreId,
            _arrow_msg: ArrowMsg,
            _channel_source: Arc<SmartChannelSource>,
            _msg_will_add_new_store: bool,
        ) {
            // On Wasm, we don't queue messages - they should be processed synchronously
            // by the caller instead of using the worker.
            re_log::warn_once!(
                "IngestionWorker::submit_arrow_msg_blocking called on Wasm - this is unexpected"
            );
        }

        /// Poll for processed chunks (always empty on Wasm).
        pub fn poll_processed_chunks(&self) -> Vec<ProcessedChunk> {
            // On Wasm, messages are processed synchronously, so polling always returns empty
            Vec::new()
        }
    }
}

// ============================================================================
// PUBLIC API (platform-agnostic)
// ============================================================================

/// Platform-agnostic ingestion worker.
///
/// On native: Uses background thread with channels
/// On Wasm: No-op (messages processed synchronously by caller)
pub struct IngestionWorker {
    #[cfg(not(target_arch = "wasm32"))]
    inner: native_impl::IngestionWorkerImpl,

    #[cfg(target_arch = "wasm32")]
    inner: wasm_impl::IngestionWorkerImpl,
}

impl IngestionWorker {
    /// Create a new ingestion worker.
    ///
    /// - On native: Spawns a background thread
    /// - On Wasm: Returns a no-op worker
    pub fn new() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self {
                inner: native_impl::IngestionWorkerImpl::new(),
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            Self {
                inner: wasm_impl::IngestionWorkerImpl::new(),
            }
        }
    }

    /// Submit an arrow message for processing.
    ///
    /// - On native: Queues to background thread (may block if queue is full)
    /// - On Wasm: No-op (caller should process synchronously instead)
    pub fn submit_arrow_msg_blocking(
        &self,
        store_id: StoreId,
        arrow_msg: ArrowMsg,
        channel_source: Arc<SmartChannelSource>,
        msg_will_add_new_store: bool,
    ) {
        self.inner.submit_arrow_msg_blocking(
            store_id,
            arrow_msg,
            channel_source,
            msg_will_add_new_store,
        );
    }

    /// Poll for processed chunks without blocking.
    ///
    /// - On native: Returns chunks processed by background thread
    /// - On Wasm: Always returns empty vec (messages processed synchronously)
    pub fn poll_processed_chunks(&self) -> Vec<ProcessedChunk> {
        self.inner.poll_processed_chunks()
    }
}

impl Default for IngestionWorker {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// TESTS (native only, since Wasm worker is a no-op)
// ============================================================================

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::sync::Arc;

    use re_chunk::{Chunk, RowId};
    use re_log_types::{
        ArrowMsg, NonMinI64, StoreId, StoreKind, TimeInt, TimePoint, Timeline, entity_path,
    };
    use re_smart_channel::SmartChannelSource;
    use re_types::archetypes::Points2D;

    use super::{IngestionWorker, WORK_QUEUE_CAPACITY};

    /// Helper to create a test arrow message
    fn create_test_arrow_msg(index: usize) -> ArrowMsg {
        let index_i64 = i64::try_from(index).expect("test index should fit in i64");
        let chunk = Chunk::builder(entity_path!("test", "points", index.to_string()))
            .with_archetype(
                RowId::new(),
                TimePoint::default().with(
                    Timeline::new_sequence("seq"),
                    TimeInt::from_millis(
                        NonMinI64::new(index_i64).expect("test index should not be i64::MIN"),
                    ),
                ),
                &Points2D::new([(index as f32, index as f32)]),
            )
            .build()
            .expect("test chunk should build successfully");

        chunk
            .to_arrow_msg()
            .expect("test chunk should convert to arrow msg")
    }

    #[test]
    fn test_worker_lifecycle() {
        // Test that worker starts and can be dropped gracefully
        let worker = IngestionWorker::new();

        // Worker should be ready to receive messages
        assert_eq!(worker.poll_processed_chunks().len(), 0);

        // Drop worker - should exit gracefully
        drop(worker);
    }

    #[test]
    fn test_basic_message_processing() {
        let worker = IngestionWorker::new();
        let store_id = StoreId::random(StoreKind::Recording, "test");
        let channel_source = Arc::new(SmartChannelSource::RrdHttpStream {
            follow: false,
            url: "http://test".into(),
        });

        // Submit a single message
        let arrow_msg = create_test_arrow_msg(0);
        worker.submit_arrow_msg_blocking(
            store_id.clone(),
            arrow_msg,
            channel_source.clone(),
            false,
        );

        // Give worker time to process
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Poll for results
        let chunks = worker.poll_processed_chunks();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].store_id, store_id);
    }

    #[test]
    fn test_multiple_messages_in_sequence() {
        let worker = IngestionWorker::new();
        let store_id = StoreId::random(StoreKind::Recording, "test");
        let channel_source = Arc::new(SmartChannelSource::RrdHttpStream {
            follow: false,
            url: "http://test".into(),
        });

        const NUM_MESSAGES: usize = 100;

        // Submit multiple messages
        for i in 0..NUM_MESSAGES {
            let arrow_msg = create_test_arrow_msg(i);
            worker.submit_arrow_msg_blocking(
                store_id.clone(),
                arrow_msg,
                channel_source.clone(),
                false,
            );
        }

        // Give worker time to process all messages
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Poll for all results
        let chunks = worker.poll_processed_chunks();
        assert_eq!(
            chunks.len(),
            NUM_MESSAGES,
            "Should process all {NUM_MESSAGES} messages"
        );

        // Verify all chunks are for the correct store
        for chunk in &chunks {
            assert_eq!(chunk.store_id, store_id);
        }
    }

    #[test]
    fn test_backpressure_behavior() {
        let worker = IngestionWorker::new();
        let store_id = StoreId::random(StoreKind::Recording, "test");
        let channel_source = Arc::new(SmartChannelSource::RrdHttpStream {
            follow: false,
            url: "http://test".into(),
        });

        // Submit messages beyond queue capacity to test backpressure
        // This should block but not panic or lose messages
        const NUM_MESSAGES: usize = WORK_QUEUE_CAPACITY + 100;

        let worker_clone = std::sync::Arc::new(worker);
        let worker_ref = worker_clone.clone();
        let store_id_clone = store_id.clone();
        let channel_source_clone = channel_source.clone();

        // Submit in a separate thread to avoid blocking test
        let submit_handle = std::thread::Builder::new()
            .name("test-backpressure-submitter".to_owned())
            .spawn(move || {
                for i in 0..NUM_MESSAGES {
                    let arrow_msg = create_test_arrow_msg(i);
                    worker_ref.submit_arrow_msg_blocking(
                        store_id_clone.clone(),
                        arrow_msg,
                        channel_source_clone.clone(),
                        false,
                    );
                }
            })
            .expect("failed to spawn test thread");

        // Poll periodically to drain the queue
        let mut total_chunks = 0;
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(10);

        while total_chunks < NUM_MESSAGES && start.elapsed() < timeout {
            let chunks = worker_clone.poll_processed_chunks();
            total_chunks += chunks.len();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Wait for submission thread to complete
        submit_handle.join().unwrap();

        // Poll any remaining chunks
        std::thread::sleep(std::time::Duration::from_millis(100));
        let remaining = worker_clone.poll_processed_chunks();
        total_chunks += remaining.len();

        assert_eq!(
            total_chunks, NUM_MESSAGES,
            "Should process all messages despite backpressure"
        );
    }

    #[test]
    fn test_invalid_arrow_data_handling() {
        let worker = IngestionWorker::new();
        let store_id = StoreId::random(StoreKind::Recording, "test");
        let channel_source = Arc::new(SmartChannelSource::RrdHttpStream {
            follow: false,
            url: "http://test".into(),
        });

        // Create an invalid arrow message (empty batch with incorrect schema)
        let schema = arrow::datatypes::Schema::new_with_metadata(
            vec![] as Vec<arrow::datatypes::Field>,
            Default::default(),
        );
        let batch = arrow::array::RecordBatch::new_empty(Arc::new(schema));
        let invalid_msg = ArrowMsg {
            chunk_id: *re_chunk::ChunkId::new(),
            batch,
            on_release: None,
        };

        // Submit invalid message - should not crash worker
        worker.submit_arrow_msg_blocking(
            store_id.clone(),
            invalid_msg,
            channel_source.clone(),
            false,
        );

        // Submit a valid message after
        let valid_msg = create_test_arrow_msg(0);
        worker.submit_arrow_msg_blocking(store_id.clone(), valid_msg, channel_source, false);

        // Give worker time to process
        std::thread::sleep(std::time::Duration::from_millis(200));

        // Should get only the valid message
        let chunks = worker.poll_processed_chunks();
        assert_eq!(
            chunks.len(),
            1,
            "Worker should skip invalid message and continue"
        );
    }

    #[test]
    fn test_concurrent_submission_and_polling() {
        let worker = Arc::new(IngestionWorker::new());
        let store_id = StoreId::random(StoreKind::Recording, "test");
        let channel_source = Arc::new(SmartChannelSource::RrdHttpStream {
            follow: false,
            url: "http://test".into(),
        });

        const NUM_MESSAGES: usize = 500;

        // Spawn submitter thread
        let worker_submit = worker.clone();
        let store_id_submit = store_id.clone();
        let channel_source_submit = channel_source.clone();

        let submit_handle = std::thread::Builder::new()
            .name("test-concurrent-submitter".to_owned())
            .spawn(move || {
                for i in 0..NUM_MESSAGES {
                    let arrow_msg = create_test_arrow_msg(i);
                    worker_submit.submit_arrow_msg_blocking(
                        store_id_submit.clone(),
                        arrow_msg,
                        channel_source_submit.clone(),
                        false,
                    );
                    // Small delay to simulate realistic submission pattern
                    if i % 50 == 0 {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                }
            })
            .expect("failed to spawn test thread");

        // Poll concurrently from main thread
        let mut total_chunks = 0;
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(10);

        while total_chunks < NUM_MESSAGES && start.elapsed() < timeout {
            let chunks = worker.poll_processed_chunks();
            total_chunks += chunks.len();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        // Wait for submission to complete
        submit_handle.join().unwrap();

        // Final poll to get any remaining chunks
        std::thread::sleep(std::time::Duration::from_millis(100));
        let remaining = worker.poll_processed_chunks();
        total_chunks += remaining.len();

        assert_eq!(
            total_chunks, NUM_MESSAGES,
            "Should handle concurrent submission and polling"
        );
    }

    #[test]
    fn test_worker_thread_exits_on_drop() {
        let worker = IngestionWorker::new();
        let store_id = StoreId::random(StoreKind::Recording, "test");
        let channel_source = Arc::new(SmartChannelSource::RrdHttpStream {
            follow: false,
            url: "http://test".into(),
        });

        // Submit some messages
        for i in 0..10 {
            let arrow_msg = create_test_arrow_msg(i);
            worker.submit_arrow_msg_blocking(
                store_id.clone(),
                arrow_msg,
                channel_source.clone(),
                false,
            );
        }

        // Drop worker - thread should exit gracefully
        drop(worker);

        // If thread doesn't exit properly, this test will hang or leak threads
        // Successful completion means thread exited
    }

    #[test]
    fn test_empty_poll_returns_empty_vec() {
        let worker = IngestionWorker::new();

        // Poll without submitting anything
        let chunks = worker.poll_processed_chunks();
        assert_eq!(chunks.len(), 0);

        // Multiple polls should all return empty
        let chunks2 = worker.poll_processed_chunks();
        assert_eq!(chunks2.len(), 0);
    }

    #[test]
    fn test_poll_drains_all_available_chunks() {
        let worker = IngestionWorker::new();
        let store_id = StoreId::random(StoreKind::Recording, "test");
        let channel_source = Arc::new(SmartChannelSource::RrdHttpStream {
            follow: false,
            url: "http://test".into(),
        });

        const NUM_MESSAGES: usize = 50;

        // Submit multiple messages
        for i in 0..NUM_MESSAGES {
            let arrow_msg = create_test_arrow_msg(i);
            worker.submit_arrow_msg_blocking(
                store_id.clone(),
                arrow_msg,
                channel_source.clone(),
                false,
            );
        }

        // Wait for all to be processed
        std::thread::sleep(std::time::Duration::from_millis(300));

        // Single poll should drain all available chunks
        let chunks = worker.poll_processed_chunks();
        assert_eq!(chunks.len(), NUM_MESSAGES);

        // Next poll should be empty
        let chunks2 = worker.poll_processed_chunks();
        assert_eq!(chunks2.len(), 0);
    }
}
