use std::sync::Arc;

use arrow::datatypes::{DataType, SchemaRef};
use datafusion::common::{DataFusionError, TableReference};
use datafusion::functions::expr_fn::concat;
use datafusion::logical_expr::{binary_expr, col as datafusion_col, lit};
use datafusion::prelude::{SessionContext, cast, encode};
use parking_lot::Mutex;

use re_log_types::Timestamp;
use re_sorbet::{BatchType, SorbetBatch};
use re_viewer_context::AsyncRuntimeHandle;

use crate::table_blueprint::{EntryLinksSpec, PartitionLinksSpec, SortBy, TableBlueprint};
use crate::{ColumnFilter, RequestedObject};

/// Make sure we escape column names correctly for datafusion.
///
/// Background: even when round-tripping column names from the very schema that datafusion returns,
/// it can happen that column names have the "wrong" case and must be escaped. See this issue:
/// <https://github.com/apache/datafusion/issues/15922>
///
/// This function is named such as to replace the datafusion's `col` function, so we do the right
/// thing even if we forget about it.
fn col(name: &str) -> datafusion::logical_expr::Expr {
    datafusion_col(format!("{name:?}"))
}

/// The subset of [`TableBlueprint`] that is actually handled by datafusion.
///
/// In general, there are aspects of a table blueprint that are handled by the UI in an immediate
/// mode fashion (e.g. is a column visible?), and other aspects that are handled by datafusion (e.g.
/// sorting). This struct is for the latter.
#[derive(Debug, Clone, PartialEq, Default)]
struct DataFusionQueryData {
    pub sort_by: Option<SortBy>,
    pub partition_links: Option<PartitionLinksSpec>,
    pub entry_links: Option<EntryLinksSpec>,
    pub prefilter: Option<datafusion::prelude::Expr>,
    pub column_filters: Vec<ColumnFilter>,
}

impl From<&TableBlueprint> for DataFusionQueryData {
    fn from(value: &TableBlueprint) -> Self {
        let TableBlueprint {
            sort_by,
            partition_links,
            entry_links,
            prefilter,
            column_filters,
        } = value;

        Self {
            sort_by: sort_by.clone(),
            partition_links: partition_links.clone(),
            entry_links: entry_links.clone(),
            prefilter: prefilter.clone(),
            column_filters: column_filters.clone(),
        }
    }
}

/// Result of the async datafusion query process.
#[derive(Debug, Clone)]
pub struct DataFusionQueryResult {
    /// The record batches to display.
    pub sorbet_batches: Vec<SorbetBatch>,

    /// The schema of the record batches.
    pub original_schema: SchemaRef,

    /// The migrated schema of the record batches (useful when the list of batches is empty).
    pub sorbet_schema: re_sorbet::SorbetSchema,
}

/// A table blueprint along with the context required to execute the corresponding datafusion query.
#[derive(Clone)]
struct DataFusionQuery {
    session_ctx: Arc<SessionContext>,
    table_ref: TableReference,

    query_data: DataFusionQueryData,
}

impl DataFusionQuery {
    fn new(
        session_ctx: Arc<SessionContext>,
        table_ref: TableReference,
        query_data: DataFusionQueryData,
    ) -> Self {
        Self {
            session_ctx,
            table_ref,
            query_data,
        }
    }

    /// Execute the query to produce the data to display.
    ///
    /// Note: the future returned by this function must be `'static`, so it takes `self`. Use
    /// `clone()` as required.
    async fn execute(self) -> Result<DataFusionQueryResult, DataFusionError> {
        let mut dataframe = self.session_ctx.table(self.table_ref).await?;

        let DataFusionQueryData {
            sort_by,
            partition_links,
            entry_links,
            prefilter,
            column_filters,
        } = &self.query_data;

        //
        // Partition links
        //

        // Important: the needs to happen first, in case we sort/filter/etc. based on that
        // particular column.
        if let Some(partition_links) = partition_links {
            //TODO(ab): we should get this from `re_uri::DatasetDataUri` instead of hardcoding
            let uri = format!(
                "{}/dataset/{}/data?partition_id=",
                partition_links.origin, partition_links.dataset_id
            );

            dataframe = dataframe.with_column(
                &partition_links.column_name,
                concat(vec![
                    lit(uri),
                    col(&partition_links.partition_id_column_name),
                ]),
            )?;
        }

        //
        // Entry links
        //

        if let Some(entry_links) = entry_links {
            let uri = format!("{}/entry/", entry_links.origin);

            let column = concat(vec![
                lit(uri),
                encode(
                    cast(col(&entry_links.entry_id_column_name), DataType::Binary),
                    lit("hex"),
                ),
            ]);
            dataframe = dataframe.with_column(&entry_links.column_name, column)?;
        }

        //
        // Prefilter
        //

        if let Some(prefilter) = prefilter {
            dataframe = dataframe.filter(prefilter.clone())?;
        }

        //
        // Filters
        //

        let filter_exprs = column_filters
            .iter()
            .filter_map(|filter| {
                filter
                    .as_filter_expression()
                    .inspect_err(|err| {
                        // TODO(ab): error handling will need to be improved once we introduce non-
                        // UI means of setting up filters.
                        re_log::warn_once!("invalid filter: {err}");
                    })
                    .ok()
            })
            .collect();
        let filter_expr =
            balanced_binary_exprs(filter_exprs, datafusion::logical_expr::Operator::And);
        if let Some(filter_expr) = filter_expr {
            dataframe = dataframe.filter(filter_expr)?;
        }

        //
        // Sort
        //

        if let Some(sort_by) = sort_by {
            dataframe = dataframe.sort(vec![
                col(&sort_by.column_physical_name).sort(sort_by.direction.is_ascending(), true),
            ])?;
        }

        //
        // Collect record batches
        //

        let original_schema = Arc::clone(dataframe.schema().inner());
        let record_batches = dataframe.collect().await?;

        //
        // Convert to `SorbetBatch`
        //

        let sorbet_batches = record_batches
            .iter()
            .map(|record_batch| {
                SorbetBatch::try_from_record_batch(record_batch, BatchType::Dataframe)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| DataFusionError::External(err.into()))?;

        //
        // Get (or create) `SorbetSchema`
        //

        let sorbet_schema = sorbet_batches
            .first()
            .map(|batch| Ok(batch.sorbet_schema().clone()))
            .unwrap_or_else(|| {
                re_sorbet::SorbetSchema::try_from_raw_arrow_schema(Arc::clone(&original_schema))
                    .map_err(|err| DataFusionError::External(err.into()))
            })?;

        Ok(DataFusionQueryResult {
            sorbet_batches,
            original_schema,
            sorbet_schema,
        })
    }
}

impl PartialEq for DataFusionQuery {
    fn eq(&self, other: &Self) -> bool {
        let Self {
            session_ctx,
            table_ref,
            query_data,
        } = self;

        Arc::ptr_eq(session_ctx, &other.session_ctx)
            && table_ref == &other.table_ref
            && query_data == &other.query_data
    }
}

type RequestedDataFusionQueryResult =
    RequestedObject<Result<DataFusionQueryResult, DataFusionError>>;

/// Helper struct to manage the datafusion async query and the resulting `SorbetBatch`.
#[derive(Clone)]
pub struct DataFusionAdapter {
    id: egui::Id,

