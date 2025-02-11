use std::sync::Arc;

use arrow::{
    array::{
        Array as ArrowArray, ArrayRef as ArrowArrayRef, AsArray, ListArray as ArrowListArray,
        RecordBatch as ArrowRecordBatch, RecordBatchOptions, StructArray as ArrowStructArray,
    },
    datatypes::{
        DataType as ArrowDataType, Field as ArrowField, FieldRef as ArrowFieldRef,
        Fields as ArrowFields, Schema as ArrowSchema,
    },
};

use re_arrow_util::{arrow_util::into_arrow_ref, ArrowArrayDowncastRef};
use re_log_types::EntityPath;
use re_types_core::ChunkId;

use crate::{
    chunk_schema::InvalidChunkSchema, ArrowBatchMetadata, ChunkSchema, ComponentColumnDescriptor,
    RowIdColumnDescriptor, TimeColumnDescriptor, WrongDatatypeError,
};

#[derive(thiserror::Error, Debug)]
pub enum MismatchedChunkSchemaError {
    #[error("{0}")]
    Custom(String),

    #[error(transparent)]
    WrongDatatypeError(#[from] WrongDatatypeError),
}

impl MismatchedChunkSchemaError {
    pub fn custom(s: impl Into<String>) -> Self {
        Self::Custom(s.into())
    }
}

/// The [`ArrowRecordBatch`] representation of a Rerun chunk.
///
/// This is a wrapper around a [`ChunkSchema`] and a [`ArrowRecordBatch`].
///
/// Each [`ChunkBatch`] contains logging data for a single [`EntityPath`].
/// It always has a [`re_types_core::RowId`] column.
#[derive(Debug, Clone)]
pub struct ChunkBatch {
    schema: ChunkSchema,
    batch: ArrowRecordBatch,
}

impl ChunkBatch {
    pub fn try_new(
        schema: ChunkSchema,
        row_ids: ArrowArrayRef,
        index_arrays: Vec<ArrowArrayRef>,
        data_arrays: Vec<ArrowArrayRef>,
    ) -> Result<Self, MismatchedChunkSchemaError> {
        let row_count = row_ids.len();

        WrongDatatypeError::compare_expected_actual(
            &schema.row_id_column.datatype(),
            row_ids.data_type(),
        )?;

        if index_arrays.len() != schema.index_columns.len() {
            return Err(MismatchedChunkSchemaError::custom(format!(
                "Schema had {} index columns, but got {}",
                schema.index_columns.len(),
                index_arrays.len()
            )));
        }
        for (schema, array) in itertools::izip!(&schema.index_columns, &index_arrays) {
            WrongDatatypeError::compare_expected_actual(schema.datatype(), array.data_type())?;
            if array.len() != row_count {
                return Err(MismatchedChunkSchemaError::custom(format!(
                    "Index column {:?} had {} rows, but we got {} row IDs",
                    schema.name(),
                    array.len(),
                    row_count
                )));
            }
        }

        if data_arrays.len() != schema.data_columns.len() {
            return Err(MismatchedChunkSchemaError::custom(format!(
                "Schema had {} data columns, but got {}",
                schema.data_columns.len(),
                data_arrays.len()
            )));
        }
        for (schema, array) in itertools::izip!(&schema.data_columns, &data_arrays) {
            WrongDatatypeError::compare_expected_actual(&schema.store_datatype, array.data_type())?;
            if array.len() != row_count {
                return Err(MismatchedChunkSchemaError::custom(format!(
                    "Data column {:?} had {} rows, but we got {} row IDs",
                    schema.column_name(crate::BatchType::Chunk),
                    array.len(),
                    row_count
                )));
            }
        }

        let arrow_columns = itertools::chain!(Some(row_ids), index_arrays, data_arrays).collect();

        let batch = ArrowRecordBatch::try_new_with_options(
            std::sync::Arc::new(ArrowSchema::from(&schema)),
            arrow_columns,
            &RecordBatchOptions::default().with_row_count(Some(row_count)),
        )
        .map_err(|err| {
            MismatchedChunkSchemaError::custom(format!(
                "Failed to create arrow record batch: {err}"
            ))
        })?;

        Ok(Self { schema, batch })
    }
}

impl ChunkBatch {
    /// The parsed rerun schema of this chunk.
    #[inline]
    pub fn chunk_schema(&self) -> &ChunkSchema {
        &self.schema
    }

    /// The globally unique ID of this chunk.
    #[inline]
    pub fn chunk_id(&self) -> ChunkId {
        self.schema.chunk_id()
    }

    /// Which entity is this chunk for?
    #[inline]
    pub fn entity_path(&self) -> &EntityPath {
        self.schema.entity_path()
    }

    /// The heap size of this chunk in bytes, if known.
    #[inline]
    pub fn heap_size_bytes(&self) -> Option<u64> {
        self.schema.heap_size_bytes()
    }

