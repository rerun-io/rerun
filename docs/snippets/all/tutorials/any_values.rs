//! Log arbitrary data.

use std::sync::Arc;

use rerun::external::arrow;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_any_values").spawn()?;

    let confidences = rerun::SerializedComponentBatch::new(
        Arc::new(arrow::array::Float64Array::from(vec![1.2, 3.4, 5.6])),
        rerun::ComponentDescriptor::new("confidence"),
    );

    let description = rerun::SerializedComponentBatch::new(
        Arc::new(arrow::array::StringArray::from(vec!["Bla bla bla…"])),
        rerun::ComponentDescriptor::new("description"),
    );

    rec.log("any_values", &[confidences, description])?;

    Ok(())
}
