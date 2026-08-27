use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use arrow::array::RecordBatch;
use futures::StreamExt as _;
use itertools::Itertools as _;
use re_protos::cloud::v1alpha1::RegisterWithDatasetResponse;
use re_protos::cloud::v1alpha1::ext::{
    DataSourceKind, QueryTasksDataframe, RegisterWithDatasetDataframe,
    RegisterWithDatasetTaskDescriptor,
};
use re_protos::common::v1alpha1::ext::SegmentId;
use re_protos::common::v1alpha1::{DataframePart, TaskId};
use re_protos::{TypeConversionError, missing_field};

use crate::{
    ApiError, ApiErrorKind, ApiResponseStream, ApiResult, ConnectionHandle, TraceId,
    format_trace_ids,
};

/// The outcome of one completed segment registration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentRegistrationResult {
    pub recording_uri: String,
    pub segment_id: SegmentId,
    pub error: Option<String>,
}

/// Decode a `/RegisterWithDataset` response payload into task descriptors.
pub(crate) fn parse_task_descriptors(
    origin: &re_uri::Origin,
    trace_id: Option<TraceId>,
    data: Option<DataframePart>,
) -> ApiResult<Vec<RegisterWithDatasetTaskDescriptor>> {
    re_tracing::profile_function!();

    let response: RecordBatch = data
        .ok_or_else(|| {
            let err = missing_field!(RegisterWithDatasetResponse, "data");
            ApiError::deserialization_with_source(
                origin,
                trace_id,
                err,
                "missing field in /RegisterWithDataset response",
            )
        })?
        .try_into()
        .map_err(|err| {
            ApiError::deserialization_with_source(
                origin,
                trace_id,
                err,
                "failed decoding /RegisterWithDataset response",
            )
        })?;

    // Validates the columns (existence, datatype, no nulls):
    let RegisterWithDatasetDataframe {
        rerun_segment_id,
        rerun_segment_layer,
        rerun_segment_type,
        rerun_storage_url,
        rerun_task_id,
    } = RegisterWithDatasetDataframe::try_from(response).map_err(|err| {
        ApiError::deserialization_quiver_from(
            origin,
            trace_id,
            err,
            "/RegisterWithDataset response",
        )
    })?;

    let segment_types = DataSourceKind::many_from_arrow(rerun_segment_type.as_arrow().as_ref())
        .map_err(|err| {
            ApiError::deserialization_with_source(
                origin,
                trace_id,
                err,
                "failed parsing /RegisterWithDataset response",
            )
        })?;

    itertools::izip!(
        rerun_segment_layer.into_iter_owned(),
        rerun_segment_id.into_iter_owned(),
        segment_types,
        rerun_storage_url.into_iter_owned(),
        rerun_task_id.into_iter_owned()
    )
    .map(
        |(layer_name, segment_id, segment_type, storage_url, task_id)| {
            Ok(RegisterWithDatasetTaskDescriptor {
                layer_name,
                segment_id,
                segment_type,
                storage_url: url::Url::parse(&storage_url).map_err(|err| {
                    ApiError::deserialization_with_source(
                        origin,
                        trace_id,
                        TypeConversionError::UrlParseError(err),
                        "failed to parse /RegisterWithDataset response",
                    )
                })?,
                task_id,
            })
        },
    )
    .try_collect()
}

struct TaskCompletion {
    task_id: String,
    status: String,
    message: Option<String>,
}

