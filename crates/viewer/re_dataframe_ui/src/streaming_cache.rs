//! Streaming cache for DataFusion table providers.
//!
//! This module provides [`StreamingCacheTableProvider`], a `TableProvider` that caches
//! streaming results from executing a DataFrame. It enables efficient caching where:
//! - First scan: starts a background task that streams from DataFrame while caching
//! - Subsequent scans: returns cached batches immediately via a MemTable
//! - All concurrent scans share the same cache
//! - Cancelling a consumer does NOT stop the background streaming

use std::any::Any;
use std::fmt;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use arrow::array::RecordBatch;
use arrow::datatypes::SchemaRef;
use async_trait::async_trait;
use datafusion::catalog::Session;
use datafusion::common::{DataFusionError, Result as DataFusionResult};
use datafusion::datasource::TableType;
use datafusion::execution::{RecordBatchStream, SendableRecordBatchStream, TaskContext};
use datafusion::logical_expr::TableProviderFilterPushDown;
use datafusion::physical_expr::{EquivalenceProperties, Partitioning};
use datafusion::physical_plan::execution_plan::{Boundedness, EmissionType};
use datafusion::physical_plan::{DisplayAs, DisplayFormatType, ExecutionPlan, PlanProperties};
use datafusion::prelude::{DataFrame, Expr, SessionContext};
use datafusion::{catalog::TableProvider, datasource::MemTable};
use futures::{Stream, StreamExt};
use parking_lot::Mutex;
use re_viewer_context::AsyncRuntimeHandle;

/// State of the streaming cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheState {
    /// Stream has not been started yet.
    NotStarted,
    /// Stream is actively running.
    Streaming,
    /// Stream completed successfully.
    Complete,
    /// Stream failed with an error.
    Failed,
}

/// Internal shared state for the streaming cache.
struct StreamingCacheInner {
    /// Schema of the cached data.
    schema: SchemaRef,
    /// Cached record batches.
    cached_batches: Vec<RecordBatch>,
    /// Current state of the cache.
    state: CacheState,
    /// Error message if stream failed.
    error: Option<String>,
    /// Wakers waiting for new data.
    wakers: Vec<Waker>,
}

impl StreamingCacheInner {
    fn new(schema: SchemaRef) -> Self {
        Self {
            schema,
            cached_batches: Vec::new(),
            state: CacheState::NotStarted,
            error: None,
            wakers: Vec::new(),
        }
    }

    /// Wake all registered wakers.
    fn wake_all(&mut self) {
        for waker in self.wakers.drain(..) {
            waker.wake();
        }
    }

    /// Register a waker to be notified when data changes.
    fn register_waker(&mut self, waker: &Waker) {
        if !self.wakers.iter().any(|w| w.will_wake(waker)) {
            self.wakers.push(waker.clone());
        }
    }
}

/// A closure that creates a DataFrame for streaming.
pub type DataFrameFactory = Box<dyn Fn() -> DataFusionResult<DataFrame> + Send + Sync>;

/// A [`TableProvider`] that caches streaming results from a DataFrame.
///
/// This provider executes a DataFrame and caches the results as they stream in.
/// On subsequent scans, it returns cached batches first, then continues with
/// new batches as they arrive.
///
/// # Caching Behavior
///
/// - **First scan**: Triggers streaming from the DataFrame. Each batch is cached.
/// - **Subsequent scans (while streaming)**: Returns cached batches immediately,
///   then waits for new batches as they arrive.
/// - **After streaming complete**: Returns all cached batches via a MemTable.
pub struct StreamingCacheTableProvider {
    /// Factory to create the DataFrame (called once on first scan).
    df_factory: DataFrameFactory,
    /// Shared cache state.
    cache: Arc<Mutex<StreamingCacheInner>>,
    /// Runtime handle for spawning background tasks.
    runtime: AsyncRuntimeHandle,
}

impl fmt::Debug for StreamingCacheTableProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cache = self.cache.lock();
        f.debug_struct("StreamingCacheTableProvider")
            .field("schema", &cache.schema)
            .field("state", &cache.state)
            .field("cached_batches", &cache.cached_batches.len())
            .finish_non_exhaustive()
    }
}

impl StreamingCacheTableProvider {
    /// Create a new streaming cache table provider.
    ///
    /// The `df_factory` is called once on the first scan to create the DataFrame
    /// that will be streamed and cached.
    pub fn new(
        schema: SchemaRef,
        df_factory: DataFrameFactory,
        runtime: AsyncRuntimeHandle,
    ) -> Self {
        Self {
            df_factory,
            cache: Arc::new(Mutex::new(StreamingCacheInner::new(schema))),
            runtime,
        }
    }

