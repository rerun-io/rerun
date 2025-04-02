use std::sync::Arc;

use arrow::{
    array::{StructArray, UInt64Array},
    datatypes::SchemaRef,
};
use arrow_flight::{
    decode::FlightRecordBatchStream, flight_service_client::FlightServiceClient, Ticket,
};
use datafusion::{
    catalog::{TableFunctionImpl, TableProvider},
    common::{exec_datafusion_err, exec_err},
    error::{DataFusionError, Result as DataFusionResult},
    prelude::Expr,
    scalar::ScalarValue,
};
use futures_util::{StreamExt as _, TryStreamExt as _};
use prost::Message as _;
use re_protos::{
    catalog::v1alpha1::{
        catalog_service_client::CatalogServiceClient, CatalogFlightRequest, EntryFilter, EntryKind,
        FindEntriesRequest, ReadDatasetEntryRequest,
    },
    common::v1alpha1::{ext::EntryId, DatasetHandle, IndexColumnSelector, Timeline},
    flights::v1alpha1::FlightRequest,
    manifest_registry::v1alpha1::{
        manifest_registry_service_client::ManifestRegistryServiceClient, GetDatasetSchemaRequest,
        ManifestRegistryFlightRequest, Query, QueryDatasetLatestAtRelevantChunks,
        QueryDatasetRequest,
    },
};
use re_tuid::Tuid;
use tonic::transport::Channel;

use crate::flight_response_provider::FlightResponseProvider;

#[derive(Debug, Clone)]
pub struct DatasetTableProvider {
    channel: Channel,
    runtime: tokio::runtime::Handle,
}

impl DatasetTableProvider {
    pub fn new(channel: Channel, runtime: tokio::runtime::Handle) -> Self {
        Self { channel, runtime }
    }
}

// We expect to receive two literal expressions for our table provider - the
// dataset name and the timeline.
impl TableFunctionImpl for DatasetTableProvider {
    fn call(&self, args: &[Expr]) -> DataFusionResult<Arc<dyn TableProvider>> {
        if args.len() != 2 {
            return exec_err!("Expected 2 arguments for DatasetTableProvider as literal strings, dataset name and timeline. Received {}", args.len());
        }

        let dataset_name = match &args[0] {
            Expr::Literal(ScalarValue::Utf8(Some(name)) | ScalarValue::Utf8View(Some(name))) => {
                name
            }
            _ => {
                return exec_err!(
                    "DatasetTableProvider expects dataset name to be a literal string"
                );
            }
        };

        let timeline = match &args[1] {
            Expr::Literal(ScalarValue::Utf8(Some(name)) | ScalarValue::Utf8View(Some(name))) => {
                name
            }
            _ => {
                return exec_err!("DatasetTableProvider expects timeline to be a literal string");
            }
        };

        // let find_entries_request = FlightRequest {
        //     request_type: Some(
        //         re_protos::flights::v1alpha1::flight_request::RequestType::CatalogRequest(
        //             CatalogFlightRequest {
        //                 request_type: Some(re_protos::catalog::v1alpha1::catalog_flight_request::RequestType::FindEntries(FindEntriesRequest {
        //                     filter: Some(EntryFilter {
        //                         id: None,
        //                         name: Some(dataset_name.clone()),
        //                         entry_kind: Some(EntryKind::Dataset.into())
        //                     })
        //                 })),
        //             },
        //         ),
        //     ),
        // };
        // let find_entries_bytes = find_entries_request.encode_to_vec();

        // let find_entries_ticket = Ticket {
        //     ticket: find_entries_bytes.into(),
        // };

        // let find_entries_response = self
        //     .runtime
        //     .block_on(self.client.clone().do_get(find_entries_ticket))
        //     .map_err(|err| DataFusionError::Execution(err.to_string()))?;

        self.runtime.block_on(create_table_provider(
            dataset_name,
            timeline,
            self.channel.clone(),
        ))
    }
}

async fn create_table_provider(
    dataset_name: &str,
    timeline: &str,
    channel: Channel,
) -> DataFusionResult<Arc<dyn TableProvider>> {
    let mut flight_client = FlightServiceClient::new(channel.clone());

    let entry_id = find_entry_id_for_dataset(&mut flight_client, dataset_name)
        .await?
        .ok_or(exec_datafusion_err!("Unable to locate dataset by name"))?;

    let mut catalog_client = CatalogServiceClient::new(channel.clone());
    let dataset_handle = find_dataset_handle(&mut catalog_client, entry_id)
        .await?
        .ok_or(exec_datafusion_err!(
            "Unable to get dataset handle from catalog"
        ))?;

    let mut manifest_client = ManifestRegistryServiceClient::new(channel);
    let schema = get_dataset_schema(&mut manifest_client, dataset_handle.clone()).await?;

    query_dataset(&mut flight_client, dataset_handle, timeline, schema).await
}