/// Helper to deserialize a stream of responses into richer types.
fn task_completion_stream(
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

/// A segment result paired with its position in the original descriptor list.
///
/// Completions arrive in task-completion order, and one pooled task may cover several descriptors.
/// [`RegistrationHandle::wait`] uses the index to restore descriptor order, while
/// [`RegistrationHandle::stream_results`] discards it to preserve completion order.
#[derive(Debug)]
struct IndexedRegistrationResult {
    descriptor_index: usize,
    result: SegmentRegistrationResult,
}

#[derive(Debug)]
struct Registration {
    request_trace_id: Option<TraceId>,
    task_descriptors: Box<[RegisterWithDatasetTaskDescriptor]>,

    /// Unique task IDs, in submission order (the server may pool several descriptors into one task).
    task_ids: Vec<TaskId>,
    descriptor_indices_by_task_id: HashMap<TaskId, Vec<usize>>,
}

/// A submitted dataset registration.
///
/// This retains the originating [`ConnectionHandle`], so lifecycle operations always resolve
/// through the registry and origin that submitted the registration.
/// Registration happens asynchronously on the server.
/// Dropping the handle does not cancel server work.
#[must_use = "registration continues asynchronously; wait for, stream, or cancel it"]
#[derive(Clone, Debug)]
pub struct RegistrationHandle {
    connection: ConnectionHandle,
    inner: Arc<Registration>,
}

impl RegistrationHandle {
    pub(crate) fn new(
        connection: ConnectionHandle,
        request_trace_id: Option<TraceId>,
        task_descriptors: Vec<RegisterWithDatasetTaskDescriptor>,
    ) -> Self {
        re_tracing::profile_function!();

        let mut task_ids = Vec::new();
        let mut descriptor_indices_by_task_id: HashMap<TaskId, Vec<usize>> = HashMap::new();
        for (index, descriptor) in task_descriptors.iter().enumerate() {
            descriptor_indices_by_task_id
                .entry(descriptor.task_id.clone())
                .or_insert_with(|| {
                    task_ids.push(descriptor.task_id.clone());
                    Vec::new()
                })
                .push(index);
        }

        Self {
            connection,
            inner: Arc::new(Registration {
                request_trace_id,
                task_descriptors: task_descriptors.into(),
                task_ids,
                descriptor_indices_by_task_id,
            }),
        }
    }

    /// Trace ID of the request that submitted the registration tasks.
    pub fn trace_id(&self) -> Option<TraceId> {
        self.inner.request_trace_id
    }

    /// Descriptors returned by the registration request, in submission order.
    pub fn descriptors(&self) -> &[RegisterWithDatasetTaskDescriptor] {
        &self.inner.task_descriptors
    }

    fn task_ids(&self) -> Vec<TaskId> {
        self.inner.task_ids.clone()
    }

    async fn indexed_results(
        &self,
        timeout: Duration,
    ) -> ApiResult<ApiResponseStream<IndexedRegistrationResult>> {
        if self.descriptors().is_empty() {
            // No tasks were run, so we also don't need a trace id.
            return Ok(ApiResponseStream::new(
                self.connection.origin().clone(),
                tokio_stream::empty(),
                None,
            ));
        }

        let responses = self
            .connection
            .client()
            .await?
            .query_tasks_on_completion(self.task_ids(), timeout)
            .await?;
        let query_trace_id = responses.trace_id();
        let origin = self.connection.origin().clone();
        let registration = self.inner.clone();
        let stream = task_completion_stream(responses).flat_map(move |completion| {
            let results = completion_results(&origin, &registration, completion)
                .into_iter()
                .map(move |result| result.map_err(|err| err.with_trace_id(query_trace_id)));
            tokio_stream::iter(results)
        });

        Ok(ApiResponseStream::new(
            self.connection.origin().clone(),
            stream,
            query_trace_id,
        ))
    }

    /// Stream segment registration results as their tasks complete.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn stream_results(
        &self,
        timeout: Duration,
    ) -> ApiResult<ApiResponseStream<SegmentRegistrationResult>> {
        let results = self.indexed_results(timeout).await?;
        let query_trace_id = results.trace_id();
        Ok(ApiResponseStream::new(
            self.connection.origin().clone(),
            results.map(|result| result.map(|result| result.result)),
            query_trace_id,
        ))
    }

    /// Cancel all registration tasks.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn cancel(&self) -> ApiResult {
        if self.descriptors().is_empty() {
            return Ok(());
        }

        self.connection
            .client()
            .await?
            .cancel_tasks(self.task_ids())
            .await
    }

    /// Wait for all registration tasks to complete.
    ///
    /// Returns the registered segment IDs in descriptor order.
    /// Returns an [`ApiError`] if any task fails or if the request, query, or timeout fails.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn wait(&self, timeout: Duration) -> ApiResult<Vec<SegmentId>> {
        let results = self.indexed_results(timeout).await?;
        self.collect_results(results).await
    }

    async fn collect_results(
        &self,
        mut results: ApiResponseStream<IndexedRegistrationResult>,
    ) -> ApiResult<Vec<SegmentId>> {
        let query_trace_id = results.trace_id();
        let mut outcomes = vec![None; self.descriptors().len()];
        while let Some(result) = results.next().await {
            let result = result?;
            let previous = outcomes[result.descriptor_index].replace(result.result);
            re_log::debug_assert!(
                previous.is_none(),
                "task-completion stream returned descriptor {} more than once",
                result.descriptor_index
            );
        }

        // A cleanly exhausted completion stream reports each requested task exactly once.
        re_log::debug_assert!(
            outcomes.iter().all(Option::is_some),
            "task-completion stream ended before every requested task reached a terminal state"
        );

        let mut segment_ids = Vec::with_capacity(outcomes.len());
        let mut seen_errors = HashSet::new();
        let mut errors = Vec::new();
        for (descriptor, outcome) in std::iter::zip(self.descriptors(), outcomes) {
            let error = match outcome {
                Some(result) if result.error.is_none() => {
                    segment_ids.push(result.segment_id);
                    continue;
                }
                Some(result) => result.error,
                None => Some(format!(
                    "registration task '{}' completed without a result",
                    descriptor.task_id.id
                )),
            };
            if let Some(error) = error
                && seen_errors.insert(error.clone())
            {
                errors.push(error);
            }
        }

        if errors.is_empty() {
            return Ok(segment_ids);
        }

        Err(ApiError::invalid_arguments(
            &self.connection.origin().clone(),
            format!(
                "Registration failed.{}\n\nThe following segments failed:\n{}",
                format_trace_ids(self.trace_id(), query_trace_id),
                errors.join("\n")
            ),
        ))
    }
}

