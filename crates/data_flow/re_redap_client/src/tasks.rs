//! Server tasks and how they ended.

use futures::StreamExt as _;
use re_protos::cloud::v1alpha1::ext::QueryTasksDataframe;

use crate::{ApiError, ApiResponseStream};

/// One task that reached a terminal state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskCompletion {
    pub task_id: String,

    /// The terminal status, such as `success` or `cancelled`.
    pub status: String,

    /// What the task reported, such as its error message.
    pub message: Option<String>,
}

impl TaskCompletion {
    pub fn is_success(&self) -> bool {
        self.status == "success"
    }
}

/// Helper to deserialize a stream of responses into richer types.
pub(crate) fn task_completion_stream(
    responses: ApiResponseStream<re_protos::cloud::v1alpha1::QueryTasksOnCompletionResponse>,
) -> ApiResponseStream<TaskCompletion> {
    let origin = responses.origin().clone();
    let query_trace_id = responses.trace_id();
    let stream = responses.flat_map({
        let origin = origin.clone();
        move |response| {
            let origin = origin.clone();
            re_tracing::profile_scope!("decode_task_completions");

            let completions = (|| {
                let response: re_protos::cloud::v1alpha1::ext::QueryTasksOnCompletionResponse =
                    response?.try_into().map_err(|err| {
                        ApiError::deserialization_with_source(
                            &origin,
                            query_trace_id,
                            err,
                            "failed decoding /QueryTasksOnCompletion response",
                        )
                    })?;
                let on_err = |err| {
                    ApiError::deserialization_quiver_from(
                        &origin,
                        query_trace_id,
                        err,
                        "/QueryTasksOnCompletion response",
                    )
                };
                let task_ids = QueryTasksDataframe::COLUMN_TASK_ID
                    .extract(&response.data)
                    .map_err(&on_err)?;
                let statuses = QueryTasksDataframe::COLUMN_EXEC_STATUS
                    .extract(&response.data)
                    .map_err(&on_err)?;
                let messages = QueryTasksDataframe::COLUMN_MSGS
                    .extract(&response.data)
                    .map_err(on_err)?;

                if response.data.column_by_name("blob").is_some() {
                    let blobs = QueryTasksDataframe::COLUMN_BLOB
                        .extract(&response.data)
                        .map_err(on_err)?;
                    for blob in blobs.iter().flatten() {
                        observe_task_meta(blob);
                    }
                }

                Ok(itertools::izip!(&task_ids, &statuses, &messages)
                    .map(|(task_id, status, message)| TaskCompletion {
                        task_id: task_id.to_owned(),
                        status: status.to_owned(),
                        message: message.map(ToOwned::to_owned),
                    })
                    .map(Ok)
                    .collect())
            })()
            .unwrap_or_else(|err| vec![Err(err)]);

            tokio_stream::iter(completions)
        }
    });

    ApiResponseStream::new(origin, stream, query_trace_id)
}

fn observe_task_meta(blob: &[u8]) {
    // Task output is opaque; only Arrow outputs carrying both fields report a revision.
    let Ok(reader) = arrow::ipc::reader::StreamReader::try_new(blob, None) else {
        return;
    };
    let schema = reader.schema();
    let metadata = schema.metadata();
    if let (Some(entry_id), Some(revision)) =
        (metadata.get("entry_id"), metadata.get("dataset_revision"))
    {
        if let (Ok(entry_id), Ok(revision)) =
            (entry_id.parse::<re_log_types::EntryId>(), revision.parse())
        {
            crate::dataset_revisions().observe(entry_id, revision);
        } else {
            re_log::warn_once!("Invalid dataset revision metadata in task output");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observe_task_meta_updates_watermark() {
        let revisions = crate::dataset_revisions();
        let id = re_log_types::EntryId::new();
        let schema = arrow::datatypes::Schema::empty().with_metadata(
            [
                ("entry_id".to_owned(), id.to_string()),
                ("dataset_revision".to_owned(), "42".to_owned()),
            ]
            .into(),
        );
        let mut blob = Vec::new();
        let mut writer = arrow::ipc::writer::StreamWriter::try_new(&mut blob, &schema).unwrap();
        writer.finish().unwrap();
        observe_task_meta(&blob);
        assert_eq!(revisions.get(id), Some(42));
        observe_task_meta(b"opaque task output");
        revisions.observe(id, 50);
        observe_task_meta(&blob);
        assert_eq!(revisions.get(id), Some(50));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn completion_decoder_tolerates_old_servers_and_records_blobs() {
        use arrow::array::{ArrayRef, BinaryArray, RecordBatch, StringArray};
        use std::sync::Arc;

        let id = re_log_types::EntryId::new();
        let schema = arrow::datatypes::Schema::empty().with_metadata(
            [
                ("entry_id".to_owned(), id.to_string()),
                ("dataset_revision".to_owned(), "17".to_owned()),
            ]
            .into(),
        );
        let mut blob = Vec::new();
        arrow::ipc::writer::StreamWriter::try_new(&mut blob, &schema)
            .unwrap()
            .finish()
            .unwrap();
        for include_blob in [false, true] {
            let mut columns: Vec<(&str, ArrayRef)> = vec![
                ("task_id", Arc::new(StringArray::from(vec!["task"]))),
                ("exec_status", Arc::new(StringArray::from(vec!["success"]))),
                ("msgs", Arc::new(StringArray::from(vec![None::<&str>]))),
            ];
            if include_blob {
                columns.push((
                    "blob",
                    Arc::new(BinaryArray::from(vec![Some(blob.as_slice())])),
                ));
            }
            let batch = RecordBatch::try_from_iter(columns).unwrap();
            let response = re_protos::cloud::v1alpha1::QueryTasksOnCompletionResponse {
                data: Some(batch.into()),
            };
            let revisions = crate::dataset_revisions();
            let responses = ApiResponseStream::new(
                re_uri::Origin::test(),
                tokio_stream::iter([Ok(response)]),
                None,
            );
            let mut completions = task_completion_stream(responses);
            let completion = completions.next().await.unwrap().unwrap();
            assert!(completion.is_success());
            assert_eq!(completion.task_id, "task");
            assert_eq!(revisions.get(id), include_blob.then_some(17));
            assert!(completions.next().await.is_none());
        }
    }
}
