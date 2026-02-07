//! Update custom user-defined values over time.
//!
//! See also the `any_values_column_updates` example, which achieves the same thing in a single operation.

use std::sync::Arc;

use rerun::external::arrow;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_any_values_row_updates").spawn()?;

    for step in 0..64 {
        let sin_cos = rerun::AnyValues::default()
            .with_component_from_data(
                "sin",
                Arc::new(arrow::array::Float64Array::from_iter(
                    [((step as f64) / 10.0).sin()], //
                )),
            )
            .with_component_from_data(
                "cos",
                Arc::new(arrow::array::Float64Array::from_iter(
                    [((step as f64) / 10.0).cos()], //
                )),
            );

        rec.set_time_sequence("step", step);
        rec.log("/", &sin_cos)?;
    }

    Ok(())
}