fn completion_results(
    origin: &re_uri::Origin,
    registration: &Registration,
    completion: ApiResult<TaskCompletion>,
) -> Vec<ApiResult<IndexedRegistrationResult>> {
    re_tracing::profile_function!();

    let TaskCompletion {
        task_id,
        status,
        message,
    } = match completion {
        Ok(completion) => completion,
        Err(err) => return vec![Err(err)],
    };
    let task_id = TaskId { id: task_id };
    let Some(indices) = registration.descriptor_indices_by_task_id.get(&task_id) else {
        return vec![Err(ApiError::with_kind_and_source(
            origin,
            ApiErrorKind::InvalidServer,
            None,
            std::io::Error::other("server returned an unrequested registration task"),
            format!(
                "task-completion query returned unrequested task ID '{}'",
                task_id.id
            ),
        ))];
    };
    let error = registration_error(&task_id, &status, message.as_deref());
    indices
        .iter()
        .map(|&descriptor_index| {
            let descriptor = &registration.task_descriptors[descriptor_index];
            Ok(IndexedRegistrationResult {
                descriptor_index,
                result: SegmentRegistrationResult {
                    recording_uri: descriptor.storage_url.to_string(),
                    segment_id: descriptor.segment_id.clone(),
                    error: error.clone(),
                },
            })
        })
        .collect()
}

