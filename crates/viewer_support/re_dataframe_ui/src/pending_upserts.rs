use std::sync::Arc;

use datafusion::prelude::SessionContext;
use datafusion::sql::TableReference as DataFusionTableReference;
use itertools::Itertools as _;
use re_async::AsyncRuntimeHandle;
use re_protos::cloud::v1alpha1::ext;
use re_redap_client::ConnectionRegistryHandle;
use re_sdk_types::blueprint::components::ColumnName;

use crate::DataFusionTableWidget;

/// An edited cell that is not yet written to the server.
#[derive(Clone)]
pub struct PendingUpsert {
    pub entry_uri: re_uri::EntryUri,
    pub session_ctx: Arc<SessionContext>,
    pub table_ref: DataFusionTableReference,
    pub row: u64,
    pub physical_column: ColumnName,

    /// The table index and the edited column of the row.
    pub batch: arrow::array::RecordBatch,
}

fn pending_upserts_id() -> egui::Id {
    egui::Id::unique("re_dataframe_ui::pending_upserts")
}

/// Queues an edited cell, replacing an earlier edit of the same cell.
pub fn queue_upsert(egui_ctx: &egui::Context, upsert: PendingUpsert) {
    egui_ctx.data_mut(|data| {
        let pending = data.get_temp_mut_or_default::<Vec<PendingUpsert>>(pending_upserts_id());
        pending.retain(|pending| {
            pending.entry_uri != upsert.entry_uri
                || pending.row != upsert.row
                || pending.physical_column != upsert.physical_column
        });
        pending.push(upsert);
    });
}

/// Writes the edited table cells to the server once the user is no longer interacting
/// or typing.
///
/// Call this once per frame.
pub fn write_pending_upserts(
    egui_ctx: &egui::Context,
    connection_registry: &ConnectionRegistryHandle,
    runtime: &AsyncRuntimeHandle,
) {
    if re_viewer_context::is_interacting(egui_ctx) || egui_ctx.egui_wants_keyboard_input() {
        return;
    }

    let Some(mut pending) =
        egui_ctx.data_mut(|data| data.remove_temp::<Vec<PendingUpsert>>(pending_upserts_id()))
    else {
        return;
    };

    pending.sort_by(|a, b| a.entry_uri.cmp(&b.entry_uri));
    for (entry_uri, upserts) in &pending
        .into_iter()
        .chunk_by(|upsert| upsert.entry_uri.clone())
    {
        let upserts = upserts.collect();
        let connection = connection_registry.connection_handle(entry_uri.origin.clone());
        runtime.spawn_future(write_upserts(connection, entry_uri.entry_id, upserts));
    }
}

/// Writes the edited rows of one table to the server, in order, then refreshes its
/// streaming cache so the next query gets the written values.
async fn write_upserts(
    connection: re_redap_client::ConnectionHandle,
    entry_id: re_log_types::EntryId,
    upserts: Vec<PendingUpsert>,
) {
    let tables: Vec<_> = upserts
        .iter()
        .map(|upsert| (Arc::clone(&upsert.session_ctx), upsert.table_ref.clone()))
        .unique_by(|(session_ctx, table_ref)| (Arc::as_ptr(session_ctx), table_ref.clone()))
        .collect();

    let result = async {
        let mut client = connection.client().await?;
        for upsert in upserts {
            client
                .write_table(
                    futures::stream::once(async { upsert.batch }),
                    entry_id,
                    // `Update`: match existing rows on the table index and update only the
                    // edited column. Unmatched source rows are dropped, which is correct here
                    // since we only ever edit values on rows that already exist.
                    ext::TableInsertMode::Update,
                )
                .await?;
        }
        Ok::<_, re_redap_client::ApiError>(())
    }
    .await;

    if let Err(err) = result {
        re_log::warn_once!("Failed to upsert edited values: {err}");
    } else {
        re_log::debug!("Successfully upserted edited values");
    }

    for (session_ctx, table_ref) in tables {
        DataFusionTableWidget::invalidate_streaming_cache(&session_ctx, &table_ref).await;
    }
}
