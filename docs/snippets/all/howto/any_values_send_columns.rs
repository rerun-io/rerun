//! Use `send_column` to send entire columns of custom data to Rerun.

#![allow(clippy::from_iter_instead_of_collect)]

use std::sync::Arc;

use rerun::{external::arrow, TimeColumn};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec =
        rerun::RecordingStreamBuilder::new("rerun_example_any_values_send_columns").spawn()?;

    const STEPS: i64 = 64;

    let times = TimeColumn::new_sequence("step", 0..STEPS);

    let sin = rerun::SerializedComponentBatch::new(
        Arc::new(arrow::array::Float64Array::from_iter(
            (0..STEPS).map(|v| ((v as f64) / 10.0).sin()),
        )),
        rerun::ComponentDescriptor::new("sin"),
    );

    let cos = rerun::SerializedComponentBatch::new(
        Arc::new(arrow::array::Float64Array::from_iter(
            (0..STEPS).map(|v| ((v as f64) / 10.0).cos()),
        )),
        rerun::ComponentDescriptor::new("cos"),
    );

    rec.send_columns_v2(
        "/",
        [times],
        [
            sin.partitioned(std::iter::repeat(1).take(STEPS as _))?,
            cos.partitioned(std::iter::repeat(1).take(STEPS as _))?,
        ],
    )?;

    Ok(())
}
