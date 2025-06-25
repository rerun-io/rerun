use std::sync::Arc;

use arrow::{
    array::{
        Array as _, ArrayRef as ArrowArrayRef, RecordBatch as ArrowRecordBatch, RecordBatchOptions,
    },
    datatypes::{Fields as ArrowFields, Schema as ArrowSchema},
    error::ArrowError,
};

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

        let batch = ArrowRecordBatch::try_new(
            std::sync::Arc::new(schema.to_arrow(batch_type)),
            arrow_columns,
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

    /// The heap size of this batch in bytes, if known.
    #[inline]
    pub fn heap_size_bytes(&self) -> Option<u64> {
        self.schema.heap_size_bytes
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
        re_format_arrow::format_record_batch_with_width(self, f.width()).fmt(f)
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
    #[tracing::instrument(level = "trace", skip_all)]
    pub fn try_from_record_batch(
        batch: &ArrowRecordBatch,
        batch_type: crate::BatchType,
    ) -> Result<Self, SorbetError> {
        re_tracing::profile_function!();

        // First migrate the incoming batch to the latest format:
        let batch = crate::migrations::migrate_record_batch(batch.clone());

        let sorbet_schema =
            SorbetSchema::try_from_migrated_arrow_schema(batch.schema_ref().as_ref())?;

        let _span = tracing::trace_span!("extend_metadata").entered();

        for (field, column) in itertools::izip!(
            sorbet_schema.columns.arrow_fields(batch_type),
            batch.columns()
        ) {
            debug_assert_eq!(field.data_type(), column.data_type());
        }

        // Extend with any metadata that might have been missing:
        let mut arrow_schema = ArrowSchema::clone(batch.schema_ref().as_ref());
        arrow_schema
            .metadata
            .extend(sorbet_schema.arrow_batch_metadata());

        let arrow_schema = Arc::new(arrow_schema);
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
