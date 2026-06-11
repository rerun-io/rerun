//! In-process `TableProvider` over a local [`re_dataframe::external::re_chunk_store::ChunkStore`].
//!
//! This is the single-segment, in-process equivalent of
//! [`crate::DataframeQueryTableProvider`]: it constructs a [`QueryEngine`] over
//! the caller's store and drives a [`re_dataframe::QueryHandle`] from a single-partition
//! `ExecutionPlan`. There is no IO source, no segment fan-out, no
//! `rerun_segment_id` prepend, no pipeline budget, and no analytics.

use std::any::Any;
use std::sync::Arc;

use arrow::array::{RecordBatch, RecordBatchOptions};
use arrow::datatypes::{Fields as ArrowFields, Schema as ArrowSchema, SchemaRef};
use async_trait::async_trait;
use datafusion::catalog::{Session, TableProvider};
use datafusion::common::DataFusionError;
use datafusion::datasource::TableType;
use datafusion::execution::{SendableRecordBatchStream, TaskContext};
use datafusion::logical_expr::Expr;
use datafusion::physical_expr::{EquivalenceProperties, Partitioning};
use datafusion::physical_plan::execution_plan::{Boundedness, EmissionType};
use datafusion::physical_plan::stream::RecordBatchStreamAdapter;
use datafusion::physical_plan::{DisplayAs, DisplayFormatType, ExecutionPlan, PlanProperties};

use re_dataframe::external::re_chunk_store::ChunkStoreHandle;
use re_dataframe::utils::align_record_batch_to_schema;
use re_dataframe::{QueryEngine, QueryExpression, StorageEngine};
use re_sorbet::BatchType;

use crate::dataframe_query_common::{DEFAULT_BATCH_BYTES, DEFAULT_BATCH_ROWS};

/// `TableProvider` that runs a [`QueryExpression`] in-process against a
/// [`ChunkStoreHandle`].
pub struct LocalChunkStoreTableProvider {
    engine: QueryEngine<StorageEngine>,
    query: QueryExpression,
    schema: SchemaRef,
}

impl std::fmt::Debug for LocalChunkStoreTableProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalChunkStoreTableProvider")
            .field("query", &self.query)
            .field("schema", &self.schema)
            .finish_non_exhaustive()
    }
}

impl LocalChunkStoreTableProvider {
    /// Build a provider that queries `store` in-process via [`QueryEngine`].
    ///
    /// The provider holds a fresh [`re_dataframe::QueryCache`]; callers cannot
    /// share caches across providers in v1.
    ///
    /// Fails if `query.filtered_index` names an index that does not exist in
    /// the store's schema.
    pub fn try_new(
        store: ChunkStoreHandle,
        query: QueryExpression,
    ) -> Result<Self, DataFusionError> {
        if let Some(idx) = query.filtered_index.as_ref() {
            let known = store
                .read()
                .schema()
                .chunk_column_descriptors()
                .indices
                .iter()
                .any(|c| c.column_name() == idx.as_str());
            if !known {
                return Err(DataFusionError::Plan(format!(
                    "Index '{idx}' does not exist in the chunk store."
                )));
            }
        }

        let engine = QueryEngine::from_store(store);

        let fields: ArrowFields = engine
            .selected_schema_for_query(&query)
            .iter()
            .map(|d| d.to_arrow_field(BatchType::Dataframe))
            .collect();
        let schema = SchemaRef::from(ArrowSchema::new_with_metadata(fields, Default::default()));

        Ok(Self {
            engine,
            query,
            schema,
        })
    }
}

#[async_trait]
impl TableProvider for LocalChunkStoreTableProvider {
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
        _state: &dyn Session,
        projection: Option<&Vec<usize>>,
        _filters: &[Expr],
        limit: Option<usize>,
    ) -> datafusion::common::Result<Arc<dyn ExecutionPlan>> {
        let projected_schema: SchemaRef = match projection {
            Some(p) => Arc::new(self.schema.project(p)?),
            None => Arc::clone(&self.schema),
        };

        // Projection indices into the unprojected schema; applied via
        // `RecordBatch::project` after we align each batch to `full_schema`.
        let projection_indices: Option<Vec<usize>> = projection.cloned();

        let props = Arc::new(PlanProperties::new(
            EquivalenceProperties::new(Arc::clone(&projected_schema)),
            Partitioning::UnknownPartitioning(1),
            EmissionType::Incremental,
            Boundedness::Bounded,
        ));

        Ok(Arc::new(LocalChunkStoreExec {
            engine: self.engine.clone(),
            query: self.query.clone(),
            full_schema: Arc::clone(&self.schema),
            projected_schema,
            projection_indices,
            props,
            limit,
        }))
    }
}

