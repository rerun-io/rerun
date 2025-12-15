use std::collections::BTreeMap;

use super::MessageLayer;
use crate::parsers::MessageParser;
use crate::parsers::ros2msg::Ros2MessageParser;
use crate::parsers::ros2msg::geometry_msgs::PoseStampedMessageParser;
use crate::parsers::ros2msg::rcl_interfaces::LogMessageParser;
use crate::parsers::ros2msg::sensor_msgs::{
    BatteryStateMessageParser, CameraInfoMessageParser, CompressedImageMessageParser,
    FluidPressureMessageParser, IlluminanceMessageParser, ImageMessageParser, ImuMessageParser,
    JointStateMessageParser, MagneticFieldMessageParser, NavSatFixMessageParser,
    PointCloud2MessageParser, RangeMessageParser, RelativeHumidityMessageParser,
    TemperatureMessageParser,
};
use crate::parsers::ros2msg::std_msgs::StringMessageParser;
use crate::parsers::ros2msg::tf2_msgs::tf_message::TfMessageParser;

type ParserFactory = fn(usize) -> Box<dyn MessageParser>;

#[derive(Debug)]
pub struct McapRos2Layer {
    registry: BTreeMap<String, ParserFactory>,
}

impl McapRos2Layer {
    const ENCODING: &str = "ros2msg";

    fn empty() -> Self {
        Self {
            registry: BTreeMap::new(),
        }
    }

    /// Creates a new [`McapRos2Layer`] with all supported message types pre-registered
    pub fn new() -> Self {
        Self::empty()
            // geometry_msgs
            .register_parser::<PoseStampedMessageParser>("geometry_msgs/msg/PoseStamped")
            // rcl_interfaces
            .register_parser::<LogMessageParser>("rcl_interfaces/msg/Log")
            // sensor_msgs
            .register_parser::<BatteryStateMessageParser>("sensor_msgs/msg/BatteryState")
            .register_parser::<CameraInfoMessageParser>("sensor_msgs/msg/CameraInfo")
            .register_parser::<CompressedImageMessageParser>("sensor_msgs/msg/CompressedImage")
            .register_parser::<FluidPressureMessageParser>("sensor_msgs/msg/FluidPressure")
            .register_parser::<IlluminanceMessageParser>("sensor_msgs/msg/Illuminance")
            .register_parser::<ImageMessageParser>("sensor_msgs/msg/Image")
            .register_parser::<ImuMessageParser>("sensor_msgs/msg/Imu")
            .register_parser::<JointStateMessageParser>("sensor_msgs/msg/JointState")
            .register_parser::<MagneticFieldMessageParser>("sensor_msgs/msg/MagneticField")
            .register_parser::<NavSatFixMessageParser>("sensor_msgs/msg/NavSatFix")
            .register_parser::<PointCloud2MessageParser>("sensor_msgs/msg/PointCloud2")
            .register_parser::<RangeMessageParser>("sensor_msgs/msg/Range")
            .register_parser::<RelativeHumidityMessageParser>("sensor_msgs/msg/RelativeHumidity")
            .register_parser::<TemperatureMessageParser>("sensor_msgs/msg/Temperature")
            // std_msgs
            .register_parser::<StringMessageParser>("std_msgs/msg/String")
            // tf2_msgs
            .register_parser::<TfMessageParser>("tf2_msgs/msg/TFMessage")
    }

    /// Registers a new message parser for the given schema name
    pub fn register_parser<T: Ros2MessageParser + 'static>(mut self, schema_name: &str) -> Self {
        self.registry
            .insert(schema_name.to_owned(), |n| Box::new(T::new(n)));
        self
    }

    /// Registers a message parser with a custom factory function
    pub fn register_parser_with_factory(
        mut self,
        schema_name: &str,
        factory: ParserFactory,
    ) -> Self {
        self.registry.insert(schema_name.to_owned(), factory);
        self
    }

    /// Returns true if the given schema is supported by this layer
    pub fn supports_schema(&self, schema_name: &str) -> bool {
        self.registry.contains_key(schema_name)
    }
}

impl Default for McapRos2Layer {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageLayer for McapRos2Layer {
    fn identifier() -> super::LayerIdentifier {
        "ros2msg".into()
    }

    fn supports_channel(&self, channel: &mcap::Channel<'_>) -> bool {
        channel.schema.as_ref().is_some_and(|s| {
            s.encoding.as_str() == Self::ENCODING && self.registry.contains_key(&s.name)
        })
    }

    fn message_parser(
        &self,
        channel: &mcap::Channel<'_>,
        num_rows: usize,
    ) -> Option<Box<dyn MessageParser>> {
        let schema = channel.schema.as_ref()?;
        if schema.encoding.as_str() != Self::ENCODING {
            return None;
        }

        if let Some(make) = self.registry.get(&schema.name) {
            Some(make(num_rows))
        } else {
            re_log::warn_once!(
                "Message schema {:?} is currently not supported",
                schema.name
            );

            None
        }
    }
}
