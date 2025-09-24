//! Minimal ROS 2 `.msg` reflection parser (messages only).
//!
//! This module parses the textual ROS 2 message definition format (aka `.msg`)
//! into a typed, reflection-friendly representation. It is intentionally kept
//! generic and does not rely on any pre-baked message definitions, so it can be
//! used to parse unknown types and still extract semantic meaning (types,
//! arrays, names, constants, default values).
use anyhow::{Context as _, Result, bail};

pub mod deserializer;

/// A parsed ROS 2 message.
#[derive(Debug, Clone, PartialEq)]
pub struct MessageSpec {
    /// Name of the message type.
    pub name: String,

    /// Fields that make up the message payload.
    pub fields: Vec<Field>,

    /// Compile-time constants defined alongside fields.
    pub constants: Vec<Constant>,
}

impl MessageSpec {
    pub fn parse(name: &str, input: &str) -> anyhow::Result<Self> {
        let mut fields = Vec::new();
        let mut constants = Vec::new();

        for (line_num, line) in input.lines().enumerate() {
            let line = strip_comment(line).trim();
            if line.is_empty() {
                continue;
            }

            if is_schema_separator(line) {
                continue;
            }

            if Constant::is_constant_line(line) {
                let constant = Constant::parse(line)
                    .with_context(|| format!("failed to parse constant on line {line_num}"))?;
                constants.push(constant);
            } else {
                let field = Field::parse(line)
                    .with_context(|| format!("failed to parse field on line {line_num}"))?;
                fields.push(field);
            }
        }

        Ok(Self {
            name: name.to_owned(),
            fields,
            constants,
        })
    }
}

fn is_schema_separator(line: &str) -> bool {
    let line = line.trim();
    line.len() >= 79 && line.chars().all(|c| c == '=')
}

fn parse_schema_name(line: &str) -> Option<&str> {
    line.trim().strip_prefix("MSG: ").map(str::trim)
}

/// A message field definition.
/// Includes type, name, and optional default value.
///
/// Examples:
/// ```text
/// // Simple int32 field with no default value
/// int32 field_name
///
/// // Bounded string with max length 10, default value "default"
/// string<=10 name "default"
///
/// // Array of 3 float64s with default value [0.0, 0.0, 0.0]
/// float64[3] position [0.0, 0.0, 0.0]
///
/// // Unbounded array of complex types
/// pkg/Type[] items
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub ty: Type,
    pub name: String,
    pub default: Option<Literal>,
}

