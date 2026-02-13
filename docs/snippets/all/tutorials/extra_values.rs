//! Log extra values with a `Points2D`.

use std::sync::Arc;

use rerun::external::arrow;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_extra_values").spawn()?;

    let points = rerun::Points2D::new([(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)]);
    let confidences = rerun::AnyValues::default().with_component_from_data(
        "confidence",
        Arc::new(arrow::array::Float64Array::from(vec![0.3, 0.4, 0.5, 0.6])),
    );

    rec.log(
        "extra_values",
        &[&points as &dyn rerun::AsComponents, &confidences],
    )?;

    Ok(())
}
