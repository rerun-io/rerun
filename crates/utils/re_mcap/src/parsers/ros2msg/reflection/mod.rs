//! Minimal ROS 2 `.msg` reflection parser (messages only).
//!
//! This module parses the textual ROS 2 message definition format (aka `.msg`)
//! into a typed, reflection-friendly representation. It is intentionally kept
//! generic and does not rely on any pre-baked message definitions, so it can be
//! used to parse unknown types and still extract semantic meaning (types,
//! arrays, names, constants, default values).
use anyhow::Context as _;
use thiserror::Error;

use crate::parsers::ros2msg::reflection::message_spec::MessageSpecification;

pub mod deserialize;
pub mod message_spec;

/// Parse a schema name from a line starting with "MSG: ".
fn parse_schema_name(line: &str) -> Option<&str> {
    line.trim().strip_prefix("MSG: ").map(str::trim)
}

#[derive(Debug, Clone, PartialEq)]
pub struct MessageSchema {
    /// Specification of the main message type.
    pub spec: MessageSpecification,

    /// Dependent message types referenced by the main type.
    pub dependencies: Vec<MessageSpecification>, // Other message types referenced by this one.
}

impl MessageSchema {
    pub fn parse(name: &str, input: &str) -> anyhow::Result<Self> {
        let main_spec_content = extract_main_msg_spec(input);
        let specs = extract_msg_specs(input);

        let main_spec = MessageSpecification::parse(name, &main_spec_content)
            .with_context(|| format!("failed to parse main message spec `{name}`"))?;

        let mut dependencies = Vec::new();
        for (dep_name, dep_content) in specs {
            let dep_spec = MessageSpecification::parse(&dep_name, &dep_content)
                .with_context(|| format!("failed to parse dependent message spec `{dep_name}`"))?;
            dependencies.push(dep_spec);
        }

        Ok(Self {
            spec: main_spec,
            dependencies,
        })
    }
}

/// Check if a line is a schema separator (a line of at least 3 '=' characters).
pub fn is_schema_separator(line: &str) -> bool {
    let line = line.trim();
    line.len() >= 3 && line.chars().all(|c| c == '=')
}

/// Extract the main message specification from input, stopping at the first schema separator.
///
/// The main spec is everything before the first "====" separator line.
fn extract_main_msg_spec(input: &str) -> String {
    input
        .lines()
        .take_while(|line| !is_schema_separator(line))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Find "MSG: <name>" and take the rest as content
/// Extract all message specifications from input that are separated by schema separators.
///
/// Returns a vector of `(message_name, message_body)` pairs for each schema found.
fn extract_msg_specs(input: &str) -> Vec<(String, String)> {
    let mut specs = Vec::new();
    let mut current_section = Vec::new();

    for line in input.lines() {
        if is_schema_separator(line) {
            if let Some(spec) = parse_section(&current_section) {
                specs.push(spec);
            }
            current_section.clear();
        } else {
            current_section.push(line);
        }
    }

    // Handle the final section if it doesn't end with a separator
    if let Some(spec) = parse_section(&current_section) {
        specs.push(spec);
    }

    specs
}

/// Parse a section of lines into a (name, body) pair.
///
/// The first line should contain "MSG: <name>" and subsequent lines form the message body.
fn parse_section(lines: &[&str]) -> Option<(String, String)> {
    if lines.len() < 2 {
        return None;
    }

    let first_line = lines[0].trim();
    let name = parse_schema_name(first_line)?;
    let body = lines[1..].join("\n");

    Some((name.to_owned(), body))
}

#[cfg(test)]
mod tests {
    use crate::parsers::dds;
    use cdr_encoding::CdrDeserializer;

    use super::*;

    #[test]
    fn test_parse_message_spec() {
        let input = r#"
    # This is a comment
    std_msgs/Header header

    int32 field1
    float64 field2 3.14
    string field3 "hello"
    uint8[] field4

    geometry_msgs/Point[] field5

    uint32 CONST1=42 # inline comment
    "#;

        MessageSpecification::parse("test", input).unwrap();
    }

    #[test]
    fn test_parse_message_schema() {
        let input = r#"
# This message contains an uncompressed image
# (0, 0) is at top-left corner of image

std_msgs/Header header # Header timestamp should be acquisition time of image
                             # Header frame_id should be optical frame of camera
                             # origin of frame should be optical center of cameara
                             # +x should point to the right in the image
                             # +y should point down in the image
                             # +z should point into to plane of the image
                             # If the frame_id here and the frame_id of the CameraInfo
                             # message associated with the image conflict
                             # the behavior is undefined

uint32 height                # image height, that is, number of rows
uint32 width                 # image width, that is, number of columns

# The legal values for encoding are in file src/image_encodings.cpp
# If you want to standardize a new string format, join
# ros-users@lists.ros.org and send an email proposing a new encoding.

string encoding       # Encoding of pixels -- channel meaning, ordering, size
                      # taken from the list of strings in include/sensor_msgs/image_encodings.hpp

uint8 is_bigendian    # is this data bigendian?
uint32 step           # Full row length in bytes
uint8[] data          # actual matrix data, size is (step * rows)

================================================================================
MSG: std_msgs/Header
# Standard metadata for higher-level stamped data types.
# This is generally used to communicate timestamped data
# in a particular coordinate frame.

# Two-integer timestamp that is expressed as seconds and nanoseconds.
builtin_interfaces/Time stamp

# Transform frame with which this data is associated.
string frame_id

================================================================================
MSG: builtin_interfaces/Time
# This message communicates ROS Time defined here:
# https://design.ros2.org/articles/clock_and_time.html

# The seconds component, valid over all int32 values.
int32 sec

# The nanoseconds component, valid in the range [0, 10e9).
uint32 nanosec

        "#;
        const RAW_MSG: &[u8] = include_bytes!("../../../../../../../last_image_msg.bin");

        let spec = MessageSchema::parse("tf2_msgs/msg/TFMessage", input).unwrap();
        let representation_identifier =
            dds::RepresentationIdentifier::from_bytes(RAW_MSG[0..2].try_into().unwrap()).unwrap();

        let payload = &RAW_MSG[4..];
        let mut de = CdrDeserializer::<byteorder::LittleEndian>::new(payload);

        let _resolver = deserialize::MapResolver::new(
            spec.dependencies.iter().map(|dep| (dep.name.clone(), dep)),
        );
    }
}
