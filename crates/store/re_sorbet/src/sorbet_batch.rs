use std::sync::Arc;

use arrow::array::{
    Array as _, ArrayRef as ArrowArrayRef, RecordBatch as ArrowRecordBatch, RecordBatchOptions,
};
use arrow::datatypes::{Fields as ArrowFields, Schema as ArrowSchema};
use arrow::error::ArrowError;
use itertools::Itertools as _;
use re_log::ResultExt as _;

use crate::{
    ArrowBatchMetadata, ColumnDescriptor, ColumnDescriptorRef, ComponentColumnDescriptor,
    IndexColumnDescriptor, SorbetError, SorbetSchema,
};

/// Any rerun-compatible [`ArrowRecordBatch`].
///
/// This is a wrapper around a [`SorbetSchema`] and a [`ArrowRecordBatch`].
#[derive(Debug, Clone)]
pub struct SorbetBatch {
    schema: SorbetSchema,

    /// This record batch contains has all the meta-data
    /// required by a [`SorbetBatch`].
    ///
    /// It also has all non-Rerun metadata intact from wherever it was created from.
    batch: ArrowRecordBatch,
}

impl SorbetBatch {
    pub fn try_new(
        batch_type: crate::BatchType,
        schema: SorbetSchema,
        row_ids: Option<ArrowArrayRef>,
        index_arrays: Vec<ArrowArrayRef>,
        data_arrays: Vec<ArrowArrayRef>,
    ) -> Result<Self, ArrowError> {
        let arrow_columns = itertools::chain!(row_ids, index_arrays, data_arrays).collect();

        let batch = ArrowRecordBatch::try_new_with_options(
            std::sync::Arc::new(schema.to_arrow(batch_type)),
            arrow_columns,
            &RecordBatchOptions::default(),
        )?;

        Ok(Self { schema, batch })
    }

    /// Returns self but with all rows removed.
    #[must_use]
    pub fn drop_all_rows(self) -> Self {
        Self {
            schema: self.schema.clone(),
            batch: self.batch.slice(0, 0),
        }
    }
}

impl SorbetBatch {
    /// The parsed rerun schema of this batch.
    #[inline]
    pub fn sorbet_schema(&self) -> &SorbetSchema {
        &self.schema
    }

    #[inline]
    pub fn fields(&self) -> &ArrowFields {
        &self.schema_ref().fields
    }

    #[inline]
    pub fn arrow_batch_metadata(&self) -> &ArrowBatchMetadata {
        &self.batch.schema_ref().metadata
    }

    /// All the columns along with their descriptors.
    pub fn all_columns(&self) -> impl Iterator<Item = (&ColumnDescriptor, &ArrowArrayRef)> {
        itertools::izip!(self.schema.columns.iter(), self.batch.columns())
    }

    /// All the columns along with their descriptors.
    pub fn all_columns_ref(
        &self,
    ) -> impl Iterator<Item = (ColumnDescriptorRef<'_>, &ArrowArrayRef)> {
        itertools::izip!(
            self.schema.columns.iter().map(|x| x.into()),
            self.batch.columns()
        )
    }

    /// The columns of the indices (timelines).
    pub fn index_columns(&self) -> impl Iterator<Item = (&IndexColumnDescriptor, &ArrowArrayRef)> {
        itertools::izip!(self.schema.columns.iter(), self.batch.columns().iter()).filter_map(
            |(descr, array)| {
                if let ColumnDescriptor::Time(descr) = descr {
                    Some((descr, array))
                } else {
                    None
                }
            },
        )
    }

    /// The columns of the components.
    pub fn component_columns(
        &self,
    ) -> impl Iterator<Item = (&ComponentColumnDescriptor, &ArrowArrayRef)> {
        itertools::izip!(self.schema.columns.iter(), self.batch.columns().iter()).filter_map(
            |(descr, array)| {
                if let ColumnDescriptor::Component(descr) = descr {
                    Some((descr, array))
                } else {
                    None
                }
            },
        )
    }
}

impl std::fmt::Display for SorbetBatch {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        re_arrow_util::format_record_batch_with_width(self, f.width(), f.sign_minus()).fmt(f)
    }
}

impl AsRef<ArrowRecordBatch> for SorbetBatch {
    #[inline]
    fn as_ref(&self) -> &ArrowRecordBatch {
        &self.batch
    }
}

impl std::ops::Deref for SorbetBatch {
    type Target = ArrowRecordBatch;

    #[inline]
    fn deref(&self) -> &ArrowRecordBatch {
        &self.batch
    }
}

impl From<SorbetBatch> for ArrowRecordBatch {
    #[inline]
    fn from(batch: SorbetBatch) -> Self {
        batch.batch
    }
}

impl From<&SorbetBatch> for ArrowRecordBatch {
    #[inline]
    fn from(batch: &SorbetBatch) -> Self {
        batch.batch.clone()
    }
}