fn registration_error(task_id: &TaskId, status: &str, message: Option<&str>) -> Option<String> {
    match status {
        "success" => None,
        "cancelled" => Some("registration was cancelled".to_owned()),
        _ => message
            .filter(|message| !message.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| {
                Some(format!(
                    "registration task '{}' finished with status '{status}'",
                    task_id.id
                ))
            }),
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use re_protos::cloud::v1alpha1::ext::DataSourceKind;
    use re_types_core::LayerName;

    use super::*;
    use crate::ConnectionRegistry;

    fn descriptor(task_id: &str, segment_id: &str) -> RegisterWithDatasetTaskDescriptor {
        RegisterWithDatasetTaskDescriptor {
            layer_name: LayerName::base(),
            segment_id: SegmentId::from(segment_id),
            segment_type: DataSourceKind::Rrd,
            storage_url: url::Url::parse(&format!("file:///{segment_id}.rrd"))
                .expect("test URL is valid"),
            task_id: TaskId {
                id: task_id.to_owned(),
            },
        }
    }

    fn connection_handle() -> ConnectionHandle {
        ConnectionHandle::new(
            ConnectionRegistry::new_without_stored_credentials(),
            re_uri::Origin::http_local_host(1),
        )
    }

    fn handle() -> RegistrationHandle {
        RegistrationHandle::new(
            connection_handle(),
            None,
            vec![
                descriptor("A", "segment-0"),
                descriptor("B", "segment-1"),
                descriptor("A", "segment-2"),
            ],
        )
    }

    fn completion(task_id: &str, status: &str, message: Option<&str>) -> TaskCompletion {
        TaskCompletion {
            task_id: task_id.to_owned(),
            status: status.to_owned(),
            message: message.map(ToOwned::to_owned),
        }
    }

    #[test]
    fn indexes_pooled_tasks_in_descriptor_order() {
        let handle = handle();
        assert_eq!(
            handle.task_ids(),
            vec![TaskId { id: "A".into() }, TaskId { id: "B".into() }]
        );
        assert_eq!(
            handle.inner.descriptor_indices_by_task_id[&TaskId { id: "A".into() }],
            [0, 2]
        );

        let results = completion_results(
            &re_uri::Origin::test(),
            &handle.inner,
            Ok(completion("A", "success", None)),
        );
        let indices = results
            .into_iter()
            .map(|result| result.expect("completion is valid").descriptor_index)
            .collect::<Vec<_>>();
        assert_eq!(indices, [0, 2]);
    }

    #[tokio::test]
    async fn wait_preserves_descriptor_order() {
        let handle = handle();
        let results = [
            completion_results(
                &re_uri::Origin::test(),
                &handle.inner,
                Ok(completion("B", "success", None)),
            ),
            completion_results(
                &re_uri::Origin::test(),
                &handle.inner,
                Ok(completion("A", "success", None)),
            ),
        ]
        .into_iter()
        .flatten();
        let stream =
            ApiResponseStream::new(re_uri::Origin::test(), tokio_stream::iter(results), None);

        assert_eq!(
            handle
                .collect_results(stream)
                .await
                .expect("all tasks completed successfully"),
            [
                SegmentId::from("segment-0"),
                SegmentId::from("segment-1"),
                SegmentId::from("segment-2")
            ]
        );
    }

    #[test]
    fn converts_registration_statuses() {
        let task_id = TaskId { id: "A".into() };
        assert_eq!(registration_error(&task_id, "success", None), None);
        assert_eq!(
            registration_error(&task_id, "cancelled", None).as_deref(),
            Some("registration was cancelled")
        );
        assert_eq!(
            registration_error(&task_id, "failed", Some("specific failure")).as_deref(),
            Some("specific failure")
        );
        assert_eq!(
            registration_error(&task_id, "not found", None).as_deref(),
            Some("registration task 'A' finished with status 'not found'")
        );
        assert_eq!(
            registration_error(&task_id, "not found", Some("")).as_deref(),
            Some("registration task 'A' finished with status 'not found'")
        );
    }

    #[cfg(debug_assertions)]
    #[tokio::test]
    #[should_panic(
        expected = "task-completion stream ended before every requested task reached a terminal state"
    )]
    async fn debug_asserts_missing_completion() {
        handle()
            .collect_results(ApiResponseStream::new(
                re_uri::Origin::test(),
                tokio_stream::empty(),
                None,
            ))
            .await
            .expect("a cleanly exhausted stream succeeds");
    }

    #[test]
    fn rejects_unrequested_task_completion() {
        let handle = handle();
        let err = completion_results(
            &re_uri::Origin::test(),
            &handle.inner,
            Ok(completion("unknown", "success", None)),
        )
        .pop()
        .expect("completion produces a result")
        .expect_err("unrequested task must fail");
        assert!(err.message.contains("unrequested task ID 'unknown'"));
    }

    #[tokio::test]
    async fn empty_lifecycle_does_not_connect() {
        let handle = RegistrationHandle::new(connection_handle(), None, Vec::new());

        assert!(
            handle
                .wait(Duration::ZERO)
                .await
                .expect("empty wait succeeds")
                .is_empty()
        );
        assert!(
            handle
                .stream_results(Duration::ZERO)
                .await
                .expect("empty stream succeeds")
                .next()
                .await
                .is_none()
        );
        handle.cancel().await.expect("empty cancellation succeeds");
    }
}
