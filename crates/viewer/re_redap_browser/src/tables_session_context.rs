//! Helper to maintain a [`SessionContext`] with the tables of a remote server.

use std::pin::Pin;
use std::sync::Arc;

use datafusion::catalog::TableProvider;
use datafusion::common::DataFusionError;
use datafusion::prelude::SessionContext;

use re_dataframe_ui::RequestedObject;
use re_datafusion::{PartitionTableProvider, TableEntryTableProvider};
use re_grpc_client::{ConnectionError, ConnectionRegistryHandle};
use re_log_types::EntryId;
use re_protos::TypeConversionError;
use re_protos::catalog::v1alpha1::ext::EntryDetails;
use re_protos::catalog::v1alpha1::{EntryFilter, EntryKind, FindEntriesRequest};
use re_protos::external::prost;
use re_viewer_context::AsyncRuntimeHandle;

#[derive(Debug, thiserror::Error)]
#[expect(clippy::enum_variant_names)]
pub enum SessionContextError {
    #[error(transparent)]
    TonicError(#[from] tonic::Status),

    #[error(transparent)]
    ConnectionError(#[from] ConnectionError),

    #[error(transparent)]
    DataFusionError(#[from] DataFusionError),

    #[error(transparent)]
    TypeConversionError(#[from] TypeConversionError),
}

struct Table {
    #[expect(dead_code)]
    entry_id: EntryId,
    #[expect(dead_code)]
    name: String,
}

/// Wrapper over a [`SessionContext`] that contains all the tables registered in the remote server,
/// including the table entries and the partition tables of the dataset entries.
//TODO(ab): add support for local caching of table data
pub struct TablesSessionContext {
    pub ctx: Arc<SessionContext>,
    #[expect(dead_code)]
    origin: re_uri::Origin,

    registered_tables: RequestedObject<Result<Vec<Table>, SessionContextError>>,
}

impl TablesSessionContext {
    pub fn new(
        connection_registry: ConnectionRegistryHandle,
        runtime: &AsyncRuntimeHandle,
        egui_ctx: &egui::Context,
        origin: re_uri::Origin,
    ) -> Self {
        let ctx = Arc::new(SessionContext::new());

        let registered_tables = RequestedObject::new_with_repaint(
            runtime,
            egui_ctx.clone(),
            register_all_table_entries(ctx.clone(), connection_registry, origin.clone()),
        );

        Self {
            ctx,
            origin,
            registered_tables,
        }
    }

    pub fn on_frame_start(&mut self) {
        self.registered_tables.on_frame_start();
    }
}

async fn register_all_table_entries(
    ctx: Arc<SessionContext>,
    connection_registry: ConnectionRegistryHandle,
    origin: re_uri::Origin,
) -> Result<Vec<Table>, SessionContextError> {
    let mut client = connection_registry.client(origin.clone()).await?;

    let entries = client
        .inner()
        .find_entries(FindEntriesRequest {
            filter: Some(EntryFilter {
                id: None,
                name: None,
                entry_kind: None,
            }),
        })
        .await?
        .into_inner()
        .entries
        .into_iter()
        .map(TryInto::try_into)
        .collect::<Result<Vec<EntryDetails>, _>>()?;

    let mut registered_tables = vec![];

    type ResultFuture = dyn Send
        + Future<
            Output = (
                Result<Arc<dyn TableProvider>, DataFusionError>,
                EntryDetails,
            ),
        >;
    let mut futures: Vec<Pin<Box<ResultFuture>>> = vec![];
    for entry in entries {
        let client = client.clone();
        #[expect(clippy::match_same_arms)]
        match entry.kind {
            // TODO(rerun-io/dataplatform#857): these are often empty datasets, and thus fail. For
            // some reason, this failure is silent but blocks other tables from being registered.
            // Since we don't need these tables yet, we just skip them for now.
            EntryKind::BlueprintDataset => {}

            EntryKind::Dataset => futures.push(Box::pin(async move {
                (
                    PartitionTableProvider::new(client, entry.id)
                        .into_provider()
                        .await,
                    entry,
                )
            })),
            EntryKind::Table => futures.push(Box::pin(async move {
                (
                    TableEntryTableProvider::new(client, entry.id)
                        .into_provider()
                        .await,
                    entry,
                )
            })),

            // TODO(ab): these do not exist yet
            EntryKind::DatasetView | EntryKind::TableView => {}

            EntryKind::Unspecified => {
                return Err(
                    TypeConversionError::from(prost::UnknownEnumValue(entry.kind as i32)).into(),
                );
            }
        }
    }

    for (result, entry) in futures::future::join_all(futures).await {
        let table_provider = result?;
        ctx.register_table(&entry.name, table_provider)?;

        registered_tables.push(Table {
            entry_id: entry.id,
            name: entry.name,
        });
    }

    Ok(registered_tables)
}
