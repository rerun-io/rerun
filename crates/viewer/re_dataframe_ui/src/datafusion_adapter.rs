use std::sync::Arc;

use datafusion::common::{DataFusionError, TableReference};
use datafusion::functions::expr_fn::concat;
use datafusion::logical_expr::{col, lit};
use datafusion::prelude::SessionContext;
use parking_lot::Mutex;
use re_log_types::EntryId;
use re_sorbet::{BatchType, SorbetBatch};
use re_viewer_context::AsyncRuntimeHandle;

use crate::RequestedObject;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    pub fn iter() -> impl Iterator<Item = Self> {
        [Self::Ascending, Self::Descending].into_iter()
    }

    pub fn is_ascending(&self) -> bool {
        matches!(self, Self::Ascending)
    }

    pub fn icon(&self) -> &'static re_ui::Icon {
        match self {
            Self::Ascending => &re_ui::icons::ARROW_DOWN,
            Self::Descending => &re_ui::icons::ARROW_UP,
        }
    }

    pub fn menu_button(&self, ui: &mut egui::Ui) -> egui::Response {
        ui.add(egui::Button::image_and_text(
            self.icon()
                .as_image()
                .fit_to_exact_size(re_ui::DesignTokens::small_icon_size()),
            match self {
                Self::Ascending => "Ascending",
                Self::Descending => "Descending",
            },
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortBy {
    pub column: String,
    pub direction: SortDirection,
}

/// Information required to generate a partition link column.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PartitionLinksSpec {
    /// Name of the column to generate.
    pub column_name: String,

    /// Name of the existing column containing the partition id.
    pub partition_id_column_name: String,

    /// Origin to use for the links.
    pub origin: re_uri::Origin,

    /// The id of the dataset to use for the links.
    pub dataset_id: EntryId,
}

/// The "blueprint" for a table, a.k.a the specification of how it should look.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TableBlueprint {
    pub sort_by: Option<SortBy>,
    pub partition_links: Option<PartitionLinksSpec>,
}

/// A table blueprint along with the context required to execute the corresponding datafusion query.
#[derive(Clone)]
pub struct DataFusionQuery {
    session_ctx: Arc<SessionContext>,
    table_ref: TableReference,

    blueprint: TableBlueprint,
}

impl DataFusionQuery {
    pub fn new(
        session_ctx: Arc<SessionContext>,
        table_ref: TableReference,
        blueprint: TableBlueprint,
    ) -> Self {
        Self {
            session_ctx,
            table_ref,
            blueprint,
        }
    }

    /// Execute the query and produce a vector of [`SorbetBatch`]s.
    ///
    /// Note: the future returned by this function must be `'static`, so it takes `self`. Use
    /// `clone()` as required.
    async fn execute(self) -> Result<Vec<SorbetBatch>, DataFusionError> {
        let mut dataframe = self.session_ctx.table(self.table_ref).await?;

        let TableBlueprint {
            sort_by,
            partition_links,
        } = &self.blueprint;

        if let Some(sort_by) = sort_by {
            dataframe = dataframe.sort(vec![
                col(&sort_by.column).sort(sort_by.direction.is_ascending(), true)
            ])?;
        }

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
            blueprint,
        } = self;

        Arc::ptr_eq(session_ctx, &other.session_ctx)
            && table_ref == &other.table_ref
            && blueprint == &other.blueprint
    }
}

type RequestedDataframe = RequestedObject<Result<Vec<SorbetBatch>, DataFusionError>>;

#[derive(Clone)]
pub struct DataFusionAdapter {
    id: egui::Id,

    /// The query used to produce the dataframe.
    query: DataFusionQuery,

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
        table_ref: TableReference,
        id: egui::Id,
        initial_blueprint: TableBlueprint,
        force_refresh: bool,
    ) -> Self {
        let adapter = if force_refresh {
            None
        } else {
            ui.data(|data| data.get_temp::<Self>(id))
        };

        let adapter = adapter.unwrap_or_else(|| {
            let query = DataFusionQuery::new(Arc::clone(session_ctx), table_ref, initial_blueprint);

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
                data.insert_temp(id, table_state.clone());
            });

            table_state
        });

        adapter.dataframe.lock().on_frame_start();

        adapter
    }

    pub fn blueprint(&self) -> &TableBlueprint {
        &self.query.blueprint
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
        new_blueprint: TableBlueprint,
    ) {
        if self.query.blueprint != new_blueprint {
            self.query.blueprint = new_blueprint;

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
