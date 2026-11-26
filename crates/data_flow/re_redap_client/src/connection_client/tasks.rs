use re_protos::cloud::v1alpha1::ext::{QueryTasksOnCompletionRequest, QueryTasksRequest};
use re_protos::cloud::v1alpha1::{
    CancelTasksRequest, QueryTasksOnCompletionResponse, QueryTasksResponse,
};
use std::time::Duration;

use futures::TryStreamExt as _;
use re_protos::common::v1alpha1::TaskId;
use re_protos::external::prost::bytes::Bytes;
use tonic::codegen::{Body, StdError};

use super::RedapClient;
use crate::tasks::task_completion_stream;
use crate::{ApiError, ApiResponseStream, ApiResult, TaskCompletion, TraceId};

impl<T> RedapClient<T>
where
    T: tonic::client::GrpcService<tonic::body::Body> + Clone,
    T::Error: Into<StdError>,
    T::ResponseBody: Body<Data = Bytes> + std::marker::Send + 'static,
    <T::ResponseBody as Body>::Error: Into<StdError> + std::marker::Send,
{
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn query_tasks_on_completion(
        &mut self,
        task_ids: Vec<TaskId>,
        timeout: std::time::Duration,
    ) -> ApiResult<ApiResponseStream<QueryTasksOnCompletionResponse>> {
        let q = QueryTasksOnCompletionRequest { task_ids, timeout };
        let origin = self.origin.clone();
        let response = self
            .inner()
            .query_tasks_on_completion(tonic::Request::new(q.try_into().map_err(|err| {
                ApiError::serialization_with_source(
                    &origin,
                    err,
                    "failed building /QueryTasksOnCompletion request",
                )
            })?))
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/QueryTasksOnCompletion"))?;
        Ok(ApiResponseStream::from_tonic_response(
            self.origin.clone(),
            response,
            "/QueryTasksOnCompletion",
        ))
    }

    /// Wait for the given tasks to finish, reporting each one as it ends.
    ///
    /// The server ends the stream once every task has finished. If `timeout` is reached first, the
    /// stream yields a `DEADLINE_EXCEEDED` error and any task still running is never reported.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn task_completions(
        &mut self,
        task_ids: Vec<TaskId>,
        timeout: std::time::Duration,
    ) -> ApiResult<ApiResponseStream<TaskCompletion>> {
        let responses = self.query_tasks_on_completion(task_ids, timeout).await?;
        Ok(task_completion_stream(responses))
    }

    /// Wait for the given tasks to finish, and return how each one ended, along with the trace-id
    /// of the completion query.
    ///
    /// Retries if the server becomes unavailable during the wait, within `timeout`.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn task_outcomes(
        &mut self,
        task_ids: Vec<TaskId>,
        timeout: Duration,
    ) -> ApiResult<(Vec<TaskCompletion>, Option<TraceId>)> {
        let deadline = web_time::Instant::now() + timeout;
        let mut backoff = re_backoff::BackoffGenerator::new_const(
            Duration::from_millis(100),
            Duration::from_secs(1),
        );
        loop {
            let remaining = deadline.saturating_duration_since(web_time::Instant::now());
            let completions = self.task_completions(task_ids.clone(), remaining).await?;
            let trace_id = completions.trace_id();
            match completions.try_collect().await {
                Err(err) if err.kind.is_retryable() && !remaining.is_zero() => {
                    re_log::debug!("Waiting for tasks failed, starting over: {err}");
                    backoff.gen_next().sleep().await;
                }
                result => return result.map(|completions| (completions, trace_id)),
            }
        }
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub async fn cancel_tasks(&mut self, task_ids: Vec<TaskId>) -> ApiResult {
        self.inner()
            .cancel_tasks(CancelTasksRequest { ids: task_ids })
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/CancelTasks"))?;

        Ok(())
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub async fn query_tasks(&mut self, task_ids: Vec<TaskId>) -> ApiResult<QueryTasksResponse> {
        let q = QueryTasksRequest { task_ids };
        let origin = self.origin.clone();
        let response = self
            .inner()
            .query_tasks(tonic::Request::new(q.try_into().map_err(|err| {
                ApiError::serialization_with_source(
                    &origin,
                    err,
                    "failed building /QueryTasks request",
                )
            })?))
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/QueryTasks"))?
            .into_inner();
        Ok(response)
    }

    /// Wait for the given tasks to finish, failing if any of them did not succeed.
    ///
    /// Reaching `timeout` while a task is still running also fails.
    ///
    /// A task failure has the trace-id of `request_trace_id` when given, which is the request that
    /// started the tasks, and the trace-id of the completion query otherwise.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn wait_for_tasks(
        &mut self,
        task_ids: Vec<TaskId>,
        request_trace_id: Option<TraceId>,
        timeout: std::time::Duration,
    ) -> ApiResult {
        if task_ids.is_empty() {
            return Ok(());
        }

        let (completions, query_trace_id) = self.task_outcomes(task_ids, timeout).await?;
        let trace_id = request_trace_id.or(query_trace_id);

        let mut failures = Vec::new();

        for completion in completions {
            if completion.is_success() {
                continue;
            }

            let TaskCompletion {
                task_id,
                status,
                message,
            } = completion;
            let message = message.unwrap_or_default();
            failures.push(format!("task '{task_id}' ended as {status}: {message}"));
        }

        if failures.is_empty() {
            Ok(())
        } else {
            Err(ApiError::internal(&self.origin, failures.join("\n")).with_trace_id(trace_id))
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Arc;

    use arrow::array::{ArrayRef, RecordBatch, StringArray};
    use http_body::Frame;
    use re_protos::cloud::v1alpha1::ext::QueryTasksDataframe;
    use re_protos::external::prost::Message as _;

    use super::*;
    use crate::ApiErrorKind;
    use crate::grpc_test_util::{Frames, grpc_frame, scripted_client};

    fn task_ids(ids: &[&str]) -> Vec<TaskId> {
        ids.iter()
            .map(|id| TaskId {
                id: (*id).to_owned(),
            })
            .collect()
    }

    /// A completion stream that reports that `succeeded` finished, then ends with `status`.
    fn completion_stream(
        succeeded: &[&str],
        status: tonic::Status,
    ) -> http::Response<tonic::body::Body> {
        let columns: Vec<(&str, ArrayRef)> = vec![
            (
                QueryTasksDataframe::COLUMN_TASK_ID_NAME,
                Arc::new(StringArray::from(succeeded.to_vec())),
            ),
            (
                QueryTasksDataframe::COLUMN_EXEC_STATUS_NAME,
                Arc::new(StringArray::from(vec!["success"; succeeded.len()])),
            ),
            (
                QueryTasksDataframe::COLUMN_MSGS_NAME,
                Arc::new(StringArray::from(vec![None::<&str>; succeeded.len()])),
            ),
        ];
        let completions = QueryTasksOnCompletionResponse {
            data: Some(
                RecordBatch::try_from_iter(columns)
                    .expect("valid batch")
                    .into(),
            ),
        };
        let frames = VecDeque::from([
            grpc_frame(&completions),
            Frame::trailers(status.into_http::<()>().into_parts().0.headers),
        ]);
        let mut response = http::Response::new(tonic::body::Body::new(Frames(frames)));
        response.headers_mut().insert(
            "content-type",
            http::HeaderValue::from_static("application/grpc"),
        );
        response
    }

    #[tokio::test(start_paused = true)]
    async fn task_outcomes_start_over_when_the_server_shuts_down() {
        let (mut client, requests) = scripted_client(vec![
            completion_stream(&["a"], tonic::Status::unavailable("server shutting down")),
            completion_stream(&["a", "b"], tonic::Status::ok("")),
        ]);
        let timeout = Duration::from_secs(60);

        let (completions, _) = client
            .task_outcomes(task_ids(&["a", "b"]), timeout)
            .await
            .expect("the wait starts over and succeeds");

        let reported: Vec<_> = completions.iter().map(|c| c.task_id.as_str()).collect();
        assert_eq!(reported, ["a", "b"]);
        let requests = requests.lock();
        assert_eq!(requests.len(), 2);
        let retry = re_protos::cloud::v1alpha1::QueryTasksOnCompletionRequest::decode(
            requests[1]
                .get(5..)
                .expect("a gRPC message has a 5-byte prefix"),
        )
        .expect("a valid request");
        assert_eq!(retry.ids, task_ids(&["a", "b"]));
        let retry_timeout: Duration = retry
            .timeout
            .expect("the retry has a timeout")
            .try_into()
            .expect("a valid timeout");
        assert!(retry_timeout <= timeout, "{retry_timeout:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn task_outcomes_give_up_on_final_errors_and_at_the_timeout() {
        for (status, timeout) in [
            (
                tonic::Status::not_found("no such task"),
                Duration::from_secs(60),
            ),
            (
                tonic::Status::unavailable("server shutting down"),
                Duration::ZERO,
            ),
        ] {
            let kind = ApiErrorKind::from(status.code());
            let (mut client, requests) = scripted_client(vec![completion_stream(&["a"], status)]);

            let err = client
                .task_outcomes(task_ids(&["a", "b"]), timeout)
                .await
                .expect_err("the wait fails");

            assert_eq!(err.kind, kind);
            assert_eq!(requests.lock().len(), 1, "{kind}");
        }
    }
}
