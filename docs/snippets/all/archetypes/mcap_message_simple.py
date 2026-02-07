"""Log a simple MCAP message with binary data."""

import rerun as rr

rr.init("rerun_example_mcap_message", spawn=True)

# Example binary message data (could be from a ROS message, protobuf, etc.)
# This represents a simple sensor reading encoded as bytes
sensor_data = b"sensor_reading: temperature=23.5, humidity=65.2, timestamp=1743465600"

rr.log(
    "mcap/messages/sensor_reading",
    rr.McapMessage(
        data=sensor_data,
    ),
)
