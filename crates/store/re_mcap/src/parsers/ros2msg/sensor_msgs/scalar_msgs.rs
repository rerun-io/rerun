use crate::parsers::ros2msg::definitions::sensor_msgs::{BatteryState, Range};
use crate::parsers::ros2msg::definitions::std_msgs::Header;
use crate::parsers::ros2msg::scalar_parser::{ScalarExtractor, ScalarMessageParser};

// Type aliases for scalar messages convenience
pub type RangeMessageParser = ScalarMessageParser<Range>;

pub type BatteryStateMessageParser = ScalarMessageParser<BatteryState>;

impl ScalarExtractor for Range {
    fn extract_scalars(&self) -> Vec<(&str, f64)> {
        vec![
            ("range", self.range as f64),
            ("min_range", self.min_range as f64),
            ("max_range", self.max_range as f64),
        ]
    }

    fn header(&self) -> &Header {
        &self.header
    }
}

impl ScalarExtractor for BatteryState {
    fn extract_scalars(&self) -> Vec<(&str, f64)> {
        vec![
            ("percentage", self.percentage as f64),
            ("voltage", self.voltage as f64),
            ("current", self.current as f64),
            ("charge", self.charge as f64),
            ("temperature", self.temperature as f64),
        ]
    }

    fn header(&self) -> &Header {
        &self.header
    }
}
