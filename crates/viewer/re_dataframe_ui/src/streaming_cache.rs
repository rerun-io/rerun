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
use futures::{Stream, StreamExt as _};
use parking_lot::Mutex;
use re_viewer_context::AsyncRuntimeHandle;

/// State of the streaming cache.
#[derive(Debug, Clone)]
pub enum CacheState {
    NotStarted,
    Streaming,
    Complete(Arc<MemTable>),
    Failed(Arc<DataFusionError>),
}

/// Internal shared state for the streaming cache.
#[derive(Debug)]
struct StreamingCacheInner {
    schema: SchemaRef,
    cached_batches: Vec<RecordBatch>,
    state: CacheState,
    wakers: Vec<Waker>,
}

impl StreamingCacheInner {
    fn new(schema: SchemaRef) -> Self {
        Self {
            schema,
            cached_batches: Vec::new(),
            state: CacheState::NotStarted,
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

/// An async closure that creates a [`DataFrame`] for streaming.
pub type DataFrameFactory = Box<dyn Fn() -> DataFrameFuture + Send + Sync>;

/// A future that resolves to a [`DataFrame`].
pub type DataFrameFuture = Pin<Box<dyn Future<Output = DataFusionResult<DataFrame>> + Send>>;

/// A [`TableProvider`] that caches streaming results from a [`DataFrame`].
///
/// This provider executes a [`DataFrame`] and caches the results as they stream in.
/// On subsequent scans, it returns cached batches first, then continues with
/// new batches as they arrive.
///
/// # Caching Behavior
///
/// - **First scan**: Triggers streaming from the [`DataFrame`]. Each batch is cached.
/// - **Subsequent scans (while streaming)**: Returns cached batches immediately,
///   then waits for new batches as they arrive.
/// - **After streaming complete**: Returns all cached batches via a [`MemTable`].
///
/// # Refresh Behavior
///
/// When `refresh()` is called, a new inner cache is created. Old streams continue
/// reading from the old cache until completion, while new scans use the fresh cache.
pub struct StreamingCacheTableProvider {
    /// Factory to create the [`DataFrame`] (called once on first scan).
    df_factory: DataFrameFactory,

    /// Schema for the cached data.
    schema: SchemaRef,

    /// Shared cache state. The outer Mutex allows swapping the inner cache on refresh.
    cache: Mutex<Arc<Mutex<StreamingCacheInner>>>,

    runtime: AsyncRuntimeHandle,
}

impl fmt::Debug for StreamingCacheTableProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inner_cache = self.cache.lock();
        let inner = inner_cache.lock();
        f.debug_struct("StreamingCacheTableProvider")
            .field("schema", &self.schema)
            .field("state", &inner.state)
            .field("cached_batches", &inner.cached_batches.len())
            .finish_non_exhaustive()
    }
}

impl StreamingCacheTableProvider {
    /// Create a new streaming cache table provider.
    ///
    /// The `df_factory` is called once on the first scan to create the [`DataFrame`]
    /// that will be streamed and cached.
    pub fn new(
        schema: SchemaRef,
        df_factory: DataFrameFactory,
        runtime: AsyncRuntimeHandle,
    ) -> Self {
        Self {
            df_factory,
            schema: Arc::clone(&schema),
            cache: Mutex::new(Arc::new(Mutex::new(StreamingCacheInner::new(schema)))),
            runtime,
        }
    }

    /// Create from a session context and table name.
    ///
    /// This is a convenience constructor that creates the [`DataFrame`] factory
    /// from the session context.
    pub fn from_session_table(
        session_ctx: Arc<SessionContext>,
        table_name: String,
        schema: SchemaRef,
        runtime: AsyncRuntimeHandle,
    ) -> Self {
        let df_factory: DataFrameFactory = Box::new(move || {
            let ctx = Arc::clone(&session_ctx);
            let table_name = table_name.clone();
            Box::pin(async move { ctx.table(&table_name).await })
        });

        Self::new(schema, df_factory, runtime)
    }

    /// Invalidate the cache and prepare for a fresh stream on next scan.
    ///
    /// Old streams will continue reading from the old cache until completion.
    /// New scans will use a fresh cache.
    pub fn refresh(&self) {
        let mut cache = self.cache.lock();
        *cache = Arc::new(Mutex::new(StreamingCacheInner::new(Arc::clone(
            &self.schema,
        ))));
    }

    /// Check if the cache is complete (all data received).
    pub fn is_complete(&self) -> bool {
        matches!(self.cache.lock().lock().state, CacheState::Complete(_))
    }

    /// Get the current number of cached batches.
    pub fn cached_batch_count(&self) -> usize {
        self.cache.lock().lock().cached_batches.len()
    }

    /// Get the current cache state.
    pub fn state(&self) -> CacheState {
        self.cache.lock().lock().state.clone()
    }

