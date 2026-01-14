//! Streaming cache for DataFusion `TableProvider`s.
//!
//! This module provides [`StreamingCacheTableProvider`], a wrapper that caches streaming results
//! from an inner [`TableProvider`]. It enables efficient caching where:
//! - First scan: starts a background task that streams from inner provider while caching
//! - Subsequent scans: returns cached batches first, then continues with new batches
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
use datafusion::execution::{RecordBatchStream, SendableRecordBatchStream, TaskContext};
use datafusion::logical_expr::TableProviderFilterPushDown;
use datafusion::physical_expr::{EquivalenceProperties, Partitioning};
use datafusion::physical_plan::execution_plan::{Boundedness, EmissionType};
use datafusion::physical_plan::{DisplayAs, DisplayFormatType, ExecutionPlan, PlanProperties};
use datafusion::prelude::Expr;
use datafusion::{catalog::TableProvider, datasource::TableType};
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
    fn new() -> Self {
        Self {
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
        // Check if this waker is already registered
        if !self.wakers.iter().any(|w| w.will_wake(waker)) {
            self.wakers.push(waker.clone());
        }
    }
}

/// A [`TableProvider`] that caches streaming results from an inner provider.
///
/// This provider wraps another `TableProvider` and caches the `RecordBatch`es as they
/// stream in. On subsequent scans, it returns cached batches first, then continues
/// streaming new batches as they arrive.
///
/// # Caching Behavior
///
/// - **First scan**: Triggers streaming from the inner provider. Each batch is cached.
/// - **Subsequent scans (while streaming)**: Returns cached batches immediately, then
///   waits for new batches as they arrive.
/// - **After streaming complete**: Returns all cached batches directly.
///
/// # Cache Scope
///
/// The cache stores the **full table** without projection or filters. This allows
/// different queries to share the same cached data.
pub struct StreamingCacheTableProvider {
    /// The wrapped table provider.
    inner_provider: Arc<dyn TableProvider>,
    /// Shared cache state.
    cache: Arc<Mutex<StreamingCacheInner>>,
    /// Runtime handle for spawning background tasks.
    runtime: AsyncRuntimeHandle,
}

impl fmt::Debug for StreamingCacheTableProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamingCacheTableProvider")
            .field("schema", &self.inner_provider.schema())
            .finish_non_exhaustive()
    }
}

impl StreamingCacheTableProvider {
    /// Create a new streaming cache wrapping the given table provider.
    pub fn new(inner: Arc<dyn TableProvider>, runtime: AsyncRuntimeHandle) -> Self {
        Self {
            inner_provider: inner,
            cache: Arc::new(Mutex::new(StreamingCacheInner::new())),
            runtime,
        }
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
        let cache = self.cache.lock();
        cache.state == CacheState::Complete
    }

    /// Get the current number of cached batches.
    pub fn cached_batch_count(&self) -> usize {
        let cache = self.cache.lock();
        cache.cached_batches.len()
    }

    /// Get the current cache state.
    pub fn state(&self) -> CacheState {
        let cache = self.cache.lock();
        cache.state
    }
}

#[async_trait]
impl TableProvider for StreamingCacheTableProvider {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn schema(&self) -> SchemaRef {
        self.inner_provider.schema()
    }

    fn table_type(&self) -> TableType {
        self.inner_provider.table_type()
    }

