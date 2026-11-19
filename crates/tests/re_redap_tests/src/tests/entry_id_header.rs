#![expect(
    deprecated,
    reason = "exercises the deprecated body entry ids that old clients still send"
)]

use futures::TryStreamExt as _;
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::ext::{
    EntryDetailsUpdate, ReadTableEntryResponse, UpdateDatasetEntryResponse, UpdateEntryResponse,
    UpdateTableEntryResponse,
};
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::cloud::v1alpha1::{
    DeleteEntryRequest, GetTableSchemaRequest, ReadDatasetEntryRequest, ReadTableEntryRequest,
    ScanTableRequest, UpdateDatasetEntryRequest, UpdateEntryRequest, UpdateTableEntryRequest,
};
use re_protos::headers::RerunHeadersInjectorExt as _;

use super::common::{RerunCloudServiceExt as _, create_table_entry_with_name, entry_name};

/// Every endpoint that still carries a deprecated body entry id must resolve its entry from the
/// `x-rerun-entry-id` header alone.
pub async fn deprecated_body_id_endpoints_accept_header_only(service: impl RerunCloudService) {
    exercise_endpoints(&service, Addressing::Header).await;
}

/// Clients that predate header addressing send the entry id in the body only.
pub async fn deprecated_body_id_endpoints_accept_body_only(service: impl RerunCloudService) {
    exercise_endpoints(&service, Addressing::Body).await;
}

#[derive(Clone, Copy)]
enum Addressing {
    Header,
    Body,
}

impl Addressing {
    /// Builds a request for `entry_id`, handing `with_body_id` the body id to put in the message.
    fn request<T>(
        self,
        entry_id: EntryId,
        with_body_id: impl FnOnce(Option<re_protos::common::v1alpha1::EntryId>) -> T,
    ) -> tonic::Request<T> {
        match self {
            Self::Header => tonic::Request::new(with_body_id(None)).with_entry_id(entry_id),
            Self::Body => tonic::Request::new(with_body_id(Some(entry_id.into()))),
        }
    }
}

async fn exercise_endpoints(service: &impl RerunCloudService, addressing: Addressing) {
    let dataset = service
        .create_dataset_entry_with_name("addressed_dataset")
        .await;
    let dataset_id = dataset.details.id;

    let table_dir = tempfile::tempdir().expect("create temp dir");
    let table = create_table_entry_with_name(service, "addressed_table", &table_dir).await;
    let table_id = table.details.id;

    let response: UpdateEntryResponse = service
        .update_entry(addressing.request(dataset_id, |id| {
            UpdateEntryRequest {
                id,
                entry_details_update: Some(
                    EntryDetailsUpdate {
                        name: Some(entry_name("addressed_dataset_renamed")),
                    }
                    .into(),
                ),
            }
        }))
        .await
        .expect("UpdateEntry")
        .into_inner()
        .try_into()
        .expect("valid UpdateEntry response");
    assert_eq!(response.entry_details.id, dataset_id);
    assert_eq!(
        response.entry_details.name,
        entry_name("addressed_dataset_renamed")
    );

    let response: UpdateDatasetEntryResponse = service
        .update_dataset_entry(
            addressing.request(dataset_id, |id| UpdateDatasetEntryRequest {
                id,
                dataset_details: Some(dataset.dataset_details.clone().into()),
            }),
        )
        .await
        .expect("UpdateDatasetEntry")
        .into_inner()
        .try_into()
        .expect("valid UpdateDatasetEntry response");
    assert_eq!(response.dataset_entry.details.id, dataset_id);

    let response: ReadTableEntryResponse = service
        .read_table_entry(addressing.request(table_id, |id| ReadTableEntryRequest { id }))
        .await
        .expect("ReadTableEntry")
        .into_inner()
        .try_into()
        .expect("valid ReadTableEntry response");
    assert_eq!(response.table_entry.details.id, table_id);

    let response: UpdateTableEntryResponse = service
        .update_table_entry(addressing.request(table_id, |id| UpdateTableEntryRequest {
            id,
            table_details: Some(table.table_details.clone().into()),
        }))
        .await
        .expect("UpdateTableEntry")
        .into_inner()
        .try_into()
        .expect("valid UpdateTableEntry response");
    assert_eq!(response.table_entry.details.id, table_id);

    let schema: arrow::datatypes::Schema = service
        .get_table_schema(
            addressing.request(table_id, |table_id| GetTableSchemaRequest { table_id }),
        )
        .await
        .expect("GetTableSchema")
        .into_inner()
        .schema
        .expect("schema should be present")
        .try_into()
        .expect("valid Arrow schema");
    let column_names: Vec<_> = schema.fields().iter().map(|field| field.name()).collect();
    assert_eq!(column_names, ["column_a"]);

    let _batches: Vec<_> = service
        .scan_table(addressing.request(table_id, |table_id| ScanTableRequest { table_id }))
        .await
        .expect("ScanTable")
        .into_inner()
        .try_collect()
        .await
        .expect("collect ScanTable results");

    for entry_id in [table_id, dataset_id] {
        service
            .delete_entry(addressing.request(entry_id, |id| DeleteEntryRequest { id }))
            .await
            .expect("DeleteEntry");
    }

    let status = service
        .read_table_entry(addressing.request(table_id, |id| ReadTableEntryRequest { id }))
        .await
        .expect_err("the table was deleted");
    assert_eq!(
        status.code(),
        tonic::Code::NotFound,
        "unexpected status: {status:?}"
    );

    let status = service
        .read_dataset_entry(
            tonic::Request::new(ReadDatasetEntryRequest {}).with_entry_id(dataset_id),
        )
        .await
        .expect_err("the dataset was deleted");
    assert_eq!(
        status.code(),
        tonic::Code::NotFound,
        "unexpected status: {status:?}"
    );
}
