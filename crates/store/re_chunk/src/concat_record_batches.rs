use arrow::array::RecordBatch;
use arrow::datatypes::Schema as ArrowSchema;

/// Concatenate multiple [`RecordBatch`]s into one.
// TODO: make a member of `RecordBatch` instead
pub fn concatenate_record_batches(
    schema: impl Into<ArrowSchema>,
    batches: &[RecordBatch],
) -> anyhow::Result<RecordBatch> {
    let schema: ArrowSchema = schema.into();
    anyhow::ensure!(
        batches
            .iter()
            .all(|batch| batch.schema_ref().as_ref() == &schema),
        "concatenate_record_batches: all batches must have the same schema"
    );

    // TODO: is_sorted is probably false now!

    let record_batch = arrow::compute::concat_batches(&schema.into(), batches)?;
    Ok(record_batch)
}
