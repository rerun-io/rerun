// Log a simple MCAP message with binary data.

#include <rerun.hpp>
#include <string>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_mcap_message");
    rec.spawn().exit_on_failure();

    // Example binary message data (could be from a ROS message, protobuf, etc.)
    // This represents a simple sensor reading encoded as bytes
    const std::string sensor_data =
        "sensor_reading: temperature=23.5, humidity=65.2, timestamp=1743465600";

    rec.log(
        "mcap/messages/sensor_reading",
        rerun::archetypes::McapMessage(rerun::components::Blob(sensor_data))
    );
}