    async fn scan(
        &self,
        state: &dyn Session,
        _projection: Option<&Vec<usize>>,
        _filters: &[Expr],
        _limit: Option<usize>,
    ) -> DataFusionResult<Arc<dyn ExecutionPlan>> {
        // We ignore projection/filters/limit and cache the full table.
        // The downstream DataFrame operations will apply these.

        // Get the inner execution plan (full table scan)
        let inner_plan = self.inner_provider.scan(state, None, &[], None).await?;

        Ok(Arc::new(CachedExecutionPlan {
            schema: self.schema(),
            cache: Arc::clone(&self.cache),
            inner_plan,
            runtime: self.runtime.clone(),
            properties: PlanProperties::new(
                EquivalenceProperties::new(self.schema()),
                Partitioning::UnknownPartitioning(1),
                EmissionType::Incremental,
                Boundedness::Bounded,
            ),
        }))
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

/// Execution plan that serves cached batches and streams from inner plan.
struct CachedExecutionPlan {
    schema: SchemaRef,
    cache: Arc<Mutex<StreamingCacheInner>>,
    inner_plan: Arc<dyn ExecutionPlan>,
    runtime: AsyncRuntimeHandle,
    properties: PlanProperties,
}

impl CachedExecutionPlan {
    /// Background task: stream from inner plan to cache.
    async fn stream_to_cache(
        mut stream: SendableRecordBatchStream,
        cache: Arc<Mutex<StreamingCacheInner>>,
    ) {
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

impl fmt::Debug for CachedExecutionPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CachedExecutionPlan")
            .field("schema", &self.schema)
            .finish_non_exhaustive()
    }
}

impl DisplayAs for CachedExecutionPlan {
    fn fmt_as(&self, t: DisplayFormatType, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match t {
            DisplayFormatType::Default
            | DisplayFormatType::Verbose
            | DisplayFormatType::TreeRender => {
                write!(f, "CachedExecutionPlan")
            }
        }
    }
}

impl ExecutionPlan for CachedExecutionPlan {
    fn name(&self) -> &str {
        "CachedExecutionPlan"
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
        vec![&self.inner_plan]
    }

    fn with_new_children(
        self: Arc<Self>,
        children: Vec<Arc<dyn ExecutionPlan>>,
    ) -> DataFusionResult<Arc<dyn ExecutionPlan>> {
        if children.len() != 1 {
            return Err(DataFusionError::Internal(
                "CachedExecutionPlan expects exactly one child".to_string(),
            ));
        }
        Ok(Arc::new(Self {
            schema: Arc::clone(&self.schema),
            cache: Arc::clone(&self.cache),
            inner_plan: Arc::clone(&children[0]),
            runtime: self.runtime.clone(),
            properties: self.properties.clone(),
        }))
    }

    fn execute(
        &self,
        partition: usize,
        context: Arc<TaskContext>,
    ) -> DataFusionResult<SendableRecordBatchStream> {
        if partition != 0 {
            return Err(DataFusionError::Internal(format!(
                "CachedExecutionPlan only supports partition 0, got {partition}"
            )));
        }

        Ok(Box::pin(CachedRecordBatchStream::new(
            Arc::clone(&self.schema),
            Arc::clone(&self.cache),
            Arc::clone(&self.inner_plan),
            context,
            self.runtime.clone(),
        )))
    }
}

/// A stream that yields cached batches, waiting for new ones as needed.
struct CachedRecordBatchStream {
    schema: SchemaRef,
    cache: Arc<Mutex<StreamingCacheInner>>,

    /// Current read position in the cache.
    read_pos: usize,

    /// Inner plan and context for spawning background task if needed.
    inner_plan: Option<Arc<dyn ExecutionPlan>>,
    task_context: Option<Arc<TaskContext>>,

    /// Runtime handle for spawning background tasks.
    runtime: AsyncRuntimeHandle,

    /// Whether we've started the background task (if we're the first).
    started: bool,
}

impl CachedRecordBatchStream {
    fn new(
        schema: SchemaRef,
        cache: Arc<Mutex<StreamingCacheInner>>,
        inner_plan: Arc<dyn ExecutionPlan>,
        context: Arc<TaskContext>,
        runtime: AsyncRuntimeHandle,
    ) -> Self {
        Self {
            schema,
            cache,
            read_pos: 0,
            inner_plan: Some(inner_plan),
            task_context: Some(context),
            runtime,
            started: false,
        }
    }

    /// Ensure the background streaming task has been started.
    fn ensure_started(&mut self) -> DataFusionResult<()> {
        if self.started {
            return Ok(());
        }

        // Check if we need to start the stream, and if so, take ownership of plan/ctx
        let should_start = {
            let cache = self.cache.lock();
            cache.state == CacheState::NotStarted
        };

        if should_start {
            if let (Some(plan), Some(ctx)) = (self.inner_plan.take(), self.task_context.take()) {
                // Execute the plan OUTSIDE the lock to avoid blocking issues
                let inner_stream = plan.execute(0, ctx)?;
                let cache_ref = Arc::clone(&self.cache);

                // Now lock to update state and store task handle
                let mut cache = self.cache.lock();

                // Double-check state in case another stream started it
                if cache.state == CacheState::NotStarted {
                    cache.state = CacheState::Streaming;

                    self.runtime.spawn_future(async move {
                        CachedExecutionPlan::stream_to_cache(inner_stream, cache_ref).await;
                    });
                }
            }
        }

        self.started = true;
        Ok(())
    }
}

impl Stream for CachedRecordBatchStream {
    type Item = DataFusionResult<RecordBatch>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Ensure background task is started
        if !self.started {
            if let Err(e) = self.ensure_started() {
                return Poll::Ready(Some(Err(e)));
            }
        }

        // Lock the cache and check state
        let mut cache = self.cache.lock();

        // If there's a batch available at our read position, return it
        if self.read_pos < cache.cached_batches.len() {
            let batch = cache.cached_batches[self.read_pos].clone();
            drop(cache); // Release lock before mutating self
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
                // Register our waker and wait for more data
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
