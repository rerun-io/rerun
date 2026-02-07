//! Sets the recording properties.

use std::sync::Arc;

use rerun::external::arrow;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_recording_properties").spawn()?;

    // Recordings can have an optional name.
    rec.send_recording_name("My recording")?;

    // Start time is set automatically, but we can overwrite it at any time.
    rec.send_recording_start_time(1742539110661000000)?;

    // Adds a user-defined property to the recording, using an existing Rerun type.
    rec.send_property(
        "camera_left",
        &rerun::archetypes::Points3D::new([[1.0, 0.1, 1.0]]),
    )?;

    let other = rerun::AnyValues::default()
        .with_component_from_data(
            "confidences",
            Arc::new(arrow::array::Float64Array::from(vec![0.3, 0.4, 0.5, 0.6])),
        )
        .with_component_from_data(
            "traffic",
            Arc::new(arrow::array::StringArray::from(vec!["low"])),
        )
        .with_component_from_data(
            "weather",
            Arc::new(arrow::array::StringArray::from(vec!["sunny"])),
        );

    // Adds another property, this time with user-defined data.
    rec.send_property("situation", &other)?;

    // Properties, including the name, can be overwritten at any time.
    rec.send_recording_name("My episode")?;

    Ok(())
}
