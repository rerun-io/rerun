use std::collections::BTreeMap;
use std::hash::Hasher as _;
use std::str::FromStr;
use std::sync::Arc;

use arrow::array::RecordBatch;
use arrow::array::StringArray;
use arrow::array::TimestampNanosecondArray;
use arrow::array::UInt8Array;
use arrow::array::UInt64Array;
use arrow::datatypes::DataType;
use arrow::datatypes::Field;
use arrow::datatypes::Schema;
use arrow::datatypes::TimeUnit;
use arrow::error::ArrowError;
use jiff::Timestamp;

use crate::common::v1alpha1::DataframePart;
use crate::common::v1alpha1::TaskId as ProtoTaskId;
use crate::v1alpha1::rerun_redap_tasks_v1alpha1::QueryTasksResponse;
use crate::{TypeConversionError, missing_field};

use super::rerun_redap_tasks_v1alpha1::QueryTasksOnCompletionRequest;
use super::rerun_redap_tasks_v1alpha1::QueryTasksRequest;

impl QueryTasksResponse {
    pub fn dataframe_part(&self) -> Result<&DataframePart, TypeConversionError> {
        Ok(self
            .data
            .as_ref()
            .ok_or_else(|| missing_field!(QueryTasksResponse, "data"))?)
    }
}

/// [`Query`] is a query to retrieve the status of a task or tasks.
#[derive(Debug, Clone)]
pub enum Query {
    /// Retrieve the status of all tasks in the system.
    // TODO(andrea): this is dangerous when we have a multi-tenant system with lots of tasks
    All,
    /// Retrieve the status of a specific set of tasks.
    Filter(Vec<TaskId>),
}

impl Query {
    pub fn filter(&self) -> &[TaskId] {
        match self {
            Self::All => &[],
            Self::Filter(ids) => ids,
        }
    }
}

/// [`QueryResult`] is the result of a query for task status.
///
/// It maps each task id requested in a [`Query`]
/// to `Some(_: TaskState)` when the task is known to the system,
/// or to `None` if the task is not known.
#[derive(Debug)]
pub struct QueryResult {
    pub result: BTreeMap<TaskId, Option<TaskState>>,
}

#[derive(
    Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct TaskId {
    id: u64,
}

impl TaskId {
    const PREFIX: &'static str = "task_";

    pub fn from_components<H: std::hash::Hash, I: IntoIterator<Item = H>>(components: I) -> Self {
        let mut hasher = std::hash::DefaultHasher::new();
        for c in components {
            c.hash(&mut hasher);
        }
        let id = hasher.finish();

        Self { id }
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{:016x}", Self::PREFIX, self.id)
    }
}

impl std::fmt::Debug for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // use the Display implementation to format the id even when debugging
        std::fmt::Display::fmt(self, f)
    }
}

impl From<TaskId> for String {
    fn from(value: TaskId) -> Self {
        value.to_string()
    }
}

impl FromStr for TaskId {
    type Err = TaskIdParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = s
            .strip_prefix(Self::PREFIX)
            .ok_or_else(|| TaskIdParseError {
                msg: format!("missing {} prefix", Self::PREFIX),
            })?;
        if id.len() != 16 {
            return Err(TaskIdParseError {
                msg: format!("invalid length: {}", id.len()),
            });
        }
        u64::from_str_radix(id, 16)
            .map_err(|e| TaskIdParseError {
                msg: format!("cannot parse: {e:?}"),
            })
            .map(|id| Self { id })
    }
}

#[derive(thiserror::Error, Debug)]
#[error("invalid task id: {msg}")]
pub struct TaskIdParseError {
    msg: String,
}

/// The output of a task execution.
///
/// Both `msg` and `blob` can be empty
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskOutput {
    /// A short human readable message that describes the task outcome.
    pub msg: String,

    /// A blob of data that can be used to store additional information about the task outcome.
    /// (usually arrow encoded `RecordBatches`)
    pub blob: Vec<u8>,
}

impl std::fmt::Display for TaskOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[msg: {}, blob(len): {}]", self.msg, self.blob.len())
    }
}

impl TaskOutput {
    pub fn empty() -> Self {
        Self::new(String::new(), Vec::new())
    }

    pub fn msg<S: Into<String>>(msg: S) -> Self {
        Self::new(msg.into(), Vec::new())
    }

    pub fn new(msg: String, blob: Vec<u8>) -> Self {
        Self { msg, blob }
    }
}

impl TryFrom<RecordBatch> for TaskOutput {
    type Error = arrow::error::ArrowError;

