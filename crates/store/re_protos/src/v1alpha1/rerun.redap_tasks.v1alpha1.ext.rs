use arrow::datatypes::{DataType, Field, Schema, TimeUnit};

use crate::common::v1alpha1::DataframePart;
use crate::v1alpha1::rerun_redap_tasks_v1alpha1::QueryTasksResponse;
use crate::{TypeConversionError, missing_field};

// TODO(dataplatform#811): improve converter methods

impl QueryTasksResponse {
    pub const TASK_ID: &str = "task_id";
    pub const KIND: &str = "kind";
    pub const DATA: &str = "data";
    pub const EXEC_STATUS: &str = "exec_status";
    pub const MSGS: &str = "msgs";
    pub const BLOB_LEN: &str = "blob_len";
    pub const LEASE_OWNER: &str = "lease_owner";
    pub const LEASE_EXPIRATION: &str = "lease_expiration";
    pub const ATTEMPTS: &str = "attempts";
    pub const CREATION_TIME: &str = "creation_time";
    pub const LAST_UPDATE_TIME: &str = "last_update_time";

    pub fn dataframe_part(&self) -> Result<&DataframePart, TypeConversionError> {
        Ok(self
            .data
            .as_ref()
            .ok_or_else(|| missing_field!(QueryTasksResponse, "data"))?)
    }

    pub fn schema() -> arrow::datatypes::Schema {
        Schema::new(vec![
            Field::new(Self::TASK_ID, DataType::Utf8, false),
            Field::new(Self::KIND, DataType::Utf8, true),
            Field::new(Self::DATA, DataType::Utf8, true),
            Field::new(Self::EXEC_STATUS, DataType::Utf8, false),
            Field::new(Self::MSGS, DataType::Utf8, true),
            Field::new(Self::BLOB_LEN, DataType::UInt64, true),
            Field::new(Self::LEASE_OWNER, DataType::Utf8, true),
            Field::new(
                Self::LEASE_EXPIRATION,
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                true,
            ),
            Field::new(Self::ATTEMPTS, DataType::UInt8, false),
            Field::new(
                Self::CREATION_TIME,
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                true,
            ),
            Field::new(
                Self::LAST_UPDATE_TIME,
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                true,
            ),
        ])
    }
}
