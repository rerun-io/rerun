use arrow::{
    array::{
        Array as _, ArrayRef as ArrowArrayRef, AsArray as _, ListArray as ArrowListArray,
        RecordBatch as ArrowRecordBatch, RecordBatchOptions, StructArray as ArrowStructArray,
    },
    datatypes::{FieldRef as ArrowFieldRef, Fields as ArrowFields, Schema as ArrowSchema},
    error::ArrowError,
};

use re_arrow_util::{into_arrow_ref, ArrowArrayDowncastRef as _};

use crate::{
    ArrowBatchMetadata, ColumnDescriptorRef, ColumnKind, ComponentColumnDescriptor,
    IndexColumnDescriptor, RowIdColumnDescriptor, SorbetError, SorbetSchema,
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

    /// The `RowId` column, if any.
    pub fn row_id_column(&self) -> Option<(&RowIdColumnDescriptor, &ArrowStructArray)> {
        self.schema.columns.row_id.as_ref().map(|row_id_desc| {
            (
                row_id_desc,
                self.batch.columns()[0]
                    .as_struct_opt()
                    .expect("Row IDs should be encoded as struct"),
            )
        })
    }

    /// All the columns along with their descriptors.
    pub fn all_columns(&self) -> impl Iterator<Item = (ColumnDescriptorRef<'_>, &ArrowArrayRef)> {
        self.schema.columns.descriptors().zip(self.batch.columns())
    }

    /// The columns of the indices (timelines).
    pub fn index_columns(&self) -> impl Iterator<Item = (&IndexColumnDescriptor, &ArrowArrayRef)> {
        let num_row_id_columns = self.schema.columns.row_id.is_some() as usize;
        itertools::izip!(
            &self.schema.columns.indices,
            self.batch.columns().iter().skip(num_row_id_columns)
        )
    }

    /// The columns of the components.
    pub fn component_columns(
        &self,
    ) -> impl Iterator<Item = (&ComponentColumnDescriptor, &ArrowArrayRef)> {
        let num_row_id_columns = self.schema.columns.row_id.is_some() as usize;
        let num_index_columns = self.schema.columns.indices.len();
        itertools::izip!(
            &self.schema.columns.components,
            self.batch
                .columns()
                .iter()
                .skip(num_row_id_columns + num_index_columns)
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
    /// Will automatically wrap data columns in `ListArrays` if they are not already.
    ///
    /// Will also migrate old types to new types.
    pub fn try_from_record_batch(
        batch: &ArrowRecordBatch,
        batch_type: crate::BatchType,
    ) -> Result<Self, SorbetError> {
        re_tracing::profile_function!();

        let batch = make_all_data_columns_list_arrays(batch);
        let batch = crate::migration::reorder_columns(&batch);
        let batch = crate::migration::migrate_tuids(&batch);
        let batch = crate::migration::migrate_record_batch(&batch);

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

        let batch = ArrowRecordBatch::try_new_with_options(
            arrow_schema.into(),
            batch.columns().to_vec(),
            &RecordBatchOptions::default().with_row_count(Some(batch.num_rows())),
        )
        .expect("Can't fail");

        Ok(Self {
            schema: sorbet_schema,
            batch,
        })
    }
}

/// Make sure all data columns are `ListArrays`.
fn make_all_data_columns_list_arrays(batch: &ArrowRecordBatch) -> ArrowRecordBatch {
    re_tracing::profile_function!();

    let num_columns = batch.num_columns();
    let mut fields: Vec<ArrowFieldRef> = Vec::with_capacity(num_columns);
    let mut columns: Vec<ArrowArrayRef> = Vec::with_capacity(num_columns);

    for (field, array) in itertools::izip!(batch.schema().fields(), batch.columns()) {
        let is_list_array = array.downcast_array_ref::<ArrowListArray>().is_some();
        let is_data_column =
            ColumnKind::try_from(field.as_ref()).is_ok_and(|kind| kind == ColumnKind::Component);
        if is_data_column && !is_list_array {
            let (field, array) = re_arrow_util::wrap_in_list_array(field, array.clone());
            fields.push(field.into());
            columns.push(into_arrow_ref(array));
        } else {
            fields.push(field.clone());
            columns.push(array.clone());
        }
    }

    let schema = ArrowSchema::new_with_metadata(fields, batch.schema().metadata.clone());

    ArrowRecordBatch::try_new_with_options(
        schema.into(),
        columns,
        &RecordBatchOptions::default().with_row_count(Some(batch.num_rows())),
    )
    .expect("Can't fail")
}