    fn try_from(batch: RecordBatch) -> Result<Self, Self::Error> {
        let mut blob = Vec::new();
        let mut writer = arrow::ipc::writer::StreamWriter::try_new(&mut blob, &batch.schema())?;
        writer.write(&batch)?;
        writer.finish()?;
        Ok(Self::new(String::new(), blob))
    }
}

/// The error type of a task execution.
#[derive(thiserror::Error, Debug)]
pub enum TaskError {
    /// When the error is retriable, that task will be picked up by another worker
    /// and re-executed (limited by max number of retries).
    #[error("Retriable error: {0}")]
    Retriable(TaskOutput),

    /// When the error is not retriable, the task will be marked as failed and
    /// will not be retried.
    #[error("Terminal error: {0}")]
    Terminal(TaskOutput),
}

impl TaskError {
    pub fn terminal(msg: String, blob: Vec<u8>) -> Self {
        Self::Terminal(TaskOutput::new(msg, blob))
    }

    pub fn terminal_msg<S: Into<String>>(msg: S) -> Self {
        Self::Terminal(TaskOutput::msg(msg))
    }

    pub fn retriable(msg: String, blob: Vec<u8>) -> Self {
        Self::Retriable(TaskOutput::new(msg, blob))
    }

    pub fn retriable_msg<S: Into<String>>(msg: S) -> Self {
        Self::Retriable(TaskOutput::msg(msg))
    }
}

/// Possible execution statuses for a Task
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionStatus {
    /// The task is submitted but not completed yet
    Pending,

    /// The task has been completed with success.
    ///
    /// Optional any data
    /// about the completion is stored as a string.
    Success(TaskOutput),

    /// The task has been completed with failure.
    ///
    /// Any data about the failure, such as error messages,
    /// is stored as a string.
    Failure(TaskOutput),
}

impl ExecutionStatus {
    const PENDING_STR: &'static str = "pending";
    const SUCCESS_STR: &'static str = "success";
    const FAILURE_STR: &'static str = "failure";
}

impl std::fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "{}", Self::PENDING_STR),
            Self::Success(_) => write!(f, "{}", Self::SUCCESS_STR),
            Self::Failure(_) => write!(f, "{}", Self::FAILURE_STR),
        }
    }
}

#[derive(thiserror::Error, Debug)]
#[error("invalid task id: {msg}")]
pub struct ExecutionStatusError {
    msg: String,
}

impl TryFrom<Result<TaskOutput, TaskError>> for ExecutionStatus {
    type Error = ExecutionStatusError;
    fn try_from(outcome: Result<TaskOutput, TaskError>) -> Result<Self, Self::Error> {
        match outcome {
            Ok(data) => Ok(Self::Success(data)),
            Err(TaskError::Terminal(data)) => Ok(Self::Failure(data)),
            Err(TaskError::Retriable(..)) => Err(ExecutionStatusError {
                msg: "TaskError::Retriable is not valid for a completed task".to_owned(),
            }),
        }
    }
}

impl TryFrom<QueryTasksRequest> for Query {
    type Error = TaskIdParseError;

    fn try_from(value: QueryTasksRequest) -> Result<Self, Self::Error> {
        if value.ids.is_empty() {
            Ok(Self::All)
        } else {
            let task_ids = value
                .ids
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?;

            Ok(Self::Filter(task_ids))
        }
    }
}

impl From<Query> for QueryTasksRequest {
    fn from(value: Query) -> Self {
        match value {
            Query::All => Self { ids: Vec::new() },
            Query::Filter(task_ids) => Self {
                ids: task_ids
                    .into_iter()
                    .map(|id| ProtoTaskId { id: id.to_string() })
                    .collect(),
            },
        }
    }
}

impl TryFrom<QueryTasksOnCompletionRequest> for Query {
    type Error = TaskIdParseError;

    fn try_from(value: QueryTasksOnCompletionRequest) -> Result<Self, Self::Error> {
        if value.ids.is_empty() {
            Ok(Self::All)
        } else {
            let task_ids = value
                .ids
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?;

            Ok(Self::Filter(task_ids))
        }
    }
}

impl TryFrom<QueryResult> for RecordBatch {
    type Error = arrow::error::ArrowError;

