use crate::parsers::ros2msg::definitions::sensor_msgs::{
    BatteryState, FluidPressure, Illuminance, Range, RelativeHumidity, Temperature,
};
use crate::parsers::ros2msg::definitions::std_msgs::Header;
use crate::parsers::ros2msg::scalar_parser::{ScalarExtractor, ScalarMessageParser};

// Type aliases for scalar messages convenience
pub type TemperatureMessageParser = ScalarMessageParser<Temperature>;

pub type FluidPressureMessageParser = ScalarMessageParser<FluidPressure>;

pub type RelativeHumidityMessageParser = ScalarMessageParser<RelativeHumidity>;

pub type IlluminanceMessageParser = ScalarMessageParser<Illuminance>;

pub type RangeMessageParser = ScalarMessageParser<Range>;

pub type BatteryStateMessageParser = ScalarMessageParser<BatteryState>;

impl ScalarExtractor for Temperature {
    fn extract_scalars(&self) -> Vec<(&str, f64)> {
        vec![
            ("temperature", self.temperature),
            ("variance", self.variance),
        ]
    }

    fn header(&self) -> &Header {
        &self.header
    }
}

impl ScalarExtractor for FluidPressure {
    fn extract_scalars(&self) -> Vec<(&str, f64)> {
        vec![
            ("fluid_pressure", self.fluid_pressure),
            ("variance", self.variance),
        ]
    }

    fn header(&self) -> &Header {
        &self.header
    }
}

impl ScalarExtractor for RelativeHumidity {
    fn extract_scalars(&self) -> Vec<(&str, f64)> {
        vec![
            ("relative_humidity", self.humidity),
            ("variance", self.variance),
        ]
    }

    fn header(&self) -> &Header {
        &self.header
    }
}

impl ScalarExtractor for Illuminance {
    fn extract_scalars(&self) -> Vec<(&str, f64)> {
        vec![
            ("illuminance", self.illuminance),
            ("variance", self.variance),
        ]
    }

    fn header(&self) -> &Header {
        &self.header
    }
}

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
