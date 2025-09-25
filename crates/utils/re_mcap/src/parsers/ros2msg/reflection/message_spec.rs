use crate::parsers::ros2msg::reflection::Ros2IdlError;

/// A parsed ROS 2 message specification, including fields and constants.
///
/// ```text
/// # Example message spec:
/// int32 id
/// string name
/// float64[3] position # bounded array of 3 float64 values
/// geometry_msgs/Point[] points # unbounded array of complex types
///
/// uint32 CONST_VALUE=42
/// string CONST_STR="hello"
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct MessageSpecification {
    /// Name of the message type.
    pub name: String,

    /// Fields that make up the message payload.
    pub fields: Vec<Field>,

    /// Compile-time constants defined alongside fields.
    pub constants: Vec<Constant>,
}

impl MessageSpecification {
    pub(super) fn parse(name: &str, input: &str) -> Result<Self, Ros2IdlError> {
        let mut fields = Vec::new();
        let mut constants = Vec::new();

        for line in input.lines() {
            let line = strip_comment(line).trim();
            if line.is_empty() {
                continue;
            }

            if super::is_schema_separator(line) {
                continue;
            }

            if Constant::is_constant_line(line) {
                let constant = Constant::parse(line)?;
                constants.push(constant);
            } else {
                let field = Field::parse(line)?;
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

    fn parse(line: &str) -> Result<Self, Ros2IdlError> {
        let (type_and_name, value_str) = line
            .split_once('=')
            .ok_or_else(|| Ros2IdlError::Parse("constant definition missing '='".to_owned()))?;

        let (type_str, name) = type_and_name.trim().rsplit_once(' ').ok_or_else(|| {
            Ros2IdlError::Parse(
                "constant definition missing space between type and name".to_owned(),
            )
        })?;

        let ty = Type::parse(type_str)?;
        let value = Literal::parse(value_str.trim(), &ty)?;

        if matches!(ty, Type::Array { .. }) {
            return Err(Ros2IdlError::Parse(
                "constant type cannot be an array".to_owned(),
            ));
        }

        if !Self::is_valid_constant_name(name) {
            return Err(Ros2IdlError::Parse(format!(
                "constant name must be all-caps alphanumeric and underscores only, got `{name}`"
            )));
        }

        Ok(Self {
            ty,
            name: name.to_owned(),
            value,
        })
    }

    /// Constant names must be uppercase alphanumeric characters with underscores for separating words.
    /// They must start with an alphabetic character, they must not end with an underscore and never have two consecutive underscores.
    fn is_valid_constant_name(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }

        // Must start with uppercase letter and not end with underscore
        let mut chars = name.chars();
        if !chars.next().is_some_and(|c| c.is_ascii_uppercase()) || name.ends_with('_') {
            return false;
        }

        // Check for valid characters and no consecutive underscores
        !name.contains("__")
            && name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    }
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
    fn parse(line: &str) -> Result<Self, Ros2IdlError> {
        let line = line.trim();

        // Parse first two whitespace-delimited tokens (type and name) with indices
        fn next_token_bounds(s: &str, start: usize) -> Option<(usize, usize)> {
            let bytes = s.as_bytes();
            let mut i = start;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i >= bytes.len() {
                return None;
            }
            let start = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            Some((start, i))
        }

        let (ty_start, ty_end) = next_token_bounds(line, 0).ok_or_else(|| {
            Ros2IdlError::Parse(format!("field definition (`{line}`) missing type"))
        })?;
        let type_str = &line[ty_start..ty_end];

        let (name_start, name_end) = next_token_bounds(line, ty_end).ok_or_else(|| {
            Ros2IdlError::Parse(format!("field definition (`{line}`) missing name"))
        })?;
        let name = &line[name_start..name_end];

        let rest = line[name_end..].trim();
        let optional_default = if rest.is_empty() { None } else { Some(rest) };

        let ty = Type::parse(type_str).map_err(|_e| {
            Ros2IdlError::Parse(format!(
                "failed to parse type `{type_str}` in field definition `{line}`"
            ))
        })?;

        let default = if let Some(default_str) = optional_default {
            Some(Literal::parse(default_str, &ty).map_err(|_e| {
                Ros2IdlError::Parse(format!(
                    "failed to parse default value `{default_str}` for type `{ty:?}` in field definition `{line}`"
                ))
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

/// A built-in field type, e.g. `int32`, `float64`, `bool`, etc.
///
/// Types are taken from: <https://docs.ros.org/en/kilted/Concepts/Basic/About-Interfaces.html#field-types>
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuiltInType {
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
    String(Option<usize>),  // Optional max length for bounded strings.
    WString(Option<usize>), // Optional max length for bounded wide strings.
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    BuiltIn(BuiltInType),
    Complex(ComplexType), // Possibly qualified with package path, e.g. `pkg/Type
    Array { ty: Box<Type>, size: ArraySize },
}

impl Type {
    /// Parse a type definition, e.g. `int32`, `string<=10`, `float32[3]`, `pkg/Type[]`, `pkg/Type[<=5]`, etc.
    fn parse(s: &str) -> Result<Self, Ros2IdlError> {
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
                "bool" => Ok(Self::BuiltIn(BuiltInType::Bool)),
                "byte" => Ok(Self::BuiltIn(BuiltInType::Byte)),
                "char" => Ok(Self::BuiltIn(BuiltInType::Char)),
                "float32" => Ok(Self::BuiltIn(BuiltInType::Float32)),
                "float64" => Ok(Self::BuiltIn(BuiltInType::Float64)),
                "int8" => Ok(Self::BuiltIn(BuiltInType::Int8)),
                "int16" => Ok(Self::BuiltIn(BuiltInType::Int16)),
                "int32" => Ok(Self::BuiltIn(BuiltInType::Int32)),
                "int64" => Ok(Self::BuiltIn(BuiltInType::Int64)),
                "uint8" => Ok(Self::BuiltIn(BuiltInType::UInt8)),
                "uint16" => Ok(Self::BuiltIn(BuiltInType::UInt16)),
                "uint32" => Ok(Self::BuiltIn(BuiltInType::UInt32)),
                "uint64" => Ok(Self::BuiltIn(BuiltInType::UInt64)),
                "string" => Ok(Self::BuiltIn(BuiltInType::String(None))),
                s if s.starts_with("string<=") => Self::parse_bounded_string(s), // e.g. `string<=10`
                s if s.starts_with("wstring<=") => Err(Ros2IdlError::Parse(
                    "wstring types are not supported yet".to_owned(),
                )), // TODO(gijsd): Support utf16 strings.
                s => ComplexType::parse(s).map(Type::Complex),
            }
        }
    }

    fn parse_bounded_string(s: &str) -> Result<Self, Ros2IdlError> {
        if let Some(len_str) = s.strip_prefix("string<=") {
            let len = len_str.parse::<usize>().map_err(|e| {
                Ros2IdlError::Parse(format!("failed to parse bounded string length: {e}"))
            })?;
            Ok(Self::BuiltIn(BuiltInType::String(Some(len))))
        } else {
            Err(Ros2IdlError::Parse(format!(
                "invalid string type specifier: `{s}`"
            )))
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
/// RelativeType
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ComplexType {
    /// An absolute type with package, e.g. `pkg/Type`
    Absolute { package: String, name: String },

    /// A relative type without package, e.g. `Type`
    Relative { name: String },
}

impl ComplexType {
    fn parse(s: &str) -> Result<Self, Ros2IdlError> {
        if let Some((package, name)) = s.rsplit_once('/') {
            if package.is_empty() || name.is_empty() {
                Err(Ros2IdlError::Parse(format!(
                    "invalid complex type specifier: `{s}`, expected `some_package/SomeMessage` format"
                )))
            } else {
                Ok(Self::Absolute {
                    package: package.to_owned(),
                    name: name.to_owned(),
                })
            }
        } else if s.is_empty() {
            Err(Ros2IdlError::Parse(format!(
                "invalid complex type specifier: `{s}`, expected `some_package/SomeMessage` or `SomeMessage` format"
            )))
        } else {
            Ok(Self::Relative { name: s.to_owned() })
        }
    }
}

/// Size specifier for array types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArraySize {
    /// A fixed-size array, e.g. `[3]`
    Fixed(usize),

    /// A bounded-size array, e.g. `[<=5]`
    Bounded(usize),

    /// An unbounded array, e.g. `[]`
    Unbounded,
}

impl ArraySize {
    fn parse(array_part: &str) -> Result<Self, Ros2IdlError> {
        let array_part = array_part
            .strip_suffix(']')
            .ok_or_else(|| Ros2IdlError::Parse("Missing closing ']' in array type".to_owned()))?;

        let array_size = if array_part.is_empty() {
            Self::Unbounded
        } else if let Ok(size) = array_part.parse::<usize>() {
            Self::Fixed(size)
        } else if let Some(n) = array_part.strip_prefix("<=") {
            let size = n.parse::<usize>().map_err(|e| {
                Ros2IdlError::Parse(format!("Failed to parse bounded array size: {e}"))
            })?;
            Self::Bounded(size)
        } else {
            return Err(Ros2IdlError::Parse(format!(
                "invalid array size specifier: `{array_part}`"
            )));
        };

        Ok(array_size)
    }
}

/// A literal value, used for default values and constant definitions. Literals can only
/// be of [`BuiltInType`]s or arrays thereof.
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
    fn parse(s: &str, ty: &Type) -> Result<Self, Ros2IdlError> {
        use BuiltInType::{
            Bool, Byte, Char, Float32, Float64, Int8, Int16, Int32, Int64, String, UInt8, UInt16,
            UInt32, UInt64, WString,
        };

        match ty {
            Type::BuiltIn(p) => match p {
                Bool => match s {
                    "true" => Ok(Self::Bool(true)),
                    "false" => Ok(Self::Bool(false)),
                    _ => Err(Ros2IdlError::Parse(format!(
                        "failed to parse bool literal: `{s}`"
                    ))),
                },
                // Char is a signed 8-bit integer representing an ASCII character.
                Char | Int8 | Int16 | Int32 | Int64 => {
                    s.parse::<i64>().map(Self::Int).map_err(|e| {
                        Ros2IdlError::Parse(format!("failed to parse integer literal `{s}`: {e}"))
                    })
                }
                // Byte is an unsigned 8-bit integer.
                Byte | UInt8 | UInt16 | UInt32 | UInt64 => {
                    s.parse::<u64>().map(Self::UInt).map_err(|e| {
                        Ros2IdlError::Parse(format!(
                            "failed to parse unsigned integer literal `{s}`: {e}"
                        ))
                    })
                }
                Float32 | Float64 => s.parse::<f64>().map(Self::Float).map_err(|e| {
                    Ros2IdlError::Parse(format!("failed to parse float literal `{s}`: {e}"))
                }),
                String(_) => {
                    let s = s.trim_matches('"');
                    Ok(Self::String(s.to_owned()))
                }
                WString(_) => Err(Ros2IdlError::Parse(
                    "wstring literals are not supported yet".to_owned(),
                )), // TODO(gijsd): Support utf16 strings.
            },
            Type::Array {
                ty: elem_ty,
                size: _,
            } => {
                let s = s.trim();

                if !s.starts_with('[') || !s.ends_with(']') {
                    Err(Ros2IdlError::Parse(format!(
                        "array literal must start with '[' and end with ']': `{s}`"
                    )))
                } else {
                    let inner = &s[1..s.len() - 1];
                    let elems = inner
                        .split(',')
                        .map(|e| e.trim())
                        .filter(|e| !e.is_empty())
                        .map(|elem_str| Self::parse(elem_str, elem_ty))
                        .collect::<Result<Vec<_>, Ros2IdlError>>()?;

                    Ok(Self::Array(elems))
                }
            }
            Type::Complex(_) => Err(Ros2IdlError::Parse(
                "literals of complex types are not supported".to_owned(),
            )),
        }
    }
}

/// Strip comments from a line (anything after a '#').
fn strip_comment(s: &str) -> &str {
    let mut in_quote = false;
    let mut quote_char: Option<char> = None;
    let mut escaped = false;
    for (i, c) in s.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' => {
                // escape next character only inside quotes; outside it doesn't matter for '#'
                if in_quote {
                    escaped = true;
                }
            }
            '"' | '\'' => {
                if !in_quote {
                    in_quote = true;
                    quote_char = Some(c);
                } else if quote_char == Some(c) {
                    in_quote = false;
                    quote_char = None;
                }
            }
            '#' if !in_quote => {
                return &s[..i];
            }
            _ => {}
        }
    }
    s
}