struct LocalChunkStoreExec {
    engine: QueryEngine<StorageEngine>,
    query: QueryExpression,

    /// The unprojected schema (matches `QueryHandle`'s emitted columns).
    full_schema: SchemaRef,

    /// The schema after applying the scan-time projection (may equal
    /// `full_schema` when projection is `None`).
    projected_schema: SchemaRef,

    /// `None` means "no projection" — emit the unprojected batch as-is.
    projection_indices: Option<Vec<usize>>,
    props: Arc<PlanProperties>,
    limit: Option<usize>,
}

impl std::fmt::Debug for LocalChunkStoreExec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalChunkStoreExec")
            .field("limit", &self.limit)
            .field("projected_schema", &self.projected_schema)
            .field("projection_indices", &self.projection_indices)
            .finish_non_exhaustive()
    }
}

impl DisplayAs for LocalChunkStoreExec {
    fn fmt_as(&self, _t: DisplayFormatType, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "LocalChunkStoreExec: limit={:?}, fields={}",
            self.limit,
            self.projected_schema.fields().len()
        )
    }
}

impl ExecutionPlan for LocalChunkStoreExec {
    fn name(&self) -> &'static str {
        "LocalChunkStoreExec"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn properties(&self) -> &Arc<PlanProperties> {
        &self.props
    }

    fn children(&self) -> Vec<&Arc<dyn ExecutionPlan>> {
        vec![]
    }

    fn with_new_children(
        self: Arc<Self>,
        children: Vec<Arc<dyn ExecutionPlan>>,
    ) -> datafusion::common::Result<Arc<dyn ExecutionPlan>> {
        if children.is_empty() {
            Ok(self)
        } else {
            Err(DataFusionError::Plan(
                "LocalChunkStoreExec does not support children".to_owned(),
            ))
        }
    }

