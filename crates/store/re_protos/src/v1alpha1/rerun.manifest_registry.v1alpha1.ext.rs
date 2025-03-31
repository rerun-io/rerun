use std::sync::Arc;

use arrow::{
    array::{ArrayRef, RecordBatch, StringArray, TimestampMillisecondArray},
    datatypes::{DataType, Field, Schema, TimeUnit},
};

use crate::manifest_registry::v1alpha1::CreatePartitionManifestsResponse;

// --- CreatePartitionManifestsResponse ---

impl CreatePartitionManifestsResponse {
    pub const FIELD_ID: &str = "id";
    pub const FIELD_UPDATED_AT: &str = "updated_at";
    pub const FIELD_URL: &str = "url";
    pub const FIELD_ERROR: &str = "error";

    /// The Arrow schema of the dataframe in [`Self::data`].
    pub fn schema() -> Schema {
        Schema::new(vec![
            Field::new(Self::FIELD_ID, DataType::Utf8, false),
            Field::new(
                Self::FIELD_UPDATED_AT,
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                true,
            ),
            Field::new(Self::FIELD_URL, DataType::Utf8, true),
            Field::new(Self::FIELD_ERROR, DataType::Utf8, true),
        ])
    }

    /// Helper to simplify instantiation of the dataframe in [`Self::data`].
    pub fn create_dataframe(
        partition_ids: Vec<String>,
        updated_ats: Vec<Option<jiff::Timestamp>>,
        partition_manifest_urls: Vec<Option<String>>,
        errors: Vec<Option<String>>,
    ) -> arrow::error::Result<RecordBatch> {
        let updated_ats = updated_ats
            .into_iter()
            .map(|ts| ts.map(|ts| ts.as_nanosecond() as i64)) // ~300 years should be fine
            .collect::<Vec<_>>();

        let schema = Arc::new(Self::schema());
        let columns: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(partition_ids)),
            Arc::new(TimestampMillisecondArray::from(updated_ats)),
            Arc::new(StringArray::from(partition_manifest_urls)),
            Arc::new(StringArray::from(errors)),
        ];

        RecordBatch::try_new(schema, columns)
    }
}

// TODO(#9430): I'd love if I could do this, but this creates a nasty circular dep with `re_log_encoding`.
#[cfg(all(unix, windows))] // always statically false
impl TryFrom<RecordBatch> for CreatePartitionManifestsResponse {
    type Error = tonic::Status;

    fn try_from(batch: RecordBatch) -> Result<Self, Self::Error> {
        if !Self::schema().contains(batch.schema()) {
            let typ = std::any::type_name::<Self>();
            return Err(tonic::Status::internal(format!(
                "invalid schema for {typ}: expected {:?} but got {:?}",
                Self::schema(),
                batch.schema(),
            )));
        }

        use re_log_encoding::codec::wire::encoder::Encode as _;
        batch
            .encode()
            .map(|data| Self { data: Some(data) })
            .map_err(|err| tonic::Status::internal(format!("failed to encode chunk: {err}")))?;
    }
}

// TODO(cmc): the other way around would be nice too, but same problem.