    /// The current table blueprint
    blueprint: TableBlueprint,

    /// The query used to produce the dataframe.
    query: DataFusionQuery,

    // Used to have something to display while the new dataframe is being queried.
    pub last_query_results: Option<DataFusionQueryResult>,

    // TODO(ab, lucasmerlin): this `Mutex` is only needed because of the `Clone` bound in egui
    // so we should clean that up if the bound is lifted.
    pub requested_query_result: Arc<Mutex<RequestedDataFusionQueryResult>>,

    pub queried_at: Timestamp,
}

impl DataFusionAdapter {
    pub fn clear_state(egui_ctx: &egui::Context, id: egui::Id) {
        egui_ctx.data_mut(|data| {
            data.remove::<Self>(id);
        });
    }

    /// Retrieve the state from egui's memory or create a new one if it doesn't exist.
    pub fn get(
        runtime: &AsyncRuntimeHandle,
        ui: &egui::Ui,
        session_ctx: &Arc<SessionContext>,
        table_ref: TableReference,
        id: egui::Id,
        initial_blueprint: TableBlueprint,
    ) -> Self {
        let adapter = ui.data(|data| data.get_temp::<Self>(id));

        let adapter = adapter.unwrap_or_else(|| {
            let initial_query = DataFusionQueryData::from(&initial_blueprint);
            let query = DataFusionQuery::new(Arc::clone(session_ctx), table_ref, initial_query);

            let table_state = Self {
                id,
                blueprint: initial_blueprint,
                requested_query_result: Arc::new(Mutex::new(RequestedObject::new_with_repaint(
                    runtime,
                    ui.ctx().clone(),
                    query.clone().execute(),
                ))),
                query,
                last_query_results: None,
                queried_at: Timestamp::now(),
            };

            ui.data_mut(|data| {
                data.insert_temp(id, table_state.clone());
            });

            table_state
        });

        adapter.requested_query_result.lock().on_frame_start();

        adapter
    }

    pub fn blueprint(&self) -> &TableBlueprint {
        &self.blueprint
    }

    /// Update the query and save the state to egui's memory.
    ///
    /// If the query has changed (e.g. because the ui mutated it), it is executed to produce a new
    /// dataframe.
    pub fn update_query(
        mut self,
        runtime: &AsyncRuntimeHandle,
        ui: &egui::Ui,
        new_blueprint: TableBlueprint,
    ) {
        self.blueprint = new_blueprint;

        // retrigger a new datafusion query if required.
        let new_query_data = DataFusionQueryData::from(&self.blueprint);
        if self.query.query_data != new_query_data {
            self.query.query_data = new_query_data;

            let mut dataframe = self.requested_query_result.lock();

            if let Some(Ok(sorbet_batches)) = dataframe.try_as_ref() {
                self.last_query_results = Some(sorbet_batches.clone());
            }

            *dataframe = RequestedObject::new_with_repaint(
                runtime,
                ui.ctx().clone(),
                self.query.clone().execute(),
            );
        }

        ui.data_mut(|data| {
            data.insert_temp(self.id, self);
        });
    }
}

/// Creates a _balanced_ chain of binary expressions.
fn balanced_binary_exprs(
    mut exprs: Vec<datafusion::logical_expr::Expr>,
    op: datafusion::logical_expr::Operator,
) -> Option<datafusion::logical_expr::Expr> {
    while exprs.len() > 1 {
        let mut exprs_next = Vec::with_capacity(exprs.len() / 2 + 1);
        let mut exprs_prev = exprs.into_iter();

        while let Some(left) = exprs_prev.next() {
            if let Some(right) = exprs_prev.next() {
                exprs_next.push(binary_expr(left, op, right));
            } else {
                exprs_next.push(left);
            }
        }

        exprs = exprs_next;
    }

    exprs.into_iter().next()
}