    /// Background task: stream from [`DataFrame`] to cache.
    ///
    /// Stops early if no consumers remain (detected via `Arc` strong count).
    async fn stream_to_cache(
        df_future: DataFrameFuture,
        cache: &Arc<Mutex<StreamingCacheInner>>,
    ) -> DataFusionResult<()> {
        let dataframe = df_future.await?;
        let mut stream = dataframe.execute_stream().await?;

        // Stream batches into cache
        while let Some(result) = stream.next().await {
            let batch = result?;

            let mut guard = cache.lock();
            guard.cached_batches.push(batch);
            guard.wake_all();

            if Arc::strong_count(cache) == 1 {
                // No more readers - stop streaming
                return Ok(());
            }
        }

        // Stream complete - build MemTable for efficient future scans
        let mut guard = cache.lock();
        // We can't mem::take the barches since some readers might still be in progress
        let batches = guard.cached_batches.clone();
        let mem_table = MemTable::try_new(Arc::clone(&guard.schema), vec![batches])?;
        guard.state = CacheState::Complete(Arc::new(mem_table));
        guard.wake_all();
        Ok(())
    }
}

#[async_trait]
impl TableProvider for StreamingCacheTableProvider {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
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
            UseMemTable(Arc<MemTable>),
            ReturnError(Arc<DataFusionError>),
            StartStreaming(Arc<Mutex<StreamingCacheInner>>),
            ReturnStreamingPlan(Arc<Mutex<StreamingCacheInner>>),
        }

        let action = {
            let inner_cache = Arc::clone(&self.cache.lock());
            let mut inner = inner_cache.lock();
            match &inner.state {
                CacheState::Complete(mem_table) => Action::UseMemTable(Arc::clone(mem_table)),
                CacheState::Failed(err) => Action::ReturnError(Arc::clone(err)),
                CacheState::NotStarted => {
                    inner.state = CacheState::Streaming;
                    drop(inner);
                    Action::StartStreaming(inner_cache)
                }
                CacheState::Streaming => {
                    drop(inner);
                    Action::ReturnStreamingPlan(inner_cache)
                }
            }
        };

        match action {
            Action::UseMemTable(mem_table) => {
                mem_table.scan(state, projection, filters, limit).await
            }
            Action::ReturnError(error) => Err(DataFusionError::Shared(error)),
            Action::StartStreaming(inner_cache) => {
                let df_future = (self.df_factory)();
                let cache_ref = Arc::clone(&inner_cache);
                self.runtime.spawn_future(async move {
                    if let Err(err) = Self::stream_to_cache(df_future, &cache_ref).await {
                        let mut guard = cache_ref.lock();
                        guard.state = CacheState::Failed(Arc::new(err));
                        guard.wake_all();
                    }
                });

                Ok(Arc::new(CachedStreamingExec::new(inner_cache)))
            }
            Action::ReturnStreamingPlan(inner_cache) => {
                Ok(Arc::new(CachedStreamingExec::new(inner_cache)))
            }
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
    cache: Arc<Mutex<StreamingCacheInner>>,
    properties: PlanProperties,
}

impl CachedStreamingExec {
    fn new(cache: Arc<Mutex<StreamingCacheInner>>) -> Self {
        let schema = Arc::clone(&cache.lock().schema);
        let properties = PlanProperties::new(
            EquivalenceProperties::new(schema),
            Partitioning::UnknownPartitioning(1),
            EmissionType::Incremental,
            Boundedness::Bounded,
        );
        Self { cache, properties }
    }
}

impl fmt::Debug for CachedStreamingExec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CachedStreamingExec")
            .field("schema", &self.cache.lock().schema)
            .finish_non_exhaustive()
    }
}

impl DisplayAs for CachedStreamingExec {
    fn fmt_as(&self, _t: DisplayFormatType, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CachedStreamingExec")
    }
}

impl ExecutionPlan for CachedStreamingExec {
    fn name(&self) -> &'static str {
        "CachedStreamingExec"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.cache.lock().schema)
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
                "CachedStreamingExec expects no children".to_owned(),
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

        Ok(Box::pin(CachedRecordBatchStream::new(Arc::clone(
            &self.cache,
        ))))
    }
}

/// A stream that yields cached batches, waiting for new ones as needed.
pub struct CachedRecordBatchStream {
    cache: Arc<Mutex<StreamingCacheInner>>,

    /// Current read position in the cache.
    read_pos: usize,
}

impl CachedRecordBatchStream {
    fn new(cache: Arc<Mutex<StreamingCacheInner>>) -> Self {
        Self { cache, read_pos: 0 }
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
        match &cache.state {
            CacheState::Complete(_) => Poll::Ready(None),
            CacheState::Failed(err) => {
                Poll::Ready(Some(Err(DataFusionError::Shared(Arc::clone(err)))))
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
        Arc::clone(&self.cache.lock().schema)
    }
}
