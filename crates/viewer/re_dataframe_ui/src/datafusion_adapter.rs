use arrow::datatypes::DataType;
use datafusion::common::{DataFusionError, TableReference};
use datafusion::functions::expr_fn::concat;
use datafusion::logical_expr::{col as datafusion_col, lit};
use datafusion::prelude::{SessionContext, cast, encode};
use parking_lot::Mutex;
use re_sorbet::{BatchType, SorbetBatch};
use re_viewer_context::AsyncRuntimeHandle;
use std::sync::Arc;

use crate::RequestedObject;
use crate::table_blueprint::{EntryLinksSpec, PartitionLinksSpec, SortBy, TableBlueprint};

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
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct DataFusionQueryData {
    pub sort_by: Option<SortBy>,
    pub partition_links: Option<PartitionLinksSpec>,
    pub entry_links: Option<EntryLinksSpec>,
}

impl From<&TableBlueprint> for DataFusionQueryData {
    fn from(value: &TableBlueprint) -> Self {
        let TableBlueprint {
            sort_by,
            partition_links,
            entry_links,
        } = value;

        Self {
            sort_by: sort_by.clone(),
            partition_links: partition_links.clone(),
            entry_links: entry_links.clone(),
        }
    }
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

    /// Execute the query and produce a vector of [`SorbetBatch`]s.
    ///
    /// Note: the future returned by this function must be `'static`, so it takes `self`. Use
    /// `clone()` as required.
    async fn execute(self) -> Result<Vec<SorbetBatch>, DataFusionError> {
        let mut dataframe = self.session_ctx.table(self.table_ref).await?;

        let DataFusionQueryData {
            sort_by,
            partition_links,
            entry_links,
        } = &self.query_data;

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

        if let Some(sort_by) = sort_by {
            dataframe = dataframe.sort(vec![
                col(&sort_by.column).sort(sort_by.direction.is_ascending(), true),
            ])?;
        }

        // collect
        let record_batches = dataframe.collect().await?;

        // convert to SorbetBatch
        let sorbet_batches = record_batches
            .iter()
            .map(|record_batch| {
                SorbetBatch::try_from_record_batch(record_batch, BatchType::Dataframe)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| DataFusionError::External(err.into()))?;

        Ok(sorbet_batches)
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

type RequestedSorbetBatches = RequestedObject<Result<Vec<SorbetBatch>, DataFusionError>>;

/// Helper struct to manage the datafusion async query and the resulting `SorbetBatch`.
#[derive(Clone)]
pub struct DataFusionAdapter {
    id: egui::Id,

    /// The current table blueprint
    blueprint: TableBlueprint,

    /// The query used to produce the dataframe.
    query: DataFusionQuery,

    // Used to have something to display while the new dataframe is being queried.
    pub last_sorbet_batches: Option<Vec<SorbetBatch>>,

    // TODO(ab, lucasmerlin): this `Mutex` is only needed because of the `Clone` bound in egui
    // so we should clean that up if the bound is lifted.
    pub requested_sorbet_batches: Arc<Mutex<RequestedSorbetBatches>>,
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
                requested_sorbet_batches: Arc::new(Mutex::new(RequestedObject::new_with_repaint(
                    runtime,
                    ui.ctx().clone(),
                    query.clone().execute(),
                ))),
                query,
                last_sorbet_batches: None,
            };

            ui.data_mut(|data| {
                data.insert_temp(id, table_state.clone());
            });

            table_state
        });

        adapter.requested_sorbet_batches.lock().on_frame_start();

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

            let mut dataframe = self.requested_sorbet_batches.lock();

            if let Some(Ok(sorbet_batches)) = dataframe.try_as_ref() {
                self.last_sorbet_batches = Some(sorbet_batches.clone());
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
