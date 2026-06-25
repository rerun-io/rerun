use std::future;
use std::sync::Arc;
use std::task::Poll;

use ahash::HashMap;
use datafusion::catalog::TableProvider;
use datafusion::common::TableReference;
use datafusion::prelude::SessionContext;
use futures::stream::FuturesUnordered;
use futures::{FutureExt as _, StreamExt as _, TryFutureExt as _};
use re_dataframe_ui::{RequestedObject, StreamingCacheTableProvider};
use re_datafusion::{SegmentTableProvider, TableEntryTableProvider, TableKind, TableQueryCaller};
use re_log_types::{EntryId, EntryName, TableId};
use re_protos::TypeConversionError;
use re_protos::cloud::v1alpha1::ext::{DatasetEntry, EntryDetails, ProviderDetails, TableEntry};
use re_protos::cloud::v1alpha1::{EntryFilter, EntryKind};
use re_protos::external::prost;
use re_redap_client::{
    ApiError, ConnectionAnalyticsExporter, ConnectionClient, ConnectionRegistryHandle,
};
use re_ui::{Icon, icons};
use re_viewer_context::{
    AsyncRuntimeHandle, CommandSender, SystemCommand, SystemCommandSender as _,
};

pub type EntryResult<T = ()> = Result<T, ApiError>;

pub struct Dataset {
    pub dataset_entry: DatasetEntry,
    pub origin: re_uri::Origin,
}

impl std::fmt::Debug for Dataset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Dataset({:?} @ {})", self.name(), self.origin)
    }
}

impl Dataset {
    pub fn id(&self) -> EntryId {
        self.dataset_entry.details.id
    }

    pub fn name(&self) -> &EntryName {
        &self.dataset_entry.details.name
    }
}

pub struct Table {
    pub table_entry: TableEntry,

    pub origin: re_uri::Origin,
}

impl std::fmt::Debug for Table {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Table({:?} @ {})", self.name(), self.origin)
    }
}

impl Table {
    pub fn id(&self) -> EntryId {
        self.table_entry.details.id
    }

    pub fn name(&self) -> &EntryName {
        &self.table_entry.details.name
    }
}

#[derive(Debug)]
pub enum EntryInner {
    Dataset(Dataset),
    Table(Table),
}

pub struct Entry {
    details: EntryDetails,
    inner: EntryResult<EntryInner>,
}

impl std::fmt::Debug for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Entry({:?})", self.name())
    }
}

impl Entry {
    pub fn details(&self) -> &EntryDetails {
        &self.details
    }

    pub fn id(&self) -> EntryId {
        self.details().id
    }

    pub fn name(&self) -> &EntryName {
        &self.details().name
    }

    pub fn icon(&self) -> Icon {
        match &self.details.kind {
            EntryKind::Dataset | EntryKind::DatasetView | EntryKind::BlueprintDataset => {
                icons::DATASET
            }
            EntryKind::Table | EntryKind::TableView => icons::TABLE,
            EntryKind::Unspecified => icons::VIEW_UNKNOWN,
        }
    }

    pub fn inner(&self) -> &EntryResult<EntryInner> {
        &self.inner
    }
}

/// All the entries of a server.
// TODO(ab): we currently load the ENTIRE list of datasets. We will need to be more granular
// about this in the future.
pub struct Entries {
    entries: RequestedObject<EntryResult<HashMap<EntryId, Entry>>>,
}

impl Entries {
    pub(crate) fn new(
        connection_registry: ConnectionRegistryHandle,
        runtime: &AsyncRuntimeHandle,
        egui_ctx: &egui::Context,
        origin: re_uri::Origin,
        session_context: Arc<SessionContext>,
        command_sender: CommandSender,
    ) -> Self {
        let entries_fut = fetch_entries_and_register_tables(
            connection_registry,
            origin,
            session_context,
            runtime.clone(),
            command_sender,
        );

        Self {
            entries: RequestedObject::new_with_repaint(runtime, egui_ctx.clone(), entries_fut),
        }
    }

    pub(crate) fn refresh(
        self,
        connection_registry: ConnectionRegistryHandle,
        runtime: &AsyncRuntimeHandle,
        egui_ctx: &egui::Context,
        origin: re_uri::Origin,
        session_context: Arc<SessionContext>,
        command_sender: CommandSender,
    ) -> Self {
        let entries_fut = fetch_entries_and_register_tables(
            connection_registry,
            origin,
            session_context,
            runtime.clone(),
            command_sender,
        );

        Self {
            entries: self.entries.refresh_with_previous_and_repaint(
                runtime,
                egui_ctx.clone(),
                entries_fut,
            ),
        }
    }

    pub(crate) fn on_frame_start(&mut self) {
        self.entries.on_frame_start();
    }