async fn find_entry_id_for_dataset(
    client: &mut FlightServiceClient<Channel>,
    dataset_name: &str,
) -> DataFusionResult<Option<EntryId>> {
    let find_entries_request = FlightRequest {
        request_type: Some(
            re_protos::flights::v1alpha1::flight_request::RequestType::CatalogRequest(
                CatalogFlightRequest {
                    request_type: Some(re_protos::catalog::v1alpha1::catalog_flight_request::RequestType::FindEntries(FindEntriesRequest {
                        filter: Some(EntryFilter {
                            id: None,
                            name: Some(dataset_name.to_owned()),
                            entry_kind: Some(EntryKind::Dataset.into())
                        })
                    })),
                },
            ),
        ),
    };
    let find_entries_bytes = find_entries_request.encode_to_vec();

    let find_entries_ticket = Ticket {
        ticket: find_entries_bytes.into(),
    };

    let find_entries_response = client
        .do_get(find_entries_ticket)
        .await
        .map_err(|err| DataFusionError::Execution(err.to_string()))?
        .into_inner();

    let mut record_batch_stream =
        FlightRecordBatchStream::new_from_flight_data(find_entries_response.map_err(|e| e.into()));

    let mut entry_id: Option<EntryId> = None;
    while let Some(flight_result) = record_batch_stream.next().await {
        let flight_data =
            flight_result.map_err(|err| DataFusionError::Execution(err.to_string()))?;

        let id_col = flight_data
            .column_by_name("id")
            .ok_or(exec_datafusion_err!(
                "Expected column `id` for FindEntries not retruned"
            ))?
            .as_any()
            .downcast_ref::<StructArray>()
            .ok_or(exec_datafusion_err!(
                "Expected column `id` for FindEntries to be a struct"
            ))?;

        let time_ns = id_col
            .column_by_name("time_ns")
            .ok_or(exec_datafusion_err!("Missing expected field time_ns in id"))?
            .as_any()
            .downcast_ref::<UInt64Array>()
            .ok_or(exec_datafusion_err!("Field time_ns has unexpected type"))?;
        if time_ns.is_empty() {
            return exec_err!("Empty field time_ns in id");
        }
        let time_nanos = time_ns.value(0);

        let inc_array = id_col
            .column_by_name("inc")
            .ok_or(exec_datafusion_err!("Missing expected field inc in id"))?
            .as_any()
            .downcast_ref::<UInt64Array>()
            .ok_or(exec_datafusion_err!("Field inc has unexpected type"))?;
        if inc_array.is_empty() {
            return exec_err!("Empty field inc in id");
        }
        let inc = inc_array.value(0);

        entry_id = Some(Tuid::from_nanos_and_inc(time_nanos, inc).into());
    }

    Ok(entry_id)
}

async fn find_dataset_handle(
    client: &mut CatalogServiceClient<Channel>,
    entry_id: EntryId,
) -> DataFusionResult<Option<DatasetHandle>> {
    let request = ReadDatasetEntryRequest {
        id: Some(entry_id.into()),
    };

    let handle = client
        .read_dataset_entry(request)
        .await
        .map_err(|err| exec_datafusion_err!("{err}"))?
        .into_inner()
        .dataset
        .and_then(|entry| entry.dataset_handle);

    Ok(handle)
}

async fn get_dataset_schema(
    client: &mut ManifestRegistryServiceClient<Channel>,
    dataset_handle: DatasetHandle,
) -> DataFusionResult<SchemaRef> {
    let request = GetDatasetSchemaRequest {
        entry: Some(dataset_handle),
    };

    client
        .get_dataset_schema(request)
        .await
        .map_err(|err| exec_datafusion_err!("{err}"))?
        .into_inner()
        .schema
        .ok_or(exec_datafusion_err!("Unable to get schema for dataset"))
        .and_then(|schema| schema.try_into().map_err(Into::into))
        .map(Arc::new)
}

async fn query_dataset(
    client: &mut FlightServiceClient<Channel>,
    dataset_handle: DatasetHandle,
    timeline: &str,
    schema: SchemaRef,
) -> DataFusionResult<Arc<dyn TableProvider>> {
    let mut query = Query::default();
    query.latest_at = Some(QueryDatasetLatestAtRelevantChunks {
        entity_paths: Vec::new(),
        index: Some(IndexColumnSelector {
            timeline: Some(Timeline {
                name: timeline.to_owned(),
            }),
        }),
        at: None,
        fuzzy_descriptors: Vec::new(),
    });
    let find_entries_request = FlightRequest {
        request_type: Some(
            re_protos::flights::v1alpha1::flight_request::RequestType::ManifestRegistryRequest(
                ManifestRegistryFlightRequest {
                    request_type: Some(re_protos::manifest_registry::v1alpha1::manifest_registry_flight_request::RequestType::QueryDataset(

                        QueryDatasetRequest { entry: dataset_handle.into(), partition_ids: Vec::new(), chunk_ids: Vec::new(), scan_parameters: None, query: Some(query), })),
                },
            ),
        ),
    };
    let find_entries_bytes = find_entries_request.encode_to_vec();

    let ticket = Ticket {
        ticket: find_entries_bytes.into(),
    };

    FlightResponseProvider {
        schema,
        ticket: Some(ticket),
        client: client.clone(),
    }
    .into_provider()
    .await
}
