use arrow::{
    array::{make_array, RecordBatch},
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
        let columns: Vec<_> = self
            .columns()
            .iter()
            .map(|arr2_array| make_array(arrow2::array::to_data(*arr2_array)))
            .collect();

        RecordBatch::try_new(self.schema(), columns)
    }

    /// Create a [`TransportChunk`] from an arrow-rs [`RecordBatch`].
    ///
    /// This is a "fairly" cheap operation, as it does not copy the underlying arrow data,
    /// but does incur overhead of generating an alternative representation of the arrow-
    /// related rust structures that refer to those data buffers.
    pub fn from_arrow_record_batch(batch: &RecordBatch) -> Self {
        let columns: Vec<_> = batch
            .columns()
            .iter()
            .map(|array| arrow2::array::from_data(&array.to_data()))
            .collect();

        let data = arrow2::chunk::Chunk::new(columns);

        Self::new(batch.schema(), data)
    }
}
