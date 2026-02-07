//! Log a simple MCAP channel definition.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_mcap_channel").spawn()?;

    rec.log(
        "mcap/channels/camera",
        &rerun::McapChannel::new(1, "/camera/image", "cdr")
            .with_metadata([("frame_id", "camera_link"), ("encoding", "bgr8")]),
    )?;

    Ok(())
}
