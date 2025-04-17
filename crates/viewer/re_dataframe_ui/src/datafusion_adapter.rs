use std::sync::Arc;

use datafusion::common::DataFusionError;
use datafusion::logical_expr::col;
use datafusion::prelude::SessionContext;
use parking_lot::Mutex;

use re_sorbet::{BatchType, SorbetBatch};
use re_viewer_context::AsyncRuntimeHandle;

use crate::RequestedObject;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    pub fn is_ascending(&self) -> bool {
        matches!(self, Self::Ascending)
    }

    pub fn icon(&self) -> &'static re_ui::Icon {
        match self {
            Self::Ascending => &re_ui::icons::ARROW_DOWN,
            Self::Descending => &re_ui::icons::ARROW_UP,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortBy {
    pub column: String,
    pub direction: SortDirection,
}

/// The full description of a query against a datafusion context, to be executed asynchronously.
//TODO(ab): when we have table blueprint, that query should be derived from it.
#[derive(Clone)]
pub struct DataFusionQuery {
    session_ctx: Arc<SessionContext>,
    table_name: String,

    pub sort_by: Option<SortBy>,
}

impl DataFusionQuery {
    pub fn new(session_ctx: Arc<SessionContext>, table_name: String) -> Self {
        Self {
            session_ctx,
            table_name,
            sort_by: None,
        }
    }

    /// Execute the query and produce a vector of [`SorbetBatch`]s.
    ///
    /// Note: the future returned by this function must by `'static`, so it takes `self`. Use
    /// `clone()` as required.
    async fn execute(self) -> Result<Vec<SorbetBatch>, DataFusionError> {
        let mut dataframe = self.session_ctx.table(&self.table_name).await?;

        if let Some(sort_by) = &self.sort_by {
            dataframe = dataframe.sort(vec![
                col(&sort_by.column).sort(sort_by.direction.is_ascending(), true)
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
            table_name,
            sort_by,
        } = self;

        Arc::ptr_eq(session_ctx, &other.session_ctx)
            && table_name == &other.table_name
            && sort_by == &other.sort_by
    }
}

type RequestedDataframe = RequestedObject<Result<Vec<SorbetBatch>, DataFusionError>>;

#[derive(Clone)]
pub struct DataFusionAdapter {
    id: egui::Id,

    /// The query used to produce the dataframe.
    pub query: DataFusionQuery,

    // Used to have something to display while the new dataframe is being queried.
    pub last_dataframe: Option<Vec<SorbetBatch>>,

    pub dataframe: Arc<Mutex<RequestedDataframe>>,
}

impl DataFusionAdapter {
    /// Retrieve the state from egui's memory or create a new one if it doesn't exist.
    pub fn get(
        runtime: &AsyncRuntimeHandle,
        ui: &egui::Ui,
        session_ctx: &Arc<SessionContext>,
        table_name: &str,
        table_id: egui::Id,
    ) -> Self {
        // The cache must be invalidated as soon as the input table name or session context change,
        // so we add that to the id.
        let id = table_id.with((table_name, session_ctx.session_id()));

        let adapter = ui
            .data(|data| data.get_temp::<Self>(id))
            .unwrap_or_else(|| {
                let query = DataFusionQuery::new(Arc::clone(session_ctx), table_name.to_owned());

                let table_state = Self {
                    id,
                    dataframe: Arc::new(Mutex::new(RequestedObject::new_with_repaint(
                        runtime,
                        ui.ctx().clone(),
                        query.clone().execute(),
                    ))),
                    query,
                    last_dataframe: None,
                };

                ui.data_mut(|data| {
                    data.insert_temp(table_id, table_state.clone());
                });

                table_state
            });

        adapter.dataframe.lock().on_frame_start();

        adapter
    }

    /// Clear from egui's memory (force refresh on the next frame).
    pub fn clear(&self, ui: &egui::Ui) {
        ui.data_mut(|data| {
            data.remove::<Self>(self.id);
        });
    }

    /// Update the query and save the state to egui's memory.
    ///
    /// If the query has changed (e.g. because the ui mutated it), it is executed to produce a new
    /// dataframe.
    pub fn update_query(
        mut self,
        runtime: &AsyncRuntimeHandle,
        ui: &egui::Ui,
        new_query: DataFusionQuery,
    ) {
        if self.query != new_query {
            self.query = new_query;

            let mut dataframe = self.dataframe.lock();

            if let Some(Ok(sorbet_batches)) = dataframe.try_as_ref() {
                self.last_dataframe = Some(sorbet_batches.clone());
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
