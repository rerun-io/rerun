//! Log a simple MCAP message with binary data.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_mcap_message").spawn()?;

    // Example binary message data (could be from a ROS message, protobuf, etc.)
    // This represents a simple sensor reading encoded as bytes
    let sensor_data = "sensor_reading: temperature=23.5, humidity=65.2, timestamp=1743465600";

    rec.log(
        "mcap/messages/sensor_reading",
        &rerun::McapMessage::new(sensor_data.as_bytes()),
    )?;

    Ok(())
}