impl Field {
    fn parse(line: &str) -> anyhow::Result<Self> {
        let line = line.trim();
        let mut parts = line.split_whitespace();

        let type_str = parts
            .next()
            .with_context(|| format!("field definition (`{line}`) missing type"))?;

        let name = parts
            .next()
            .with_context(|| format!("field definition (`{line}`) missing name"))?;

        let optional_default = parts.next();

        if parts.next().is_some() {
            bail!("field definition (`{line}`) has too many parts");
        }

        let ty = Type::parse(type_str).with_context(|| {
            format!("failed to parse type `{type_str}` in field definition `{line}`")
        })?;

        let default = if let Some(default_str) = optional_default {
            Some(Literal::parse(default_str, &ty).with_context(|| {
                format!(
                    "failed to parse default value `{default_str}` for type `{ty:?}` in field definition `{line}`"
                )
            })?)
        } else {
            None
        };

        Ok(Self {
            ty,
            name: name.to_owned(),
            default,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrimitiveType {
    Bool,
    Byte,
    Char,
    Float32,
    Float64,
    Int8,
    Int16,
    Int32,
    Int64,
    UInt8,
    UInt16,
    UInt32,
    UInt64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Primitive(PrimitiveType),
    String(Option<usize>), // Optional max length for bounded strings.
    Complex(ComplexType),  // Possibly qualified with package path, e.g. `pkg/Type
    Array { ty: Box<Type>, size: ArraySize },
}

impl Type {
    /// Parse a type definition, e.g. `int32`, `string<=10`, `float32[3]`, `pkg/Type[]`, `pkg/Type[<=5]`, etc.
    fn parse(s: &str) -> anyhow::Result<Self> {
        let s = s.trim();

        if let Some((base, array_part)) = s.split_once('[') {
            let base = Self::parse(base)?;
            let array_size = ArraySize::parse(array_part)?;

            Ok(Self::Array {
                ty: Box::new(base),
                size: array_size,
            })
        } else {
            match s {
                "bool" => Ok(Self::Primitive(PrimitiveType::Bool)),
                "byte" => Ok(Self::Primitive(PrimitiveType::Byte)),
                "char" => Ok(Self::Primitive(PrimitiveType::Char)),
                "float32" => Ok(Self::Primitive(PrimitiveType::Float32)),
                "float64" => Ok(Self::Primitive(PrimitiveType::Float64)),
                "int8" => Ok(Self::Primitive(PrimitiveType::Int8)),
                "int16" => Ok(Self::Primitive(PrimitiveType::Int16)),
                "int32" => Ok(Self::Primitive(PrimitiveType::Int32)),
                "int64" => Ok(Self::Primitive(PrimitiveType::Int64)),
                "uint8" => Ok(Self::Primitive(PrimitiveType::UInt8)),
                "uint16" => Ok(Self::Primitive(PrimitiveType::UInt16)),
                "uint32" => Ok(Self::Primitive(PrimitiveType::UInt32)),
                "uint64" => Ok(Self::Primitive(PrimitiveType::UInt64)),
                "string" => Ok(Self::String(None)),
                s if s.starts_with("string") => Self::parse_bounded_string(s), // e.g. `string<=10`
                s => ComplexType::parse(s).map(Type::Complex),
            }
        }
    }

    fn parse_bounded_string(s: &str) -> anyhow::Result<Self> {
        if s.starts_with("string<=") {
            let len_str = &s["string<=".len()..s.len() - 1];
            let len = len_str
                .parse::<usize>()
                .with_context(|| "failed to parse bounded string length")?;
            Ok(Self::String(Some(len)))
        } else {
            bail!("invalid string type specifier: `{s}`");
        }
    }
}

/// A complex (non-primitive) type, possibly qualified with a package path.
///
/// Examples:
/// ```text
/// // Absolute type with package
/// pkg/Type
///
/// // Relative type without package
/// Type
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ComplexType {
    /// An absolute type with package, e.g. `pkg/Type`
    Absolute { package: String, name: String },

    /// A relative type without package, e.g. `Type`
    Relative { name: String },
}

impl ComplexType {
    fn parse(s: &str) -> anyhow::Result<Self> {
        if let Some((package, name)) = s.rsplit_once('/') {
            if package.is_empty() || name.is_empty() {
                bail!(
                    "invalid complex type specifier: `{s}`, expected `some_package/SomeMessage` format"
                );
            }
            Ok(Self::Absolute {
                package: package.to_owned(),
                name: name.to_owned(),
            })
        } else {
            if s.is_empty() {
                bail!(
                    "invalid complex type specifier: `{s}`, expected `some_package/SomeMessage` or `SomeMessage` format"
                );
            }

            Ok(Self::Relative { name: s.to_owned() })
        }
    }
}

/// Size specifier for array types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArraySize {
    Fixed(usize),
    Bounded(usize),
    Unbounded,
}

impl ArraySize {
    fn parse(array_part: &str) -> Result<Self> {
        let array_part = array_part
            .strip_suffix(']')
            .with_context(|| "Missing closing ']' in array type")?;

        let array_size = if array_part.is_empty() {
            Self::Unbounded
        } else if let Ok(size) = array_part.parse::<usize>() {
            Self::Fixed(size)
        } else if array_part.ends_with('>') && array_part.starts_with('<') {
            let size_str = &array_part[1..array_part.len() - 1];
            let size = size_str
                .parse::<usize>()
                .with_context(|| "Failed to parse bounded array size")?;
            Self::Bounded(size)
        } else {
            bail!("invalid array size specifier: `{array_part}`");
        };
        Ok(array_size)
    }
}

/// A literal value, used for default values and constant definitions.
/// Can be a primitive, string, or array of literals.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    String(String),
    Array(Vec<Literal>),
}

impl Literal {
    fn parse(s: &str, ty: &Type) -> Result<Self> {
        match ty {
            Type::Primitive(p) => match p {
                PrimitiveType::Bool => {
                    let v = match s {
                        "true" => true,
                        "false" => false,
                        _ => bail!("invalid boolean literal: `{s}`"),
                    };
                    Ok(Self::Bool(v))
                }
                PrimitiveType::Byte
                | PrimitiveType::Char
                | PrimitiveType::Int8
                | PrimitiveType::Int16
                | PrimitiveType::Int32
                | PrimitiveType::Int64 => s
                    .parse::<i64>()
                    .map(Self::Int)
                    .with_context(|| "failed to parse integer literal"),
                PrimitiveType::UInt8
                | PrimitiveType::UInt16
                | PrimitiveType::UInt32
                | PrimitiveType::UInt64 => s
                    .parse::<u64>()
                    .map(Self::UInt)
                    .with_context(|| "failed to parse unsigned integer literal"),
                PrimitiveType::Float32 | PrimitiveType::Float64 => s
                    .parse::<f64>()
                    .map(Self::Float)
                    .with_context(|| "failed to parse float literal"),
            },
            Type::String(_) => {
                let s = s.trim_matches('"');
                Ok(Self::String(s.to_owned()))
            }
            Type::Array {
                ty: elem_ty,
                size: _,
            } => {
                let s = s.trim();

                if !s.starts_with('[') || !s.ends_with(']') {
                    bail!("array literal must start with '[' and end with ']': `{s}`");
                }
                let inner = &s[1..s.len() - 1];
                let elems_str = inner.split(',').map(|e| e.trim()).filter(|e| !e.is_empty());
                let mut elems = Vec::new();
                for elem_str in elems_str {
                    let elem = Self::parse(elem_str, elem_ty)?;
                    elems.push(elem);
                }

                Ok(Self::Array(elems))
            }
            Type::Complex(_) => bail!("cannot parse literal for named type"),
        }
    }
}

/// A compile-time constant defined alongside fields.
/// Includes type, name, and value.
///
/// Examples:
/// ```text
/// // Integer constant
/// int32 CONST_NAME=42
///
/// // String constant
/// string CONST_STR="hello"
///
/// // Float constant
/// float64 CONST_FLOAT=3.14
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Constant {
    pub ty: Type,
    pub name: String,
    pub value: Literal,
}

impl Constant {
    /// Determine if a line is a constant definition or a field definition.
    ///
    /// A constant definition has the following structure: `<type> <NAME>=<value>`
    /// where `NAME` is all-caps with digits and underscores only, and `<type>` is not an array.
    ///
    /// We look for the first `=` that is not inside quotes or brackets to make this determination.
    fn is_constant_line(line: &str) -> bool {
        let mut in_quote = false;
        let mut bracket = 0usize;
        for c in line.chars() {
            match c {
                '"' | '\'' => in_quote = !in_quote,
                '[' => bracket += 1,
                ']' => {
                    bracket = bracket.saturating_sub(1);
                }
                '=' if !in_quote && bracket == 0 => return true,
                _ => {}
            }
        }

        false
    }

    fn parse(line: &str) -> anyhow::Result<Self> {
        let (type_and_name, value_str) = line
            .split_once('=')
            .with_context(|| "constant definition missing '='")?;
        let (type_str, name) = type_and_name
            .trim()
            .rsplit_once(' ')
            .with_context(|| "constant definition missing space between type and name")?;

        let ty = Type::parse(type_str)?;
        let value = Literal::parse(value_str.trim(), &ty)?;

        if matches!(ty, Type::Array { .. }) {
            bail!("constant type cannot be an array");
        }
        if !name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        {
            bail!("constant name must be all-caps alphanumeric and underscores only, got `{name}`");
        }

        Ok(Self {
            ty,
            name: name.to_owned(),
            value,
        })
    }
}

fn strip_comment(s: &str) -> &str {
    s.split_once('#').map(|(before, _)| before).unwrap_or(s)
}

#[derive(Debug, Clone, PartialEq)]
pub struct MessageSchema {
    pub name: String,
    pub spec: MessageSpec,
    pub dependencies: Vec<MessageSpec>, // Other message types referenced by this one.
}

impl MessageSchema {
    pub fn parse(name: String, input: &str) -> anyhow::Result<Self> {
        let main_spec_content = extract_main_msg_spec(input);
        let specs = extract_msg_specs(input);

        let main_spec = MessageSpec::parse(&name, &main_spec_content)
            .with_context(|| format!("failed to parse main message spec `{name}`"))?;

        let mut dependencies = Vec::new();
        for (dep_name, dep_content) in specs {
            let dep_spec = MessageSpec::parse(&dep_name, &dep_content)
                .with_context(|| format!("failed to parse dependent message spec `{dep_name}`"))?;
            dependencies.push(dep_spec);
        }

        Ok(Self {
            name,
            spec: main_spec,
            dependencies,
        })
    }
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

        MessageSpec::parse("test", input).unwrap();
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

        let spec = MessageSchema::parse("tf2_msgs/msg/TFMessage".to_owned(), input).unwrap();
        let representation_identifier =
            dds::RepresentationIdentifier::from_bytes(RAW_MSG[0..2].try_into().unwrap()).unwrap();

        let payload = &RAW_MSG[4..];
        let mut de = CdrDeserializer::<byteorder::LittleEndian>::new(payload);

        let mut resolver = std::collections::HashMap::new();
        for dep in &spec.dependencies {
            resolver.insert(dep.name.clone(), dep);
        }
    }
}