    /// Create from a session context and table name.
    ///
    /// This is a convenience constructor that creates the DataFrame factory
    /// from the session context.
    pub fn from_session_table(
        session_ctx: Arc<SessionContext>,
        table_name: String,
        schema: SchemaRef,
        runtime: AsyncRuntimeHandle,
    ) -> Self {
        let df_factory: DataFrameFactory = Box::new(move || {
            // We need to block on the async table() call.
            // This is safe because it's called from within an async context.
            futures::executor::block_on(session_ctx.table(&table_name))
        });

        Self::new(schema, df_factory, runtime)
    }

    /// Invalidate the cache and prepare for a fresh stream on next scan.
    pub fn refresh(&self) {
        let mut cache = self.cache.lock();
        cache.cached_batches.clear();
        cache.state = CacheState::NotStarted;
        cache.error = None;
        cache.wakers.clear();
    }

    /// Check if the cache is complete (all data received).
    pub fn is_complete(&self) -> bool {
        self.cache.lock().state == CacheState::Complete
    }

    /// Get the current number of cached batches.
    pub fn cached_batch_count(&self) -> usize {
        self.cache.lock().cached_batches.len()
    }

    /// Get the current cache state.
    pub fn state(&self) -> CacheState {
        self.cache.lock().state
    }

    /// Background task: stream from DataFrame to cache.
    async fn stream_to_cache(
        df_result: DataFusionResult<DataFrame>,
        cache: Arc<Mutex<StreamingCacheInner>>,
    ) {
        let dataframe = match df_result {
            Ok(df) => df,
            Err(err) => {
                let mut guard = cache.lock();
                guard.state = CacheState::Failed;
                guard.error = Some(err.to_string());
                guard.wake_all();
                return;
            }
        };

        let stream_result = dataframe.execute_stream().await;

        let mut stream = match stream_result {
            Ok(s) => s,
            Err(err) => {
                let mut guard = cache.lock();
                guard.state = CacheState::Failed;
                guard.error = Some(err.to_string());
                guard.wake_all();
                return;
            }
        };

        // Stream batches into cache
        loop {
            match stream.next().await {
                Some(Ok(batch)) => {
                    let mut guard = cache.lock();
                    guard.cached_batches.push(batch);
                    guard.wake_all();
                }
                Some(Err(err)) => {
                    let mut guard = cache.lock();
                    guard.state = CacheState::Failed;
                    guard.error = Some(err.to_string());
                    guard.wake_all();
                    return;
                }
                None => {
                    let mut guard = cache.lock();
                    guard.state = CacheState::Complete;
                    guard.wake_all();
                    return;
                }
            }
        }
    }
}

#[async_trait]
impl TableProvider for StreamingCacheTableProvider {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn schema(&self) -> SchemaRef {
        self.cache.lock().schema.clone()
    }

    fn table_type(&self) -> TableType {
        TableType::Base
    }

