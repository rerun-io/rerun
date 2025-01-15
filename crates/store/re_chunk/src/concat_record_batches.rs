use crate::TransportChunk;

use arrow::datatypes::Schema as ArrowSchema;
use arrow2::chunk::Chunk as Arrow2Chunk;

/// Concatenate multiple [`TransportChunk`]s into one.
///
/// This is a temporary method that we use while waiting to migrate towards `arrow-rs`.
/// * `arrow2` doesn't have a `RecordBatch` type, therefore we emulate that using our `TransportChunk`s.
/// * `arrow-rs` does have one, and it natively supports concatenation.
pub fn concatenate_record_batches(
    schema: impl Into<ArrowSchema>,
    batches: &[TransportChunk],
) -> anyhow::Result<TransportChunk> {
    let schema: ArrowSchema = schema.into();
    anyhow::ensure!(
        batches
            .iter()
            .all(|batch| batch.schema_ref().as_ref() == &schema),
        "concatenate_record_batches: all batches must have the same schema"
    );

    let mut output_columns = Vec::new();

    if !batches.is_empty() {
        for (i, _field) in schema.fields.iter().enumerate() {
            let arrays: Option<Vec<_>> = batches.iter().map(|batch| batch.column(i)).collect();
            let arrays = arrays.ok_or_else(|| {
                anyhow::anyhow!("concatenate_record_batches: all batches must have the same schema")
            })?;
            let array = re_arrow_util::arrow2_util::concat_arrays(&arrays)?;
            output_columns.push(array);
        }
    }

    Ok(TransportChunk::new(
        schema,
        Arrow2Chunk::new(output_columns),
    ))
}