    fn execute(
        &self,
        partition: usize,
        _context: Arc<TaskContext>,
    ) -> datafusion::common::Result<SendableRecordBatchStream> {
        if partition != 0 {
            return Err(DataFusionError::Internal(format!(
                "LocalChunkStoreExec only supports partition 0, got {partition}"
            )));
        }

        let engine = self.engine.clone();
        let query = self.query.clone();
        let full_schema = Arc::clone(&self.full_schema);
        let projected_schema = Arc::clone(&self.projected_schema);
        let projection_indices = self.projection_indices.clone();
        let limit = self.limit;
        let schema_for_adapter = Arc::clone(&projected_schema);

        let stream = async_stream::try_stream! {
            // An empty store still yields a single phantom row out of the
            // static-only emit path. Suppress it: a zero-field schema means
            // the user asked for nothing, so emit nothing.
            if full_schema.fields().is_empty() {
                return;
            }

            let mut query_handle = engine.query(query);
            let mut rows_sent: usize = 0;

            loop {
                let remaining = match limit {
                    Some(l) => {
                        if rows_sent >= l {
                            break;
                        }
                        l - rows_sent
                    }
                    None => usize::MAX,
                };
                let max_rows = remaining.min(DEFAULT_BATCH_ROWS);

                let next = query_handle
                    .next_n_rows_async(max_rows, DEFAULT_BATCH_BYTES as usize)
                    .await;
                if next.num_rows == 0 {
                    break;
                }

                let query_schema = Arc::clone(query_handle.schema());
                let batch = RecordBatch::try_new_with_options(
                    query_schema,
                    next.columns,
                    &RecordBatchOptions::default().with_row_count(Some(next.num_rows)),
                )
                .map_err(|err| DataFusionError::ArrowError(Box::new(err), None))?;

                // Align to the unprojected schema (handles nullability /
                // type widening between `QueryHandle::schema()` and our
                // cached `full_schema`).
                let aligned = align_record_batch_to_schema(&batch, &full_schema)
                    .map_err(|err| DataFusionError::ArrowError(Box::new(err), None))?;

                let projected = match projection_indices.as_deref() {
                    None => aligned,
                    Some(indices) => aligned
                        .project(indices)
                        .map_err(|err| DataFusionError::ArrowError(Box::new(err), None))?,
                };

                let to_yield = if let Some(l) = limit {
                    let remaining = l.saturating_sub(rows_sent);
                    if projected.num_rows() > remaining {
                        projected.slice(0, remaining)
                    } else {
                        projected
                    }
                } else {
                    projected
                };

                rows_sent += to_yield.num_rows();
                yield to_yield;

                if limit.is_some_and(|l| rows_sent >= l) {
                    break;
                }
            }
        };

        Ok(Box::pin(RecordBatchStreamAdapter::new(
            schema_for_adapter,
            stream,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use arrow::array::{Array as _, Int64Array, RecordBatch};
    use datafusion::prelude::SessionContext;
    use futures::StreamExt as _;
    use re_dataframe::external::re_chunk::{Chunk, RowId, TimePoint};
    use re_dataframe::external::re_chunk_store::{
        ChunkStore, ChunkStoreConfig, StaticColumnSelection,
    };
    use re_dataframe::external::re_log_types::example_components::{MyPoint, MyPoints};
    use re_dataframe::external::re_log_types::{EntityPath, StoreId, StoreKind, TimeInt, Timeline};

    fn new_store() -> ChunkStoreHandle {
        let store_id = StoreId::random(StoreKind::Recording, "test-app");
        ChunkStore::new_handle(store_id, ChunkStoreConfig::ALL_DISABLED)
    }

    fn build_point_chunk(path: &str, ts: &[i64]) -> Chunk {
        let timeline = Timeline::new_sequence("t");
        let mut builder = Chunk::builder(EntityPath::from(path));
        for t in ts {
            let tp = TimePoint::default().with(timeline, TimeInt::new_temporal(*t));
            #[expect(clippy::cast_sign_loss)]
            let pt = MyPoint::from_iter((*t as u32)..(*t as u32 + 1));
            builder = builder.with_archetype(RowId::new(), tp, &MyPoints::new(pt));
        }
        builder.build().unwrap()
    }

    async fn collect_all(provider: LocalChunkStoreTableProvider) -> Vec<RecordBatch> {
        let ctx = SessionContext::new();
        ctx.register_table("t", Arc::new(provider)).unwrap();
        let df = ctx.sql("SELECT * FROM t").await.unwrap();
        df.collect().await.unwrap()
    }

    #[tokio::test]
    async fn smoke() {
        let store = new_store();
        store
            .write()
            .insert_chunk(&Arc::new(build_point_chunk("/a", &[1, 2, 3])))
            .unwrap();
        store
            .write()
            .insert_chunk(&Arc::new(build_point_chunk("/b", &[1, 2])))
            .unwrap();

        let query = QueryExpression {
            filtered_index: Some("t".into()),
            include_static_columns: StaticColumnSelection::Both,
            ..Default::default()
        };
        let provider = LocalChunkStoreTableProvider::try_new(store, query).unwrap();
        let schema = provider.schema();
        assert!(
            schema.fields().iter().any(|f| f.name() == "t"),
            "expected `t` index column, fields={:?}",
            schema.fields().iter().map(|f| f.name()).collect::<Vec<_>>()
        );

        let batches = collect_all(provider).await;
        let total: usize = batches.iter().map(|b| b.num_rows()).sum();
        assert!(total > 0, "expected non-zero rows");
    }

    #[tokio::test]
    async fn limit() {
        let store = new_store();
        let ts: Vec<i64> = (0..100).collect();
        store
            .write()
            .insert_chunk(&Arc::new(build_point_chunk("/a", &ts)))
            .unwrap();

        let query = QueryExpression {
            filtered_index: Some("t".into()),
            include_static_columns: StaticColumnSelection::Both,
            ..Default::default()
        };
        let provider = LocalChunkStoreTableProvider::try_new(store, query).unwrap();

        let ctx = SessionContext::new();
        ctx.register_table("t", Arc::new(provider)).unwrap();
        let df = ctx.sql("SELECT * FROM t LIMIT 3").await.unwrap();
        let batches = df.collect().await.unwrap();
        let total: usize = batches.iter().map(|b| b.num_rows()).sum();
        assert_eq!(total, 3);
    }

    #[tokio::test]
    async fn projection() {
        let store = new_store();
        let ts: Vec<i64> = (0..10).collect();
        store
            .write()
            .insert_chunk(&Arc::new(build_point_chunk("/a", &ts)))
            .unwrap();

        let query = QueryExpression {
            filtered_index: Some("t".into()),
            include_static_columns: StaticColumnSelection::Both,
            ..Default::default()
        };
        let provider = LocalChunkStoreTableProvider::try_new(store, query).unwrap();
        let schema = provider.schema();
        let t_name = schema
            .fields()
            .iter()
            .find(|f| f.name() == "t")
            .expect("`t` index field present")
            .name()
            .clone();

        let ctx = SessionContext::new();
        ctx.register_table("t", Arc::new(provider)).unwrap();
        let df = ctx.sql(&format!("SELECT {t_name:?} FROM t")).await.unwrap();
        let batches = df.collect().await.unwrap();
        assert!(!batches.is_empty());
        let total: usize = batches.iter().map(|b| b.num_rows()).sum();
        assert_eq!(total, 10);
        for b in &batches {
            assert_eq!(b.num_columns(), 1, "expected single projected column");
            assert_eq!(b.schema().field(0).name(), &t_name);
            let _ = b
                .column(0)
                .as_any()
                .downcast_ref::<Int64Array>()
                .expect("`t` is Int64");
        }
    }

    #[tokio::test]
    async fn empty_store() {
        // No `filtered_index` — the static-only query is the only valid query
        // against an empty store, since no index exists yet.
        let store = new_store();
        let query = QueryExpression {
            filtered_index: None,
            include_static_columns: StaticColumnSelection::StaticOnly,
            ..Default::default()
        };
        let provider = LocalChunkStoreTableProvider::try_new(store, query).unwrap();
        assert_eq!(
            provider.schema().fields().len(),
            0,
            "empty store schema must be empty"
        );

        let batches = collect_all(provider).await;
        let total: usize = batches.iter().map(|b| b.num_rows()).sum();
        assert_eq!(total, 0, "empty store must emit zero rows");
    }

    #[tokio::test]
    async fn selection_reorders_and_keeps_only_picked_columns() {
        use re_sorbet::{ColumnSelector, TimeColumnSelector};

        let store = new_store();
        store
            .write()
            .insert_chunk(&Arc::new(build_point_chunk("/a", &[1, 2, 3])))
            .unwrap();

        // Selection: just the `t` index — drops the component column.
        let query = QueryExpression {
            filtered_index: Some("t".into()),
            include_static_columns: StaticColumnSelection::Both,
            selection: Some(vec![ColumnSelector::Time(TimeColumnSelector::from("t"))]),
            ..Default::default()
        };
        let provider = LocalChunkStoreTableProvider::try_new(store, query).unwrap();
        let schema = provider.schema();
        assert_eq!(schema.fields().len(), 1, "selection should pick 1 column");
        assert_eq!(schema.field(0).name(), "t");

        let batches = collect_all(provider).await;
        let total: usize = batches.iter().map(|b| b.num_rows()).sum();
        assert_eq!(total, 3);
        for b in &batches {
            assert_eq!(b.num_columns(), 1);
            assert_eq!(b.schema().field(0).name(), "t");
        }
    }

    #[tokio::test]
    async fn unknown_index_rejected() {
        let store = new_store();
        store
            .write()
            .insert_chunk(&Arc::new(build_point_chunk("/a", &[1, 2, 3])))
            .unwrap();

        let query = QueryExpression {
            filtered_index: Some("does_not_exist".into()),
            include_static_columns: StaticColumnSelection::Both,
            ..Default::default()
        };
        let err = LocalChunkStoreTableProvider::try_new(store, query).unwrap_err();
        assert!(
            err.to_string().contains("does not exist"),
            "expected 'does not exist' in error, got: {err}"
        );
    }

    #[tokio::test]
    async fn multi_batch() {
        let store = new_store();
        let n: i64 = 5000;
        let ts: Vec<i64> = (0..n).collect();
        store
            .write()
            .insert_chunk(&Arc::new(build_point_chunk("/a", &ts)))
            .unwrap();

        let query = QueryExpression {
            filtered_index: Some("t".into()),
            include_static_columns: StaticColumnSelection::Both,
            ..Default::default()
        };
        let provider = LocalChunkStoreTableProvider::try_new(store, query).unwrap();
        let ctx = SessionContext::new();
        ctx.register_table("t", Arc::new(provider)).unwrap();

        // `df.collect()` runs through `SizedCoalesceBatchesExec` which would
        // mask source-side batching. Drive the physical plan directly.
        let df = ctx.sql("SELECT * FROM t").await.unwrap();
        let physical = df.create_physical_plan().await.unwrap();
        let mut stream = physical.execute(0, ctx.task_ctx()).unwrap();
        let mut batches = Vec::new();
        while let Some(b) = stream.next().await {
            batches.push(b.unwrap());
        }

        assert!(
            batches.len() >= 2,
            "expected >= 2 batches, got {}",
            batches.len()
        );
        let total: usize = batches.iter().map(|b| b.num_rows()).sum();
        assert_eq!(total, n as usize);
        let half = DEFAULT_BATCH_ROWS / 2;
        for (i, b) in batches.iter().enumerate() {
            if i + 1 == batches.len() {
                continue;
            }
            assert!(
                b.num_rows() >= half,
                "non-trailing batch {i} too small: {} < {half}",
                b.num_rows()
            );
        }
    }
}
