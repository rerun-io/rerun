#![expect(clippy::unwrap_used)]

use std::sync::Arc;

use arrow::{
    array::{Array, ArrayRef, RecordBatch, RecordBatchOptions},
    datatypes::{Field, Schema},
};

/// Helper function to wrap an [`ArrayRef`] into a [`RecordBatch`] for easier printing.
pub fn wrap_in_record_batch(array: ArrayRef) -> RecordBatch {
    let schema = Arc::new(Schema::new_with_metadata(
        vec![Field::new("col", array.data_type().clone(), true)],
        Default::default(),
    ));
    RecordBatch::try_new_with_options(schema, vec![array], &RecordBatchOptions::default()).unwrap()
}

pub struct DisplayRB<T: Array + Clone + 'static>(pub T);

impl<T: Array + Clone + 'static> std::fmt::Display for DisplayRB<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let rb = wrap_in_record_batch(Arc::new(self.0.clone()));
        write!(f, "{}", re_arrow_util::format_record_batch(&rb))
    }
}
