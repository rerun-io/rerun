use std::{collections::HashMap, sync::Arc, task::Poll};

use async_trait::async_trait;

use arrow::{
    array::{ArrayRef, Int32Array, Int64Array, RecordBatch, StringArray, StructArray, UInt64Array},
    datatypes::{DataType, Field, Fields, Schema, SchemaRef},
};
use datafusion::error::Result as DataFusionResult;
use itertools::multiunzip;
use re_protos::catalog::v1alpha1::{
    catalog_service_client::CatalogServiceClient, EntryFilter, EntryType, FindEntriesRequest,
    FindEntriesResponse,
};
use tonic::transport::Channel;

use crate::grpc_table_provider::GrpcResultToTable;

#[async_trait]
impl GrpcResultToTable for CatalogServiceClient<Channel> {
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
                Field::new("entry_type", DataType::Int32, true),
                Field::new("created_at", DataType::Int64, true),
                Field::new("updated_at", DataType::Int64, true),
            ],
            HashMap::default(),
        ))
    }

    async fn send_request(&mut self) -> Result<tonic::Response<Self::GrpcResponse>, tonic::Status> {
        self.find_entries(tonic::Request::new(FindEntriesRequest {
            filter: Some(EntryFilter {
                id: None,
                name: None,
                entry_type: Some(EntryType::Dataset.into()),
            }),
        }))
        .await
    }

    fn process_response(
        &mut self,
        response: Self::GrpcResponse,
    ) -> std::task::Poll<Option<DataFusionResult<RecordBatch>>> {
        let (id_time_ns, id_inc, name, entry_type, created_at, updated_at): (
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
        ) = multiunzip(response.entries.into_iter().map(|entry| {
            (
                entry.id.map(|id| id.time_ns),
                entry.id.map(|id| id.inc),
                entry.name,
                entry.entry_type,
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
        let entry_type: ArrayRef = Arc::new(Int32Array::from(entry_type));
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
            ("entry_type", entry_type),
            ("created_at", created_at),
            ("updated_at", updated_at),
        ]) {
            Ok(rb) => rb,
            Err(err) => return Poll::Ready(Some(Err(err.into()))),
        };

        Poll::Ready(Some(Ok(record_batch)))
    }
}
