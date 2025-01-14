use arrow::{
    array::{make_array, RecordBatch},
    datatypes::{Field, Schema},
    error::ArrowError,
};

use crate::TransportChunk;

impl TransportChunk {
    /// Create an arrow-rs [`RecordBatch`] containing the data from this [`TransportChunk`].
    ///
    /// This is a "fairly" cheap operation, as it does not copy the underlying arrow data,
    /// but does incur overhead of generating an alternative representation of the arrow-
    /// related rust structures that refer to those data buffers.
    pub fn try_to_arrow_record_batch(&self) -> Result<RecordBatch, ArrowError> {
        let fields: Vec<Field> = self
            .schema
            .fields
            .iter()
            .map(|f| f.clone().into())
            .collect();

        let metadata = self.schema.metadata.clone().into_iter().collect();

        let schema = Schema::new_with_metadata(fields, metadata);

        let columns: Vec<_> = self
            .all_columns()
            .map(|(_field, arr2_array)| {
                let data = arrow2::array::to_data(arr2_array.as_ref());
                make_array(data)
            })
            .collect();

        RecordBatch::try_new(std::sync::Arc::new(schema), columns)
    }

    /// Create a [`TransportChunk`] from an arrow-rs [`RecordBatch`].
    ///
    /// This is a "fairly" cheap operation, as it does not copy the underlying arrow data,
    /// but does incur overhead of generating an alternative representation of the arrow-
    /// related rust structures that refer to those data buffers.
    pub fn from_arrow_record_batch(batch: &RecordBatch) -> Self {
        let fields: Vec<arrow2::datatypes::Field> = batch
            .schema()
            .fields
            .iter()
            .map(|f| f.clone().into())
            .collect();

        let metadata = batch.schema().metadata.clone().into_iter().collect();

        let schema = arrow2::datatypes::Schema::from(fields).with_metadata(metadata);

        let columns: Vec<_> = batch
            .columns()
            .iter()
            .map(|array| arrow2::array::from_data(&array.to_data()))
            .collect();

        let data = arrow2::chunk::Chunk::new(columns);

        Self::new(schema, data)
    }
}
