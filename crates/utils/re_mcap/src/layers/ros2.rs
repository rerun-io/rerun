use crate::{
    parsers::MessageParser,
    parsers::ros2msg::{
        sensor_msgs::{
            CameraInfoMessageParser, CompressedImageMessageParser, ImageMessageParser,
            ImuMessageParser, JointStateMessageParser, PointCloud2MessageParser,
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
        let Some(name) = channel.schema.as_ref().map(|schema| schema.name.as_str()) else {
            re_log::warn_once!(
                "Encountered ROS2 message without schema in channel {:?}",
                channel.topic
            );
            return None;
        };

        Some(match name {
            "std_msgs/msg/String" => Box::new(StringMessageParser::new(num_rows)),
            "sensor_msgs/msg/JointState" => Box::new(JointStateMessageParser::new(num_rows)),
            "sensor_msgs/msg/Imu" => Box::new(ImuMessageParser::new(num_rows)),
            "sensor_msgs/msg/Image" => Box::new(ImageMessageParser::new(num_rows)),
            "sensor_msgs/msg/CameraInfo" => Box::new(CameraInfoMessageParser::new(num_rows)),
            "sensor_msgs/msg/CompressedImage" => {
                Box::new(CompressedImageMessageParser::new(num_rows))
            }
            "sensor_msgs/msg/PointCloud2" => Box::new(PointCloud2MessageParser::new(num_rows)),
            _ => {
                re_log::warn_once!("Message schema {name:?} is currently not supported");
                return None;
            }
        })
    }
}
