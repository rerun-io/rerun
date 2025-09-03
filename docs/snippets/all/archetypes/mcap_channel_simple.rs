//! Log a simple MCAP channel definition.

use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_mcap_channel").spawn()?;

    let mut metadata = HashMap::new();
    metadata.insert("frame_id".to_string(), "camera_link".to_string());
    metadata.insert("encoding".to_string(), "bgr8".to_string());

    rec.log(
        "mcap/channels/camera",
        &rerun::McapChannel::new(1, "/camera/image", "cdr").with_metadata(metadata),
    )?;

    Ok(())
}
