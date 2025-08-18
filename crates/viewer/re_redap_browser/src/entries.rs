use std::future;
use std::sync::Arc;
use std::task::Poll;

use ahash::HashMap;
use datafusion::catalog::TableProvider;
use datafusion::common::DataFusionError;
use datafusion::prelude::SessionContext;
use futures::stream::FuturesUnordered;
use futures::{FutureExt as _, StreamExt as _, TryFutureExt as _};

use re_dataframe_ui::RequestedObject;
use re_datafusion::{PartitionTableProvider, TableEntryTableProvider};
use re_grpc_client::{ConnectionClient, ConnectionError, ConnectionRegistryHandle, StreamError};
use re_log_encoding::codec::CodecError;
use re_log_types::EntryId;
use re_protos::TypeConversionError;
use re_protos::catalog::v1alpha1::ext::{EntryDetails, TableEntry};
use re_protos::catalog::v1alpha1::{EntryFilter, EntryKind, FindEntriesRequest, ext::DatasetEntry};
use re_protos::external::prost;
use re_protos::external::prost::Name as _;
use re_sorbet::SorbetError;
use re_ui::{Icon, icons};
use re_viewer_context::AsyncRuntimeHandle;

pub type EntryResult<T> = Result<T, EntryError>;

#[expect(clippy::enum_variant_names)]
#[derive(Debug, thiserror::Error)]
pub enum EntryError {
    /// You usually want to use [`EntryError::tonic_status`] instead
    /// (there are multiple variants holding [`tonic::Status`]).
    #[error(transparent)]
    TonicError(Box<tonic::Status>),