impl SorbetBatch {
    /// Will perform some transformations:
    /// * Will automatically wrap data columns in `ListArrays` if they are not already
    /// * Will migrate legacy data to more modern form
    ///
    /// Non-Rerun metadata will be preserved (both at batch-level and column-level).
    /// Rerun metadata will be updated and added to the batch if needed.
    #[tracing::instrument(level = "trace", skip_all)]
    pub fn try_from_record_batch(
        batch: &ArrowRecordBatch,
        batch_type: crate::BatchType,
    ) -> Result<Self, SorbetError> {
        re_tracing::profile_function!();

        // First migrate the incoming batch to the latest format:
        let batch = crate::migrations::migrate_record_batch(batch.clone(), batch_type);

        let sorbet_schema =
            SorbetSchema::try_from_migrated_arrow_schema(batch.schema_ref().as_ref())?;

        let _span = tracing::trace_span!("extend_metadata").entered();

        let new_fields = itertools::izip!(
            batch.schema_ref().fields(),
            sorbet_schema.columns.arrow_fields(batch_type),
            batch.columns()
        )
        .map(|(old_field, mut new_field, column)| {
            re_log::debug_assert_eq!(new_field.data_type(), column.data_type());

            let mut metadata = old_field.metadata().clone();
            metadata.extend(new_field.metadata().clone()); // overwrite old with new
            new_field.set_metadata(metadata);

            Arc::new(new_field)
        })
        .collect_vec();

        let mut batch_metadata = batch.schema_ref().metadata.clone();
        batch_metadata.extend(sorbet_schema.arrow_batch_metadata()); // overwrite old with new

        let arrow_schema = Arc::new(ArrowSchema::new_with_metadata(new_fields, batch_metadata));

        let batch = ArrowRecordBatch::try_new_with_options(
            arrow_schema.clone(),
            batch.columns().to_vec(),
            &RecordBatchOptions::default().with_row_count(Some(batch.num_rows())),
        )
        .ok_or_log_error()
        .unwrap_or_else(|| ArrowRecordBatch::new_empty(arrow_schema));

        Ok(Self {
            schema: sorbet_schema,
            batch,
        })
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{RowIdColumnDescriptor, sorbet_batch};

    /// Test that user-provided metadata is preserved when converting to and from a [`SorbetBatch`].
    ///
    /// Also test that we add the proper Rerun metadata, and remove old Rerun metadata that is not relevant anymore.
    #[test]
    fn test_sorbet_batch_metadata() {
        let original: ArrowRecordBatch = {
            let mut row_id_field = RowIdColumnDescriptor::from_sorted(false).to_arrow_field();
            row_id_field
                .metadata_mut()
                .remove("ARROW:extension:metadata");
            row_id_field.metadata_mut().insert(
                "custom_column_key".to_owned(),
                "custom_column_value".to_owned(),
            );
            let fields = vec![Arc::new(row_id_field)];
            let arrow_schema = ArrowSchema::new_with_metadata(
                fields,
                [
                    (
                        "rerun.id".to_owned(),
                        re_types_core::ChunkId::new().to_string(),
                    ),
                    (
                        "custom_batch_key".to_owned(),
                        "custom_batch_value".to_owned(),
                    ),
                ]
                .into_iter()
                .collect(),
            );
            ArrowRecordBatch::new_empty(arrow_schema.into())
        };

        {
            // Check original has what we expect:
            assert!(original.schema().metadata().contains_key("rerun.id"));
            assert!(
                original
                    .schema()
                    .metadata()
                    .contains_key("custom_batch_key")
            );
            let row_id = original.schema_ref().field(0);
            assert!(
                !row_id.metadata().contains_key("ARROW:extension:metadata"),
                "We intentionally omitted this from the original"
            );
        }

        let sorbet_batch = sorbet_batch::SorbetBatch::try_from_record_batch(
            &original,
            crate::BatchType::Dataframe,
        )
        .unwrap();

        let ret = ArrowRecordBatch::from(sorbet_batch);

        assert!(
            !ret.schema().metadata().contains_key("rerun.id"),
            "This should have been removed/renamed"
        );
        assert!(
            ret.schema().metadata().contains_key("rerun:id"),
            "This should have been added/renamed"
        );
        assert!(
            ret.schema().metadata().contains_key("custom_batch_key"),
            "This should remain"
        );
        assert!(
            ret.schema().metadata().contains_key("sorbet:version"),
            "This should have been added"
        );

        // Check field:
        let row_id = ret.schema_ref().field(0);
        assert!(
            row_id.metadata().contains_key("custom_column_key"),
            "This should remain"
        );
        assert!(
            row_id.metadata().contains_key("ARROW:extension:metadata"),
            "This should have been added"
        );
    }
}
