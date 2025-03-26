use std::{collections::HashMap, sync::Arc, task::Poll};

use async_trait::async_trait;

use arrow::{
    array::{ArrayRef, Int32Array, Int64Array, RecordBatch, StringArray, StructArray, UInt64Array},
    datatypes::{DataType, Field, Fields, Schema, SchemaRef},
};
use datafusion::{catalog::TableProvider, error::Result as DataFusionResult};
use itertools::multiunzip;
use re_log_types::external::re_tuid::Tuid;
use re_protos::catalog::v1alpha1::{
    catalog_service_client::CatalogServiceClient, EntryFilter, EntryKind, FindEntriesRequest,
    FindEntriesResponse,
};
use tonic::transport::Channel;

use crate::grpc_response_provider::{GrpcResponseProvider, GrpcResponseToTable};

#[derive(Debug, Clone)]
pub struct CatalogFindEntryProvider {
    client: CatalogServiceClient<Channel>,
    tuid_filter: Option<Tuid>,
    name_filter: Option<String>,
    entry_kind_filter: Option<EntryKind>,
}

impl CatalogFindEntryProvider {
    pub fn new(
        client: CatalogServiceClient<Channel>,
        tuid_filter: Option<Tuid>,
        name_filter: Option<String>,
        entry_kind_filter: Option<EntryKind>,
    ) -> Self {
        Self {
            client,
            tuid_filter,
            name_filter,
            entry_kind_filter,
        }
    }

    /// This is a convenience function
    pub fn into_provider(self) -> Arc<dyn TableProvider> {
        let provider: GrpcResponseProvider<Self> = self.into();

        Arc::new(provider)
    }
}

#[async_trait]
impl GrpcResponseToTable for CatalogFindEntryProvider {
    type GrpcResponse = FindEntriesResponse;

    fn create_schema() -> SchemaRef {
        Arc::new(Schema::new_with_metadata(
            vec![
                Field::new(
                    "id",
                    DataType::Struct(Fields::from([
                        Arc::new(Field::new("time_ns", DataType::UInt64, true)),
                        Arc::new(Field::new("inc", DataType::UInt64, true)),
                    ])),
                    true,
                ),
                Field::new("name", DataType::Utf8, true),
                Field::new("entry_kind", DataType::Int32, true),
                Field::new("created_at", DataType::Int64, true),
                Field::new("updated_at", DataType::Int64, true),
            ],
            HashMap::default(),
        ))
    }

    async fn send_request(&mut self) -> Result<tonic::Response<Self::GrpcResponse>, tonic::Status> {
        self.client
            .find_entries(tonic::Request::new(FindEntriesRequest {
                filter: Some(EntryFilter {
                    id: self.tuid_filter.map(Into::into),
                    name: self.name_filter.clone(),
                    entry_kind: self.entry_kind_filter.map(Into::into),
                }),
            }))
            .await
    }

    fn process_response(
        &mut self,
        response: Self::GrpcResponse,
    ) -> std::task::Poll<Option<DataFusionResult<RecordBatch>>> {
        let (id_time_ns, id_inc, name, entry_kind, created_at, updated_at): (
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
        ) = multiunzip(response.entries.into_iter().map(|entry| {
            (
                entry
                    .id
                    .and_then(|entry_id| entry_id.id.map(|id| id.time_ns())),
                entry.id.and_then(|entry_id| entry_id.id.map(|id| id.inc())),
                entry.name,
                entry.entry_kind,
                entry
                    .created_at
                    .map(|t| t.seconds * 1_000_000_000 + t.nanos as i64),
                entry
                    .updated_at
                    .map(|t| t.seconds * 1_000_000_000 + t.nanos as i64),
            )
        }));

        let id_time_ns: ArrayRef = Arc::new(UInt64Array::from(id_time_ns));
        let id_inc: ArrayRef = Arc::new(UInt64Array::from(id_inc));
        let name: ArrayRef = Arc::new(StringArray::from(name));
        let entry_kind: ArrayRef = Arc::new(Int32Array::from(entry_kind));
        let created_at: ArrayRef = Arc::new(Int64Array::from(created_at));
        let updated_at: ArrayRef = Arc::new(Int64Array::from(updated_at));

        let id: ArrayRef = Arc::new(StructArray::from(vec![
            (
                Arc::new(Field::new("time_ns", DataType::UInt64, true)),
                id_time_ns,
            ),
            (Arc::new(Field::new("inc", DataType::UInt64, true)), id_inc),
        ]));

        let record_batch = match RecordBatch::try_from_iter(vec![
            ("id", id),
            ("name", name),
            ("entry_kind", entry_kind),
            ("created_at", created_at),
            ("updated_at", updated_at),
        ]) {
            Ok(rb) => rb,
            Err(err) => return Poll::Ready(Some(Err(err.into()))),
        };

        Poll::Ready(Some(Ok(record_batch)))
    }
}
