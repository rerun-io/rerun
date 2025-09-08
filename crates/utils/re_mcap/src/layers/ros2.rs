use crate::{
    parsers::MessageParser,
    parsers::ros2msg::{
        rcl_interfaces::LogMessageParser,
        sensor_msgs::{
            BatteryStateMessageParser, CameraInfoMessageParser, CompressedImageMessageParser,
            FluidPressureMessageParser, IlluminanceMessageParser, ImageMessageParser,
            ImuMessageParser, JointStateMessageParser, MagneticFieldMessageParser, NavSatFixMessageParser,
            PointCloud2MessageParser, RangeMessageParser, RelativeHumidityMessageParser,
            TemperatureMessageParser,
        },
        std_msgs::StringMessageParser,
    },
};

use super::MessageLayer;

/// Provides a set of predefined conversion of ROS2 messages.
///
/// Additionally, this layer will output Rerun archetypes for visualization in the viewer
/// for supported ROS2 message types.
#[derive(Debug, Default)]
pub struct McapRos2Layer;

impl MessageLayer for McapRos2Layer {
    fn identifier() -> super::LayerIdentifier {
        "ros2msg".into()
    }

    fn message_parser(
        &self,
        channel: &mcap::Channel<'_>,
        num_rows: usize,
    ) -> Option<Box<dyn MessageParser>> {
        let Some(schema) = channel.schema.as_ref() else {
            re_log::warn_once!(
                "Encountered ROS2 message without schema in channel {:?}",
                channel.topic
            );
            return None;
        };

        if schema.encoding.as_str() != "ros2msg" {
            return None;
        }

        Some(match schema.name.as_ref() {
            "rcl_interfaces/msg/Log" => Box::new(LogMessageParser::new(num_rows)),
            "sensor_msgs/msg/BatteryState" => Box::new(BatteryStateMessageParser::new(num_rows)),
            "sensor_msgs/msg/CameraInfo" => Box::new(CameraInfoMessageParser::new(num_rows)),
            "sensor_msgs/msg/CompressedImage" => {
                Box::new(CompressedImageMessageParser::new(num_rows))
            }
            "sensor_msgs/msg/FluidPressure" => Box::new(FluidPressureMessageParser::new(num_rows)),
            "sensor_msgs/msg/Illuminance" => Box::new(IlluminanceMessageParser::new(num_rows)),
            "sensor_msgs/msg/Image" => Box::new(ImageMessageParser::new(num_rows)),
            "sensor_msgs/msg/Imu" => Box::new(ImuMessageParser::new(num_rows)),
            "sensor_msgs/msg/JointState" => Box::new(JointStateMessageParser::new(num_rows)),
            "sensor_msgs/msg/MagneticField" => Box::new(MagneticFieldMessageParser::new(num_rows)),
            "sensor_msgs/msg/NavSatFix" => Box::new(NavSatFixMessageParser::new(num_rows)),
            "sensor_msgs/msg/PointCloud2" => Box::new(PointCloud2MessageParser::new(num_rows)),
            "sensor_msgs/msg/Range" => Box::new(RangeMessageParser::new(num_rows)),
            "sensor_msgs/msg/RelativeHumidity" => {
                Box::new(RelativeHumidityMessageParser::new(num_rows))
            }
            "sensor_msgs/msg/Temperature" => Box::new(TemperatureMessageParser::new(num_rows)),
            "std_msgs/msg/String" => Box::new(StringMessageParser::new(num_rows)),
            _ => {
                re_log::warn_once!(
                    "Message schema {:?} is currently not supported",
                    schema.name
                );
                return None;
            }
        })
    }
}
