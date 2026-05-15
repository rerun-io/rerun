use std::collections::BTreeMap;

use super::MessageDecoder;
use crate::parsers::MessageParser;
use crate::parsers::ros1msg::Ros1MessageParser;
use crate::parsers::ros1msg::geometry_msgs::PoseWithCovarianceStampedMessageParser;
use crate::parsers::ros1msg::nav_msgs::OccupancyGridMessageParser;
use crate::parsers::ros1msg::sensor_msgs::{
    CameraInfoMessageParser, CompressedImageMessageParser, ImageMessageParser,
};
use crate::parsers::ros1msg::std_msgs::StringMessageParser;
use crate::parsers::ros1msg::tf2_msgs::tf_message::TfMessageParser;

type ParserFactory = fn(usize) -> Box<dyn MessageParser>;

#[derive(Debug)]
pub struct McapRos1Decoder {
    registry: BTreeMap<String, ParserFactory>,
}

impl McapRos1Decoder {
    const SCHEMA_ENCODING: &str = "ros1msg";

    fn empty() -> Self {
        Self {
            registry: BTreeMap::new(),
        }
    }

    pub fn new() -> Self {
        Self::empty()
            .register_parser::<PoseWithCovarianceStampedMessageParser>(
                "geometry_msgs/PoseWithCovarianceStamped",
            )
            .register_parser::<ImageMessageParser>("sensor_msgs/Image")
            .register_parser::<CompressedImageMessageParser>("sensor_msgs/CompressedImage")
            .register_parser::<CameraInfoMessageParser>("sensor_msgs/CameraInfo")
            .register_parser::<TfMessageParser>("tf2_msgs/TFMessage")
            .register_parser::<OccupancyGridMessageParser>("nav_msgs/OccupancyGrid")
            .register_parser::<StringMessageParser>("std_msgs/String")
    }

    pub fn register_parser<T: Ros1MessageParser + 'static>(mut self, schema_name: &str) -> Self {
        self.registry
            .insert(schema_name.to_owned(), |n| Box::new(T::new(n)));
        self
    }
}

impl Default for McapRos1Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageDecoder for McapRos1Decoder {
    fn identifier() -> super::DecoderIdentifier {
        "ros1msg".into()
    }

    fn supports_channel(&self, channel: &mcap::Channel<'_>) -> bool {
        let Some(schema) = channel.schema.as_ref() else {
            return false;
        };

        self.registry.contains_key(&schema.name) && supports_ros1_channel(channel)
    }

    fn message_parser(
        &self,
        channel: &mcap::Channel<'_>,
        num_rows: usize,
    ) -> Option<Box<dyn MessageParser>> {
        let schema = channel.schema.as_ref()?;
        if schema.encoding.as_str() != Self::SCHEMA_ENCODING {
            return None;
        }

        self.registry.get(&schema.name).map(|make| make(num_rows))
    }
}

fn is_ros1_message_encoding(message_encoding: &str) -> bool {
    message_encoding.eq_ignore_ascii_case("ros1")
}

fn supports_ros1_channel(channel: &mcap::Channel<'_>) -> bool {
    let Some(schema) = channel.schema.as_ref() else {
        return false;
    };

    if schema.encoding.as_str() != McapRos1Decoder::SCHEMA_ENCODING {
        return false;
    }

    if !is_ros1_message_encoding(&channel.message_encoding) {
        if !channel.message_encoding.trim().is_empty() {
            re_log::warn_once!(
                concat!(
                    "MCAP channel '{}' has a ROS1 message schema, but unknown encoding '{}'. ",
                    "ROS 1 deserialization is only supported for ros1-encoded messages."
                ),
                channel.topic,
                channel.message_encoding,
            );
        }
        return false;
    }

    true
}
