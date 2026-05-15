use crate::parsers::MessageParser;

pub mod definitions;
pub mod geometry_msgs;
pub mod nav_msgs;
pub mod sensor_msgs;
pub mod std_msgs;
pub mod tf2_msgs;
pub mod wire;

pub trait Ros1MessageParser: MessageParser {
    fn new(num_rows: usize) -> Self;
}