    /// Are we sorted by the row id column?
    #[inline]
    pub fn is_sorted(&self) -> bool {
        self.schema.is_sorted()
    }

    #[inline]
    pub fn fields(&self) -> &ArrowFields {
        &self.schema_ref().fields
    }

    #[inline]
    pub fn arrow_bacth_metadata(&self) -> &ArrowBatchMetadata {
        &self.batch.schema_ref().metadata
    }

    pub fn row_id_column(&self) -> (&RowIdColumnDescriptor, &ArrowStructArray) {
        // The first column is always the row IDs.
        (
            &self.schema.row_id_column,
            self.batch.columns()[0]
                .as_struct_opt()
                .expect("Row IDs should be encoded as struct"),
        )
    }

    /// The columns of the indices (timelines).
    pub fn index_columns(&self) -> impl Iterator<Item = (&TimeColumnDescriptor, &ArrowArrayRef)> {
        itertools::izip!(
            &self.schema.index_columns,
            self.batch.columns().iter().skip(1) // skip row IDs
        )
    }

    /// The columns of the indices (timelines).
    pub fn data_columns(
        &self,
    ) -> impl Iterator<Item = (&ComponentColumnDescriptor, &ArrowArrayRef)> {
        itertools::izip!(
            &self.schema.data_columns,
            self.batch
                .columns()
                .iter()
                .skip(1 + self.schema.index_columns.len()) // skip row IDs and indices
        )
    }
}

impl std::fmt::Display for ChunkBatch {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        re_format_arrow::format_record_batch_with_width(self, f.width()).fmt(f)
    }
}

impl AsRef<ArrowRecordBatch> for ChunkBatch {
    #[inline]
    fn as_ref(&self) -> &ArrowRecordBatch {
        &self.batch
    }
}

impl std::ops::Deref for ChunkBatch {
    type Target = ArrowRecordBatch;

    #[inline]
    fn deref(&self) -> &ArrowRecordBatch {
        &self.batch
    }
}

impl From<ChunkBatch> for ArrowRecordBatch {
    #[inline]
    fn from(chunk: ChunkBatch) -> Self {
        chunk.batch
    }
}

impl From<&ChunkBatch> for ArrowRecordBatch {
    #[inline]
    fn from(chunk: &ChunkBatch) -> Self {
        chunk.batch.clone()
    }
}

impl TryFrom<&ArrowRecordBatch> for ChunkBatch {
    type Error = InvalidChunkSchema;

    /// Will automatically wrap data columns in `ListArrays` if they are not already.
    fn try_from(batch: &ArrowRecordBatch) -> Result<Self, Self::Error> {
        re_tracing::profile_function!();

        let batch = make_all_data_columns_list_arrays(batch);

        let chunk_schema = ChunkSchema::try_from(batch.schema_ref().as_ref())?;

        for (field, column) in itertools::izip!(chunk_schema.arrow_fields(), batch.columns()) {
            debug_assert_eq!(field.data_type(), column.data_type());
        }

        // Extend with any metadata that might have been missing:
        let mut arrow_schema = ArrowSchema::clone(batch.schema_ref().as_ref());
        arrow_schema
            .metadata
            .extend(chunk_schema.arrow_batch_metadata());

        let batch = ArrowRecordBatch::try_new_with_options(
            arrow_schema.into(),
            batch.columns().to_vec(),
            &RecordBatchOptions::default().with_row_count(Some(batch.num_rows())),
        )
        .expect("Can't fail");

        Ok(Self {
            schema: chunk_schema,
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
        let is_data_column = field
            .metadata()
            .get("rerun.kind")
            .is_some_and(|kind| kind == "data");
        if is_data_column && !is_list_array {
            let (field, array) = wrap_in_list_array(field, array);
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

// TODO(cmc): we can do something faster/simpler here; see https://github.com/rerun-io/rerun/pull/8945#discussion_r1950689060
fn wrap_in_list_array(field: &ArrowField, data: &dyn ArrowArray) -> (ArrowField, ArrowListArray) {
    re_tracing::profile_function!();

    // We slice each column array into individual arrays and then convert the whole lot into a ListArray

    let data_field_inner =
        ArrowField::new("item", field.data_type().clone(), true /* nullable */);

    let data_field = ArrowField::new(
        field.name().clone(),
        ArrowDataType::List(Arc::new(data_field_inner.clone())),
        false, /* not nullable */
    )
    .with_metadata(field.metadata().clone());

    let mut sliced: Vec<ArrowArrayRef> = Vec::new();
    for idx in 0..data.len() {
        sliced.push(data.slice(idx, 1));
    }

    let data_arrays = sliced.iter().map(|e| Some(e.as_ref())).collect::<Vec<_>>();
    #[allow(clippy::unwrap_used)] // we know we've given the right field type
    let list_array: ArrowListArray = re_arrow_util::arrow_util::arrays_to_list_array(
        data_field_inner.data_type().clone(),
        &data_arrays,
    )
    .unwrap();
    (data_field, list_array)
}