    #[error(transparent)]
    ConnectionError(#[from] ConnectionError),

    #[error(transparent)]
    StreamError(#[from] StreamError),

    #[error(transparent)]
    TypeConversionError(#[from] TypeConversionError),

    #[error(transparent)]
    CodecError(#[from] CodecError),

    #[error(transparent)]
    SorbetError(#[from] SorbetError),

    #[error(transparent)]
    DataFusionError(Box<DataFusionError>),
}

#[test]
fn test_error_size() {
    assert!(
        std::mem::size_of::<EntryError>() <= 80,
        "Size of error is {} bytes. Let's try to keep errors small.",
        std::mem::size_of::<EntryError>()
    );
}

impl From<tonic::Status> for EntryError {
    fn from(status: tonic::Status) -> Self {
        Self::TonicError(Box::new(status))
    }
}

impl From<DataFusionError> for EntryError {
    fn from(err: DataFusionError) -> Self {
        Self::DataFusionError(Box::new(err))
    }
}

impl EntryError {
    fn tonic_status(&self) -> Option<&tonic::Status> {
        // Be explicit here so we don't miss any future variants that might have a `tonic::Status`.
        match self {
            Self::TonicError(status) => Some(status.as_ref()),
            Self::StreamError(StreamError::TonicStatus(status)) => Some(status.as_ref()),
            #[cfg(not(target_arch = "wasm32"))]
            Self::StreamError(StreamError::Transport(_)) => None,
            Self::StreamError(
                StreamError::ConnectionError(_)
                | StreamError::Tokio(_)
                | StreamError::CodecError(_)
                | StreamError::ChunkError(_)
                | StreamError::DecodeError(_)
                | StreamError::InvalidUri(_)
                | StreamError::InvalidSorbetSchema(_)
                | StreamError::TypeConversionError(_)
                | StreamError::MissingChunkData
                | StreamError::MissingDataframeColumn(_)
                | StreamError::MissingData(_)
                | StreamError::ArrowError(_),
            )
            | Self::ConnectionError(_)
            | Self::TypeConversionError(_)
            | Self::CodecError(_)
            | Self::SorbetError(_)
            | Self::DataFusionError(_) => None,
        }
    }

    pub fn is_missing_token(&self) -> bool {
        if let Some(status) = self.tonic_status() {
            status.code() == tonic::Code::Unauthenticated
                && status.message() == re_auth::ERROR_MESSAGE_MISSING_CREDENTIALS
        } else {
            false
        }
    }

    pub fn is_wrong_token(&self) -> bool {
        if let Some(status) = self.tonic_status() {
            status.code() == tonic::Code::Unauthenticated
                && status.message() == re_auth::ERROR_MESSAGE_INVALID_CREDENTIALS
        } else {
            false
        }
    }
}

pub struct Dataset {
    pub dataset_entry: DatasetEntry,
    pub origin: re_uri::Origin,
}

impl Dataset {
    pub fn id(&self) -> EntryId {
        self.dataset_entry.details.id
    }

    pub fn name(&self) -> &str {
        self.dataset_entry.details.name.as_ref()
    }
}

pub struct Table {
    pub table_entry: TableEntry,

    pub origin: re_uri::Origin,
}

impl Table {
    pub fn id(&self) -> EntryId {
        self.table_entry.details.id
    }

    pub fn name(&self) -> &str {
        self.table_entry.details.name.as_ref()
    }
}

pub enum EntryInner {
    Dataset(Dataset),
    Table(Table),
}

pub struct Entry {
    details: EntryDetails,
    inner: EntryResult<EntryInner>,
}

impl Entry {
    pub fn details(&self) -> &EntryDetails {
        &self.details
    }

    pub fn id(&self) -> EntryId {
        self.details().id
    }

    pub fn name(&self) -> &str {
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
    ) -> Self {
        let entries_fut =
            fetch_entries_and_register_tables(connection_registry, origin, session_context);

        Self {
            entries: RequestedObject::new_with_repaint(runtime, egui_ctx.clone(), entries_fut),
        }
    }

    pub(crate) fn on_frame_start(&mut self) {
        self.entries.on_frame_start();
    }

    pub fn find_entry(&self, entry_id: EntryId) -> Option<&Entry> {
        self.entries.try_as_ref()?.as_ref().ok()?.get(&entry_id)
    }

    pub fn state(&self) -> Poll<Result<&HashMap<EntryId, Entry>, &EntryError>> {
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
) -> EntryResult<HashMap<EntryId, Entry>> {
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

    let origin_ref = &origin;
    let futures_iter = entries
        .into_iter()
        .filter_map(move |e| fetch_entry_details(client.clone(), origin_ref, e));

    let mut entries = HashMap::default();

    let mut futures_unordered: FuturesUnordered<_> = futures_iter.collect();
    while let Some((details, result)) = futures_unordered.next().await {
        let id = details.id;
        let inner_result = result.map(|(inner, provider)| {
            session_ctx.register_table(&details.name, provider).ok();
            inner
        });

        let is_system_table = match &inner_result {
            Ok(EntryInner::Table(table)) => {
                table.table_entry.provider_details.type_url
                    == re_protos::catalog::v1alpha1::SystemTable::type_url()
            }
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
    entry: EntryDetails,
) -> Option<impl Future<Output = FetchEntryDetailsOutput>> {
    // We could also box the future but then we'd need to use `.boxed()` natively and
    // `.boxed_local()` on wasm. Either passes the `Send` type info transparently.
    use itertools::Either::{Left, Right};
    #[expect(clippy::match_same_arms)]
    match &entry.kind {
        // TODO(rerun-io/dataplatform#857): these are often empty datasets, and thus fail. For
        // some reason, this failure is silent but blocks other tables from being registered.
        // Since we don't need these tables yet, we just skip them for now.
        EntryKind::BlueprintDataset => None,
        EntryKind::Dataset => Some(Left(Left(
            fetch_dataset_details(client, entry.id, origin)
                .map_ok(|(dataset, table_provider)| (EntryInner::Dataset(dataset), table_provider))
                .map(move |res| (entry, res)),
        ))),
        EntryKind::Table => Some(Left(Right(
            fetch_table_details(client, entry.id, origin)
                .map_ok(|(table, table_provider)| (EntryInner::Table(table), table_provider))
                .map(move |res| (entry, res)),
        ))),

        // TODO(ab): these do not exist yet
        EntryKind::DatasetView | EntryKind::TableView => None,

        EntryKind::Unspecified => {
            let kind = entry.kind;
            Some(Right(future::ready((
                entry,
                Err(TypeConversionError::from(prost::UnknownEnumValue(kind as i32)).into()),
            ))))
        }
    }
}

async fn fetch_dataset_details(
    mut client: ConnectionClient,
    id: EntryId,
    origin: &re_uri::Origin,
) -> EntryResult<(Dataset, Arc<dyn TableProvider>)> {
    let result = client
        .read_dataset_entry(id)
        .await
        .map(|dataset_entry| Dataset {
            dataset_entry,
            origin: origin.clone(),
        })?;

    let table_provider = PartitionTableProvider::new(client, id)
        .into_provider()
        .await?;

    Ok((result, table_provider))
}

async fn fetch_table_details(
    mut client: ConnectionClient,
    id: EntryId,
    origin: &re_uri::Origin,
) -> EntryResult<(Table, Arc<dyn TableProvider>)> {
    let result = client.read_table_entry(id).await.map(|table_entry| Table {
        table_entry,
        origin: origin.clone(),
    })?;

    let table_provider = TableEntryTableProvider::new(client, id)
        .into_provider()
        .await?;

    Ok((result, table_provider))
}
