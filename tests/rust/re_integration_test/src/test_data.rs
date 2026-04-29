use std::error::Error;
use std::str::FromStr as _;
use std::time::Duration;

use futures::StreamExt as _;

use re_protos::cloud::v1alpha1::QueryTasksResponse;
use re_protos::cloud::v1alpha1::ext::{DataSource, QueryTasksOnCompletionResponse};
use re_protos::cloud::v1alpha1::{EntryFilter, EntryKind};
use re_protos::common::v1alpha1::SegmentId;
use re_protos::common::v1alpha1::ext::IfDuplicateBehavior;
use re_redap_client::ConnectionClient;
use re_sdk::external::re_tuid;
use re_sdk::time::TimeType;
use re_sdk::{RecordingStreamBuilder, TimeCell};
use re_viewer::external::re_sdk_types::archetypes;

pub async fn load_test_data(mut client: ConnectionClient) -> Result<SegmentId, Box<dyn Error>> {
    load_test_data_with_name(
        &mut client,
        "my_dataset",
        "187b552b95a5c2f73f37894708825ba5",
        "new_recording_id",
    )
    .await
}

pub async fn load_test_data_with_name(
    client: &mut ConnectionClient,
    dataset_name: &str,
    dataset_id_str: &str,
    recording_id: &str,
) -> Result<SegmentId, Box<dyn Error>> {
    let path = {
        let path = tempfile::NamedTempFile::new()?;
        let stream = RecordingStreamBuilder::new("rerun_example_integration_test")
            .recording_id(recording_id)
            .save(path.path())?;

        for x in 0..20 {
            stream.set_time("test_time", TimeCell::new(TimeType::Sequence, x));
            stream
                .log(
                    "test_entity",
                    &archetypes::Points3D::new([(x as f32, 0.0, 0.0)]),
                )
                .expect("Failed to log points 3D");
        }

        stream.flush_with_timeout(Duration::from_secs(60))?;

        path
    };

    // Make sure that we have an entries table.
    let entries_table = client
        .find_entries(EntryFilter::default().with_entry_kind(EntryKind::Table))
        .await?;
    assert_eq!(entries_table.len(), 1);
    assert_eq!(entries_table[0].name, re_protos::EntryName::entries_table());
    assert_eq!(entries_table[0].kind, EntryKind::Table);

    let dataset_id = re_tuid::Tuid::from_str(dataset_id_str).expect("Failed to parse TUID");

    let entry = client
        .create_dataset_entry(dataset_name.to_owned(), Some(dataset_id.into()))
        .await?;

    let item = client
        .register_with_dataset(
            entry.details.id,
            vec![DataSource::new_rrd(format!(
                "file://{}",
                path.path()
                    .to_str()
                    .ok_or_else(|| "Failed to convert path to str".to_owned())?
            ))?],
            IfDuplicateBehavior::Error,
        )
        .await?
        .into_iter()
        .next()
        .expect("We created this with one segment");

    let re_protos::cloud::v1alpha1::ext::RegisterWithDatasetTaskDescriptor {
        segment_id,
        segment_type: _,
        storage_url: _,
        task_id,
    } = item;

    // Wait for the registration task to complete:
    let timeout = Duration::from_secs(10);
    let mut response_stream = client
        .query_tasks_on_completion(vec![task_id], timeout)
        .await?;

    while let Some(response) = response_stream.next().await {
        let response: QueryTasksOnCompletionResponse = response?.try_into()?;
        let batch = response.data;
        let status_col = batch
            .column_by_name(QueryTasksResponse::FIELD_EXEC_STATUS)
            .ok_or("missing exec_status column")?
            .as_any()
            .downcast_ref::<arrow::array::StringArray>()
            .ok_or("exec_status should be a string array")?;
        let msgs_col = batch
            .column_by_name(QueryTasksResponse::FIELD_MSGS)
            .ok_or("missing msgs column")?
            .as_any()
            .downcast_ref::<arrow::array::StringArray>()
            .ok_or("msgs should be a string array")?;

        for i in 0..batch.num_rows() {
            let status = status_col.value(i);
            if status != "success" {
                let msg = msgs_col.value(i);
                return Err(format!("Registration task failed with status {status}: {msg}").into());
            }
        }
    }

    Ok(segment_id.into())
}
