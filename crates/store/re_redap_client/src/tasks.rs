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