    fn try_from(value: QueryResult) -> Result<Self, Self::Error> {
        let map = value.result;
        let schema = Arc::new(Schema::new(vec![
            Field::new("task_id", DataType::Utf8, false),
            Field::new("kind", DataType::Utf8, true),
            Field::new("data", DataType::Utf8, true),
            Field::new("exec_status", DataType::Utf8, false),
            Field::new("msgs", DataType::Utf8, true),
            Field::new("blob_len", DataType::UInt64, true),
            Field::new("lease_owner", DataType::Utf8, true),
            Field::new(
                "lease_expiration",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                true,
            ),
            Field::new("attempts", DataType::UInt8, false),
            Field::new(
                "creation_time",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                true,
            ),
            Field::new(
                "last_update_time",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                true,
            ),
        ]));

        let mut ids = Vec::new();
        let mut kinds = Vec::new();
        let mut data = Vec::new();
        let mut exec_status = Vec::new();
        let mut msgs = Vec::new();
        let mut blobs = Vec::new();
        let mut lease_owners = Vec::new();
        let mut lease_expirations = Vec::new();
        let mut retries = Vec::new();
        let mut creation_time = Vec::new();
        let mut update_time = Vec::new();

        for (task_id, task_status) in map {
            ids.push(task_id.to_string());
            if let Some(status) = task_status {
                kinds.push(Some(status.task_kind));
                data.push(Some(status.task_data));
                let ts = status.status.to_string();
                exec_status.push(ts);
                match status.status {
                    ExecutionStatus::Pending => {
                        msgs.push(None);
                        blobs.push(None);
                    }
                    ExecutionStatus::Success(TaskOutput { msg, blob })
                    | ExecutionStatus::Failure(TaskOutput { msg, blob }) => {
                        msgs.push(Some(msg));
                        blobs.push(Some(blob.len() as u64));
                    }
                };
                if let Some(lease) = status.lease {
                    lease_owners.push(Some(lease.owner));
                    let exp = lease
                        .lease_expiration
                        .as_nanosecond()
                        .try_into()
                        .map_err(|e| ArrowError::from_external_error(Box::new(e)))?;
                    lease_expirations.push(Some(exp));
                } else {
                    lease_owners.push(None);
                    lease_expirations.push(None);
                }
                retries.push(Some(status.retries));
                let creation: i64 = status
                    .creation_time
                    .as_nanosecond()
                    .try_into()
                    .map_err(|e| ArrowError::from_external_error(Box::new(e)))?;
                let update: i64 = status
                    .last_update_time
                    .as_nanosecond()
                    .try_into()
                    .map_err(|e| ArrowError::from_external_error(Box::new(e)))?;

                creation_time.push(Some(creation));
                update_time.push(Some(update));
            } else {
                kinds.push(None);
                data.push(None);
                exec_status.push("not found".to_owned());
                msgs.push(None);
                blobs.push(None);
                lease_owners.push(None);
                lease_expirations.push(None);
                retries.push(None);
                creation_time.push(None);
                update_time.push(None);
            }
        }

        let id_array = Arc::new(StringArray::from(ids));
        let kind_array = Arc::new(StringArray::from(kinds));
        let data_array = Arc::new(StringArray::from(data));
        let status_array = Arc::new(StringArray::from(exec_status));
        let msg_array = Arc::new(StringArray::from(msgs));
        let bloblens_array = Arc::new(UInt64Array::from(blobs));
        let owners_array = Arc::new(StringArray::from(lease_owners));
        let lease_exp_array = Arc::new(TimestampNanosecondArray::from(lease_expirations));
        let retries_array = Arc::new(UInt8Array::from(retries));
        let creation_time_array = Arc::new(TimestampNanosecondArray::from(creation_time));
        let update_time_array = Arc::new(TimestampNanosecondArray::from(update_time));

        Self::try_new(
            schema,
            vec![
                id_array,
                kind_array,
                data_array,
                status_array,
                msg_array,
                bloblens_array,
                owners_array,
                lease_exp_array,
                retries_array,
                creation_time_array,
                update_time_array,
            ],
        )
    }
}

impl From<TaskId> for ProtoTaskId {
    fn from(value: TaskId) -> Self {
        Self {
            id: value.to_string(),
        }
    }
}

impl TryFrom<ProtoTaskId> for TaskId {
    type Error = TaskIdParseError;

    fn try_from(value: ProtoTaskId) -> Result<Self, Self::Error> {
        Self::from_str(&value.id)
    }
}

/// [`Lease`] describes a lease on a task, which is made by the identifier of the worker that
/// owning the task, and the expiration time of the lease.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lease {
    pub owner: String,
    pub lease_expiration: Timestamp,
}

impl Lease {
    pub fn new<T: Into<String>>(owner: T, lease_expiration: Timestamp) -> Self {
        Self {
            owner: owner.into(),
            lease_expiration,
        }
    }
}

/// [`TaskState`] represent a deserialized view of the status of a task async
/// specified in the tasks table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskState {
    pub task_kind: String,
    pub task_data: String,
    pub status: ExecutionStatus,
    pub lease: Option<Lease>,
    pub retries: u8,
    pub creation_time: Timestamp,
    pub last_update_time: Timestamp,
}