    async fn scan(
        &self,
        state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> DataFusionResult<Arc<dyn ExecutionPlan>> {
        enum Action {
            UseMemTable(Vec<RecordBatch>),
            ReturnError(String),
            StartStreaming,
            ReturnStreamingPlan,
        }

        let action = {
            let mut cache = self.cache.lock();
            match cache.state {
                CacheState::Complete => Action::UseMemTable(cache.cached_batches.clone()),
                CacheState::Failed => Action::ReturnError(
                    cache
                        .error
                        .clone()
                        .unwrap_or_else(|| "Unknown error".to_string()),
                ),
                CacheState::NotStarted => {
                    cache.state = CacheState::Streaming;
                    Action::StartStreaming
                }
                CacheState::Streaming => Action::ReturnStreamingPlan,
            }
        };

        match action {
            Action::UseMemTable(batches) => {
                let schema = self.schema();
                let mem_table = MemTable::try_new(schema, vec![batches])?;
                mem_table.scan(state, projection, filters, limit).await
            }
            Action::ReturnError(error) => Err(DataFusionError::Execution(error)),
            Action::StartStreaming => {
                let df_result = (self.df_factory)();
                let cache_ref = Arc::clone(&self.cache);
                self.runtime.spawn_future(async move {
                    Self::stream_to_cache(df_result, cache_ref).await;
                });

                Ok(Arc::new(CachedStreamingExec::new(
                    self.schema(),
                    Arc::clone(&self.cache),
                )))
            }
            Action::ReturnStreamingPlan => Ok(Arc::new(CachedStreamingExec::new(
                self.schema(),
                Arc::clone(&self.cache),
            ))),
        }
    }

    fn supports_filters_pushdown(
        &self,
        filters: &[&Expr],
    ) -> DataFusionResult<Vec<TableProviderFilterPushDown>> {
        // We don't push down filters since we cache the full table
        Ok(vec![
            TableProviderFilterPushDown::Unsupported;
            filters.len()
        ])
    }
}

/// Execution plan that streams from the cache.
struct CachedStreamingExec {
    schema: SchemaRef,
    cache: Arc<Mutex<StreamingCacheInner>>,
    properties: PlanProperties,
}

impl CachedStreamingExec {
    fn new(schema: SchemaRef, cache: Arc<Mutex<StreamingCacheInner>>) -> Self {
        let properties = PlanProperties::new(
            EquivalenceProperties::new(Arc::clone(&schema)),
            Partitioning::UnknownPartitioning(1),
            EmissionType::Incremental,
            Boundedness::Bounded,
        );
        Self {
            schema,
            cache,
            properties,
        }
    }
}

impl fmt::Debug for CachedStreamingExec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CachedStreamingExec")
            .field("schema", &self.schema)
            .finish_non_exhaustive()
    }
}

impl DisplayAs for CachedStreamingExec {
    fn fmt_as(&self, t: DisplayFormatType, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match t {
            DisplayFormatType::Default
            | DisplayFormatType::Verbose
            | DisplayFormatType::TreeRender => {
                write!(f, "CachedStreamingExec")
            }
        }
    }
}

impl ExecutionPlan for CachedStreamingExec {
    fn name(&self) -> &str {
        "CachedStreamingExec"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }

    fn properties(&self) -> &PlanProperties {
        &self.properties
    }

    fn children(&self) -> Vec<&Arc<dyn ExecutionPlan>> {
        vec![]
    }

    fn with_new_children(
        self: Arc<Self>,
        children: Vec<Arc<dyn ExecutionPlan>>,
    ) -> DataFusionResult<Arc<dyn ExecutionPlan>> {
        if !children.is_empty() {
            return Err(DataFusionError::Internal(
                "CachedStreamingExec expects no children".to_string(),
            ));
        }
        Ok(self)
    }

    fn execute(
        &self,
        partition: usize,
        _context: Arc<TaskContext>,
    ) -> DataFusionResult<SendableRecordBatchStream> {
        if partition != 0 {
            return Err(DataFusionError::Internal(format!(
                "CachedStreamingExec only supports partition 0, got {partition}"
            )));
        }

        Ok(Box::pin(CachedRecordBatchStream::new(
            Arc::clone(&self.schema),
            Arc::clone(&self.cache),
        )))
    }
}

/// A stream that yields cached batches, waiting for new ones as needed.
pub struct CachedRecordBatchStream {
    schema: SchemaRef,
    cache: Arc<Mutex<StreamingCacheInner>>,
    /// Current read position in the cache.
    read_pos: usize,
}

impl CachedRecordBatchStream {
    fn new(schema: SchemaRef, cache: Arc<Mutex<StreamingCacheInner>>) -> Self {
        Self {
            schema,
            cache,
            read_pos: 0,
        }
    }
}

impl Stream for CachedRecordBatchStream {
    type Item = DataFusionResult<RecordBatch>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut cache = self.cache.lock();

        // If there's a batch available at our read position, return it
        if self.read_pos < cache.cached_batches.len() {
            let batch = cache.cached_batches[self.read_pos].clone();
            drop(cache);
            self.read_pos += 1;
            return Poll::Ready(Some(Ok(batch)));
        }

        // No more batches available - check state
        match cache.state {
            CacheState::Complete => Poll::Ready(None),
            CacheState::Failed => {
                let error = cache
                    .error
                    .clone()
                    .unwrap_or_else(|| "Unknown error".to_string());
                Poll::Ready(Some(Err(DataFusionError::Execution(error))))
            }
            CacheState::NotStarted | CacheState::Streaming => {
                cache.register_waker(cx.waker());
                Poll::Pending
            }
        }
    }
}

impl RecordBatchStream for CachedRecordBatchStream {
    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }
}
