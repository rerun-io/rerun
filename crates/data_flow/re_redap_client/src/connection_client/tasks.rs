use re_protos::cloud::v1alpha1::ext::{QueryTasksOnCompletionRequest, QueryTasksRequest};
use re_protos::cloud::v1alpha1::{
    CancelTasksRequest, QueryTasksOnCompletionResponse, QueryTasksResponse,
};
use re_protos::common::v1alpha1::TaskId;
use re_protos::external::prost::bytes::Bytes;
use tokio_stream::StreamExt as _;
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

        let completions = self.task_completions(task_ids, timeout).await?;
        let trace_id = request_trace_id.or_else(|| completions.trace_id());
        let mut completions = std::pin::pin!(completions);

        let mut failures = Vec::new();

        while let Some(completion) = completions.next().await {
            let completion = completion?;
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