    pub fn find_entry(&self, entry_id: EntryId) -> Option<&Entry> {
        self.entries.try_as_ref()?.as_ref().ok()?.get(&entry_id)
    }

    pub fn state(&self) -> Poll<Result<&HashMap<EntryId, Entry>, &ApiError>> {
        self.entries
            .try_as_ref()
            .map_or(Poll::Pending, |r| match r {
                Ok(entries) => Poll::Ready(Ok(entries)),
                Err(err) => Poll::Ready(Err(err)),
            })
    }
}

async fn fetch_entries_and_register_tables(
    connection_registry: ConnectionRegistryHandle,
    origin: re_uri::Origin,
    session_ctx: Arc<SessionContext>,
    runtime: AsyncRuntimeHandle,
    command_sender: CommandSender,
) -> EntryResult<HashMap<EntryId, Entry>> {
    let connection = connection_registry.connection(origin.clone()).await?;
    let mut client = connection.client;
    let analytics = connection.analytics;

    let entries = client
        .find_entries(EntryFilter {
            id: None,
            name: None,
            entry_kind: None,
        })
        .await?;

    let origin_ref = &origin;
    let runtime_ref = &runtime;
    let command_sender_ref = &command_sender;
    let futures_iter = entries.into_iter().filter_map(move |e| {
        fetch_entry_details(
            client.clone(),
            origin_ref,
            analytics.clone(),
            e,
            runtime_ref,
            command_sender_ref,
        )
    });

    let mut entries = HashMap::default();

    let mut futures_unordered: FuturesUnordered<_> = futures_iter.collect();
    while let Some((details, result)) = futures_unordered.next().await {
        let id = details.id;
        let inner_result = result.map(|(inner, provider)| {
            // Create cached provider that reads from the raw table
            let cached_provider = StreamingCacheTableProvider::new(provider, runtime.clone());

            // Register cached provider with original name (in default schema).
            // Use `TableReference::bare` to prevent DataFusion from splitting
            // names containing dots into schema.table pairs.
            session_ctx
                .register_table(
                    TableReference::bare(details.name.as_str()),
                    Arc::new(cached_provider),
                )
                .ok();

            inner
        });

        let is_system_table = match &inner_result {
            Ok(EntryInner::Table(table)) => matches!(
                table.table_entry.provider_details,
                ProviderDetails::SystemTable(_)
            ),
            Err(_) | Ok(EntryInner::Dataset(_)) => false,
        };
        if !is_system_table {
            let entry = Entry {
                details,
                inner: inner_result,
            };
            entries.insert(id, entry);
        }
    }

    Ok(entries)
}

/// Basically a [`Entry`] + `Arc<dyn TableProvider>`.
type FetchEntryDetailsOutput = (
    EntryDetails,
    EntryResult<(EntryInner, Arc<dyn TableProvider>)>,
);

/// Returns None if the entry should not be presented in the UI.
fn fetch_entry_details(
    client: ConnectionClient,
    origin: &re_uri::Origin,
    analytics: Option<ConnectionAnalyticsExporter>,
    entry: EntryDetails,
    runtime: &AsyncRuntimeHandle,
    command_sender: &CommandSender,
) -> Option<impl Future<Output = FetchEntryDetailsOutput>> {
    // We could also box the future but then we'd need to use `.boxed()` natively and
    // `.boxed_local()` on wasm. Either passes the `Send` type info transparently.
    use itertools::Either::{Left, Right};
    #[expect(clippy::match_same_arms)]
    match &entry.kind {
        // These are often empty datasets, and thus fail.
        // Since we don't need these tables yet, we just skip them for now.
        EntryKind::BlueprintDataset => None,
        EntryKind::Dataset => Some(Left(Left(
            fetch_dataset_details(client, entry.id, origin, runtime, command_sender)
                .map_ok(|(dataset, table_provider)| (EntryInner::Dataset(dataset), table_provider))
                .map(move |res| (entry, res)),
        ))),
        EntryKind::Table => Some(Left(Right(
            fetch_table_details(client, entry.id, origin, analytics, runtime, command_sender)
                .map_ok(|(table, table_provider)| (EntryInner::Table(table), table_provider))
                .map(move |res| (entry, res)),
        ))),

        // TODO(ab): these do not exist yet
        EntryKind::DatasetView | EntryKind::TableView => None,

        EntryKind::Unspecified => {
            let kind = entry.kind;
            let err = TypeConversionError::from(prost::UnknownEnumValue(kind as i32));
            Some(Right(future::ready((
                entry,
                Err(ApiError::deserialization_with_source(
                    None,
                    err,
                    "unknown entry kind",
                )),
            ))))
        }
    }
}

