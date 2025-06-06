//! Log arbitrary data.

use std::sync::Arc;

use rerun::external::arrow;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_any_values").spawn()?;

    let confidences = rerun::SerializedComponentBatch::new(
        Arc::new(arrow::array::Float64Array::from(vec![1.2, 3.4, 5.6])),
        rerun::ComponentDescriptor::partial("confidence"),
    );

    let description = rerun::SerializedComponentBatch::new(
        Arc::new(arrow::array::StringArray::from(vec!["Bla bla bla…"])),
        rerun::ComponentDescriptor::partial("description"),
    );

    // URIs will become clickable links
    let homepage = rerun::SerializedComponentBatch::new(
        Arc::new(arrow::array::StringArray::from(vec![
            "https://www.rerun.io",
        ])),
        rerun::ComponentDescriptor::partial("homepage"),
    );

    let repository = rerun::SerializedComponentBatch::new(
        Arc::new(arrow::array::StringArray::from(vec![
            "https://github.com/rerun-io/rerun",
        ])),
        rerun::ComponentDescriptor::partial("repository"),
    );

    rec.log(
        "any_values",
        &[confidences, description, homepage, repository],
    )?;

    Ok(())
}
