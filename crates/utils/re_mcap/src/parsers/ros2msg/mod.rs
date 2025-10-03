use crate::parsers::MessageParser;

mod definitions;

pub mod rcl_interfaces;
pub mod scalar_parser;
pub mod sensor_msgs;
pub mod std_msgs;

/// Trait for ROS2 message parsers that can be constructed with just a row count.
pub trait Ros2MessageParser: MessageParser {
    /// Create a new parser instance.
    fn new(num_rows: usize) -> Self;
}