async fn fetch_dataset_details(
    mut client: ConnectionClient,
    id: EntryId,
    origin: &re_uri::Origin,
    runtime: &AsyncRuntimeHandle,
    command_sender: &CommandSender,
) -> EntryResult<(Dataset, Arc<dyn TableProvider>)> {
    let dataset_entry = client.read_dataset_entry(id).await?;

    start_streaming_segment_table_blueprint(
        client.clone(),
        &dataset_entry,
        origin,
        runtime,
        command_sender,
    );

    let result = Dataset {
        dataset_entry,
        origin: origin.clone(),
    };

    let table_provider = SegmentTableProvider::new(client, id)
        .into_provider()
        .await
        .map_err(|err| {
            ApiError::internal_with_source(None, err, "failed creating segment table provider")
        })?;

    Ok((result, table_provider))
}

/// Stream the dataset's default segment-table blueprint (if any) and associate it with the dataset's segment table view.
fn start_streaming_segment_table_blueprint(
    client: ConnectionClient,
    dataset_entry: &DatasetEntry,
    origin: &re_uri::Origin,
    runtime: &AsyncRuntimeHandle,
    command_sender: &CommandSender,
) {
    let dataset_id = dataset_entry.details.id;
    let Some((blueprint_dataset, blueprint_segment)) = dataset_entry
        .dataset_details
        .default_segment_table_blueprint()
    else {
        return;
    };

    let blueprint_store_id =
        re_log_types::StoreId::random(re_log_types::StoreKind::Blueprint, dataset_id.to_string());

    let (tx, rx) = re_redap_client::table_blueprint_log_channel(
        origin.clone(),
        blueprint_dataset,
        &blueprint_segment,
        TableId::new(dataset_id.to_string()),
        blueprint_store_id.clone(),
    );

    command_sender.send_system(SystemCommand::AddReceiver(rx));

    runtime.spawn_future(async move {
        if let Err(err) = re_redap_client::stream_table_blueprint_segment_from_server(
            client,
            tx,
            blueprint_store_id,
            blueprint_dataset,
            blueprint_segment,
        )
        .await
        {
            re_log::warn!("Failed to stream segment table blueprint: {err}");
        }
    });
}

fn start_registered_table_blueprint_stream(
    client: ConnectionClient,
    table_entry: &TableEntry,
    origin: &re_uri::Origin,
    runtime: &AsyncRuntimeHandle,
    command_sender: &CommandSender,
) {
    let table_id = table_entry.details.id;
    let Some((blueprint_dataset, blueprint_segment)) =
        table_entry.table_details.default_blueprint()
    else {
        return;
    };

    let blueprint_store_id = re_log_types::StoreId::random(
        re_log_types::StoreKind::Blueprint,
        table_entry.details.id.to_string(),
    );

    let (tx, rx) = re_redap_client::table_blueprint_log_channel(
        origin.clone(),
        blueprint_dataset,
        &blueprint_segment,
        TableId::new(table_id.to_string()),
        blueprint_store_id.clone(),
    );

    command_sender.send_system(SystemCommand::AddReceiver(rx));

    runtime.spawn_future(async move {
        if let Err(err) = re_redap_client::stream_table_blueprint_segment_from_server(
            client,
            tx,
            blueprint_store_id,
            blueprint_dataset,
            blueprint_segment,
        )
        .await
        {
            re_log::warn!("Failed to stream table blueprint: {err}");
        }
    });
}

async fn fetch_table_details(
    mut client: ConnectionClient,
    id: EntryId,
    origin: &re_uri::Origin,
    analytics: Option<ConnectionAnalyticsExporter>,
    runtime: &AsyncRuntimeHandle,
    command_sender: &CommandSender,
) -> EntryResult<(Table, Arc<dyn TableProvider>)> {
    let result = client.read_table_entry(id).await.map(|table_entry| Table {
        table_entry,
        origin: origin.clone(),
    })?;

    start_registered_table_blueprint_stream(
        client.clone(),
        &result.table_entry,
        origin,
        runtime,
        command_sender,
    );

    #[cfg(target_arch = "wasm32")]
    let runtime = None;
    #[cfg(not(target_arch = "wasm32"))]
    let runtime = Some(runtime.inner().clone());

    let table_kind = TableKind::from(&result.table_entry.provider_details);
    let caller = match table_kind {
        TableKind::SystemEntries | TableKind::SystemNamespaces => TableQueryCaller::EntriesTable,
        TableKind::Lance | TableKind::Unknown => TableQueryCaller::BrowserDetailView,
    };

    let mut table_provider = TableEntryTableProvider::new(client, id, runtime)
        .with_caller(caller)
        .with_table_kind(table_kind);
    if let Some(exporter) = analytics {
        table_provider = table_provider.with_analytics(exporter);
    }
    let table_provider = table_provider.into_provider().await.map_err(|err| {
        ApiError::internal_with_source(None, err, "failed creating table-entry table provider")
    })?;

    Ok((result, table_provider))
}
