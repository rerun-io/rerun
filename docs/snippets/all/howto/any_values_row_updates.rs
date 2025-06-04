//! Update custom user-defined values over time.
//!
//! See also the `any_values_column_updates` example, which achieves the same thing in a single operation.

#![allow(clippy::from_iter_instead_of_collect)]

use std::sync::Arc;

use rerun::external::arrow;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_any_values_row_updates").spawn()?;

    for step in 0..64 {
        let sin = rerun::SerializedComponentBatch::new(
            Arc::new(arrow::array::Float64Array::from_iter(
                [((step as f64) / 10.0).sin()], //
            )),
            rerun::ComponentDescriptor::partial("sin"),
        );

        let cos = rerun::SerializedComponentBatch::new(
            Arc::new(arrow::array::Float64Array::from_iter(
                [((step as f64) / 10.0).cos()], //
            )),
            rerun::ComponentDescriptor::partial("cos"),
        );

        rec.set_time_sequence("step", step);
        rec.log("/", &[sin, cos])?;
    }

    Ok(())
}
