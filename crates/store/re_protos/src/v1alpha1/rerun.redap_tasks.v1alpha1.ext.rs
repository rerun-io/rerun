use crate::common::v1alpha1::DataframePart;
use crate::v1alpha1::rerun_redap_tasks_v1alpha1::QueryTasksResponse;
use crate::{missing_field, TypeConversionError};

impl QueryTasksResponse {
    pub fn dataframe_part(&self) -> Result<&DataframePart, TypeConversionError> {
        Ok(self
            .data
            .as_ref()
            .ok_or_else(|| missing_field!(QueryTasksResponse, "data"))?)
    }
}
