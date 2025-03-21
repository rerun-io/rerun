//! Sets the recording properties.

use std::sync::Arc;

use rerun::external::arrow;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_recording_properties").spawn()?;

    // Recordings can have an optional name.
    rec.send_recording_name("My recording")?;

    // Start time is set automatically, but we can overwrite it at any time.
    rec.send_recording_start_time(1742539110661000000)?;

    // Adds a user-defined property to the recording.
    rec.send_property(
        "camera_left",
        &rerun::archetypes::Points3D::new([[1.0, 0.1, 1.0]]),
    )?;

    let confidences = rerun::SerializedComponentBatch::new(
        Arc::new(arrow::array::Float64Array::from(vec![0.3, 0.4, 0.5, 0.6])),
        rerun::ComponentDescriptor::new("confidences"),
    );

    let traffic = rerun::SerializedComponentBatch::new(
        Arc::new(arrow::array::StringArray::from(vec!["low"])),
        rerun::ComponentDescriptor::new("traffic"),
    );

    let weather = rerun::SerializedComponentBatch::new(
        Arc::new(arrow::array::StringArray::from(vec!["sunny"])),
        rerun::ComponentDescriptor::new("weather"),
    );

    // Adds another property, this time with user-defined data.
    rec.send_property(
        "situation",
        &[&confidences as &dyn rerun::AsComponents, &traffic, &weather],
    )?;

    // Properties, including the name, can be overwritten at any time.
    rec.send_recording_name("My episode")?;

    Ok(())
}
