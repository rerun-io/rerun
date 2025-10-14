//! Use `send_column` to send an entire column of custom data to Rerun.

#![expect(clippy::from_iter_instead_of_collect)]

use std::sync::Arc;

use rerun::{TimeColumn, external::arrow};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_any_batch_value_column_updates")
        .spawn()?;

    const STEPS: i64 = 64;

    let times = TimeColumn::new_sequence("step", 0..STEPS);

    let one_per_timestamp = rerun::SerializedComponentBatch::new(
        Arc::new(arrow::array::Float64Array::from_iter(
            (0..STEPS).map(|v| ((v as f64) / 10.0).sin()),
        )),
        rerun::ComponentDescriptor::partial("custom_component_single"),
    );

    let ten_per_timestamp = rerun::SerializedComponentBatch::new(
        Arc::new(arrow::array::Float64Array::from_iter((0..STEPS).flat_map(
            |_| (0..STEPS * 10).map(|v| ((v as f64) / 100.0).cos()),
        ))),
        rerun::ComponentDescriptor::partial("custom_component_multi"),
    );

    rec.send_columns(
        "/",
        [times],
        [
            one_per_timestamp.partitioned(std::iter::repeat_n(1, STEPS as _))?,
            ten_per_timestamp.partitioned(std::iter::repeat_n(10, STEPS as _))?,
        ],
    )?;

    Ok(())
}
