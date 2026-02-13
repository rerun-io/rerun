use arrow::array::{RecordBatch, RecordBatchOptions, new_null_array};
use arrow::datatypes::{DataType, Schema};
use arrow::error::ArrowError;
use std::sync::Arc;

#[tracing::instrument(level = "info", skip_all)]
pub fn align_record_batch_to_schema(
    batch: &RecordBatch,
    target_schema: &Arc<Schema>,
) -> Result<RecordBatch, ArrowError> {
    let num_rows = batch.num_rows();

    let mut aligned_columns = Vec::with_capacity(target_schema.fields().len());

    for field in target_schema.fields() {
        if let Some((idx, _)) = batch.schema().column_with_name(field.name()) {
            let batch_data_type = batch.column(idx).data_type();
            if batch_data_type == &DataType::Null && field.data_type() != &DataType::Null {
                // Chunk store may output a null array of null data type
                aligned_columns.push(new_null_array(field.data_type(), num_rows));
            } else {
                aligned_columns.push(batch.column(idx).clone());
            }
        } else {
            // Fill with nulls of the right data type
            aligned_columns.push(new_null_array(field.data_type(), num_rows));
        }
    }

    RecordBatch::try_new_with_options(
        target_schema.clone(),
        aligned_columns,
        &RecordBatchOptions::new().with_row_count(Some(num_rows)),
    )
}
