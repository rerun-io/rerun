use std::sync::Arc;

use arrow::{
    array::{
        ArrayRef as ArrowArrayRef, ListArray as ArrowListArray, RecordBatch as ArrowRecordBatch,
        RecordBatchOptions,
    },
    datatypes::{FieldRef as ArrowFieldRef, Schema as ArrowSchema},
};

use re_arrow_util::{ArrowArrayDowncastRef as _, into_arrow_ref};
use re_log::ResultExt as _;

/// Make sure all data columns are `ListArrays`.
#[tracing::instrument(level = "trace", skip_all)]
pub fn make_all_data_columns_list_arrays(
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
