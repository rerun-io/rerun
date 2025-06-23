use std::sync::Arc;

use arrow::{
    array::{
        Array as _, ArrayRef as ArrowArrayRef, ListArray as ArrowListArray,
        RecordBatch as ArrowRecordBatch, RecordBatchOptions,
    },
    datatypes::{FieldRef as ArrowFieldRef, Fields as ArrowFields, Schema as ArrowSchema},
    error::ArrowError,
};

use re_arrow_util::{ArrowArrayDowncastRef as _, into_arrow_ref};
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

        // Migrations to `0.23` below.
        let batch = if !crate::migrations::v0_24::matches_schema(batch) {
            let batch = make_all_data_columns_list_arrays(
                batch,
                crate::migrations::v0_23::is_component_column,
            );
            let batch = crate::migrations::v0_23::migrate_tuids(&batch);
            crate::migrations::v0_23::migrate_record_batch(&batch)
        } else {
            batch.clone()
        };

        // Migrations to `0.24` below.
        let batch = make_all_data_columns_list_arrays(
            &batch,
            crate::migrations::v0_24::is_component_column,
        );

        let sorbet_schema = SorbetSchema::try_from(batch.schema_ref().as_ref())?;

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

/// Make sure all data columns are `ListArrays`.
#[tracing::instrument(level = "trace", skip_all)]
fn make_all_data_columns_list_arrays(
    batch: &ArrowRecordBatch,
    is_component_column: impl Fn(&&ArrowFieldRef) -> bool,
) -> ArrowRecordBatch {
    re_tracing::profile_function!();

    let needs_migration = batch
        .schema_ref()
        .fields()
        .iter()
        .filter(|f| is_component_column(f))
        .any(|field| !matches!(field.data_type(), arrow::datatypes::DataType::List(_)));
    if !needs_migration {
        return batch.clone();
    }

    let num_columns = batch.num_columns();
    let mut fields: Vec<ArrowFieldRef> = Vec::with_capacity(num_columns);
    let mut columns: Vec<ArrowArrayRef> = Vec::with_capacity(num_columns);

    for (field, array) in itertools::izip!(batch.schema().fields(), batch.columns()) {
        let is_list_array = array.downcast_array_ref::<ArrowListArray>().is_some();
        let is_data_column = is_component_column(&field);
        if is_data_column && !is_list_array {
            let (field, array) = re_arrow_util::wrap_in_list_array(field, array.clone());
            fields.push(field.into());
            columns.push(into_arrow_ref(array));
        } else {
            fields.push(field.clone());
            columns.push(array.clone());
        }
    }

    let schema = Arc::new(ArrowSchema::new_with_metadata(
        fields,
        batch.schema().metadata.clone(),
    ));

    ArrowRecordBatch::try_new_with_options(
        schema.clone(),
        columns,
        &RecordBatchOptions::default().with_row_count(Some(batch.num_rows())),
    )
    .ok_or_log_error()
    .unwrap_or_else(|| ArrowRecordBatch::new_empty(schema))
}
