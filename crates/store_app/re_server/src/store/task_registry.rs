use std::sync::Arc;

use ahash::HashMap;
use parking_lot::RwLock;
use re_protos::common::v1alpha1::TaskId;

/// A constant task ID used for all successful tasks.
pub const TASK_ID_SUCCESS: &str = "task_00000000DEADBEEF";

/// Result of a completed task.
#[derive(Clone, Debug)]
pub struct TaskResult {
    pub exec_status: String,
    pub msgs: String,
}

impl TaskResult {
    pub fn success() -> Self {
        Self {
            exec_status: "success".to_owned(),
            msgs: String::new(),
        }
    }

    pub fn failed(msg: impl Into<String>) -> Self {
        Self {
            exec_status: "failed".to_owned(),
            msgs: msg.into(),
        }
    }
}

/// In-memory registry for tracking task results.
///
/// Since this registry is not garbage collected, use [`TASK_ID_SUCCESS`] for successful entries and
/// do not register them ([`Self::get`] treats them as known and successful).
#[derive(Default, Clone)]
pub struct TaskRegistry {
    tasks: Arc<RwLock<HashMap<TaskId, TaskResult>>>,
}

impl TaskRegistry {
    /// Register a failed task with its error message.
    pub fn register_failure(&self, task_id: TaskId, result: TaskResult) {
        self.tasks.write().insert(task_id, result);
    }

    /// Get the result for a task. Returns None if not found.
    pub fn get(&self, task_id: &TaskId) -> Option<TaskResult> {
        if task_id.id.as_str() == TASK_ID_SUCCESS {
            Some(TaskResult::success())
        } else {
            self.tasks.read().get(task_id).cloned()
        }
    }
}
