use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    /// Bad text/shape: missing tokens, bad delimiters, malformed constructs
    #[error("syntax error: {0}")]
    Syntax(String),

    /// Type names/annotations: unknown/invalid types, bad bounds, array forms
    #[error("type error: {0}")]
    Type(String),

    /// Literal/default/constant value parsing and type mismatches
    #[error("value error: {0}")]
    Value(String),

    /// Cross-entry checks: duplicates, bounds exceeded, defaults not allowed, naming rules
    #[error("validation error: {0}")]
    Validate(String),
}

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
    pub(super) fn parse(name: &str, input: &str) -> Result<Self, ParseError> {
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

            // Parse type and name (common to both constants and fields)
            let (ty_start, ty_end) = next_token_bounds(line, 0)
                .ok_or_else(|| ParseError::Syntax(format!("missing type in line: `{line}`")))?;
            let (name_start, name_end) = next_token_bounds(line, ty_end)
                .ok_or_else(|| ParseError::Syntax(format!("missing name in line: `{line}`")))?;

            let ty_str = &line[ty_start..ty_end];
            let name = &line[name_start..name_end];
            let ty = Type::parse(ty_str)?;

            // Check if rest starts with '=' to differentiate constant from field
            let rest = line[name_end..].trim();

            if rest.starts_with('=') {
                let constant = Constant::parse(ty, name, rest)?;
                constants.push(constant);
            } else {
                let field = Field::parse(ty, name, rest)?;
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
    fn parse(ty: Type, name: &str, rest: &str) -> Result<Self, ParseError> {
        if !Self::is_valid_constant_name(name) {
            return Err(ParseError::Validate(format!(
                "constant name must be all-caps alphanumeric and underscores only, got `{name}`"
            )));
        }

        if matches!(ty, Type::Array { .. }) {
            return Err(ParseError::Type(
                "constant type cannot be an array".to_owned(),
            ));
        }

        if matches!(ty, Type::Complex(_)) {
            return Err(ParseError::Type(
                "constant type must be a built-in type".to_owned(),
            ));
        }

        // rest should start with '='
        let value_str = rest
            .strip_prefix('=')
            .ok_or_else(|| ParseError::Syntax("constant definition missing '='".to_owned()))?
            .trim();

        let value = Literal::parse(value_str, &ty)?;

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
    fn parse(ty: Type, name: &str, rest: &str) -> Result<Self, ParseError> {
        let default = if rest.is_empty() {
            None
        } else {
            Some(Literal::parse(rest, &ty)?)
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
    Array { ty: Box<Self>, size: ArraySize },
}

impl Type {
    /// Parse a type definition, e.g. `int32`, `string<=10`, `float32[3]`, `pkg/Type[]`, `pkg/Type[<=5]`, etc.
    fn parse(s: &str) -> Result<Self, ParseError> {
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
                "wstring" => Ok(Self::BuiltIn(BuiltInType::WString(None))),
                s if s.starts_with("string<=") => {
                    // e.g. `string<=10`
                    Ok(Self::BuiltIn(BuiltInType::String(Some(
                        Self::parse_bounded_string_length(s, "string<=")?,
                    ))))
                }
                s if s.starts_with("wstring<=") => {
                    // e.g. `wstring<=10`
                    Ok(Self::BuiltIn(BuiltInType::WString(Some(
                        Self::parse_bounded_string_length(s, "wstring<=")?,
                    ))))
                }
                s => ComplexType::parse(s).map(Type::Complex),
            }
        }
    }

    fn parse_bounded_string_length(s: &str, prefix: &str) -> Result<usize, ParseError> {
        if let Some(len_str) = s.strip_prefix(prefix) {
            let len = len_str.parse::<usize>().map_err(|err| {
                ParseError::Type(format!("failed to parse bounded string length: {err}"))
            })?;

            Ok(len)
        } else {
            Err(ParseError::Type(format!(
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
    fn parse(s: &str) -> Result<Self, ParseError> {
        if let Some((package, name)) = s.rsplit_once('/') {
            if package.is_empty() || name.is_empty() {
                Err(ParseError::Type(format!(
                    "invalid complex type specifier: `{s}`, expected `some_package/SomeMessage` format"
                )))
            } else {
                Ok(Self::Absolute {
                    package: package.to_owned(),
                    name: name.to_owned(),
                })
            }
        } else if s.is_empty() {
            Err(ParseError::Type(format!(
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
    fn parse(array_part: &str) -> Result<Self, ParseError> {
        let array_part = array_part
            .strip_suffix(']')
            .ok_or_else(|| ParseError::Syntax("Missing closing ']' in array type".to_owned()))?;

        let array_size = if array_part.is_empty() {
            Self::Unbounded
        } else if let Ok(size) = array_part.parse::<usize>() {
            Self::Fixed(size)
        } else if let Some(n) = array_part.strip_prefix("<=") {
            let size = n.parse::<usize>().map_err(|err| {
                ParseError::Value(format!("Failed to parse bounded array size: {err}"))
            })?;
            Self::Bounded(size)
        } else {
            return Err(ParseError::Value(format!(
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
    Array(Vec<Self>),
}

impl Literal {
    fn parse(s: &str, ty: &Type) -> Result<Self, ParseError> {
        use BuiltInType::{
            Bool, Byte, Char, Float32, Float64, Int8, Int16, Int32, Int64, String, UInt8, UInt16,
            UInt32, UInt64, WString,
        };

        match ty {
            Type::BuiltIn(p) => match p {
                Bool => match s {
                    "true" => Ok(Self::Bool(true)),
                    "false" => Ok(Self::Bool(false)),
                    _ => Err(ParseError::Value(format!(
                        "failed to parse bool literal: `{s}`"
                    ))),
                },
                // Char is a signed 8-bit integer representing an ASCII character.
                Char | Int8 | Int16 | Int32 | Int64 => {
                    s.parse::<i64>().map(Self::Int).map_err(|err| {
                        ParseError::Value(format!("failed to parse integer literal `{s}`: {err}"))
                    })
                }
                // Byte is an unsigned 8-bit integer.
                Byte | UInt8 | UInt16 | UInt32 | UInt64 => {
                    s.parse::<u64>().map(Self::UInt).map_err(|err| {
                        ParseError::Value(format!(
                            "failed to parse unsigned integer literal `{s}`: {err}"
                        ))
                    })
                }
                Float32 | Float64 => s.parse::<f64>().map(Self::Float).map_err(|err| {
                    ParseError::Value(format!("failed to parse float literal `{s}`: {err}"))
                }),
                String(_) | WString(_) => {
                    let s = if (s.starts_with('"') && s.ends_with('"'))
                        || (s.starts_with('\'') && s.ends_with('\''))
                    {
                        // Remove quotes from quoted strings (both " and ')
                        &s[1..s.len() - 1]
                    } else {
                        // Use unquoted strings as-is
                        s
                    };
                    Ok(Self::String(s.to_owned()))
                }
            },
            Type::Array {
                ty: elem_ty,
                size: _,
            } => {
                let s = s.trim();

                if !s.starts_with('[') || !s.ends_with(']') {
                    Err(ParseError::Value(format!(
                        "array literal must start with '[' and end with ']': `{s}`"
                    )))
                } else {
                    let inner = &s[1..s.len() - 1];
                    let elems = inner
                        .split(',')
                        .map(|e| e.trim())
                        .filter(|e| !e.is_empty())
                        .map(|elem_str| Self::parse(elem_str, elem_ty))
                        .collect::<Result<Vec<_>, ParseError>>()?;

                    Ok(Self::Array(elems))
                }
            }
            Type::Complex(_) => Err(ParseError::Value(
                "literals of complex types are not supported".to_owned(),
            )),
        }
    }
}

/// Parse the bounds of the next whitespace-delimited token in a string.
/// Returns (`start_index`, `end_index`) of the token, or `None` if no token found.
/// Treats '=' as a separate single-character token, except when part of '<=' type bounds.
fn next_token_bounds(s: &str, start: usize) -> Option<(usize, usize)> {
    let remaining = s.get(start..)?;
    let token_start = start + remaining.len() - remaining.trim_start().len();
    let token = remaining.trim_start();

    if token.is_empty() {
        return None;
    }

    // If the token starts with '=', return it as a single-character token
    if token.starts_with('=') {
        return Some((token_start, token_start + 1));
    }

    // Find the end of the token
    // Stop at whitespace or standalone '=' (not part of '<=' type bound)
    let chars: Vec<char> = token.chars().collect();
    let mut token_len_bytes = 0;

    for (i, &c) in chars.iter().enumerate() {
        if c.is_whitespace() {
            break;
        }

        // Check if this is '=' and it's not part of '<='
        if c == '=' {
            // Check if previous char was '<'
            let prev_is_lt = i > 0 && chars[i - 1] == '<';
            if !prev_is_lt {
                // This '=' is standalone, stop here
                break;
            }
        }

        token_len_bytes += c.len_utf8();
    }

    if token_len_bytes == 0 {
        None
    } else {
        Some((token_start, token_start + token_len_bytes))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_constant() {
        assert_eq!(
            Constant::parse(Type::BuiltIn(BuiltInType::Int32), "CONST_NAME", "=42").unwrap(),
            Constant {
                ty: Type::BuiltIn(BuiltInType::Int32),
                name: "CONST_NAME".to_owned(),
                value: Literal::Int(42)
            }
        );

        assert_eq!(
            Constant::parse(
                Type::BuiltIn(BuiltInType::String(None)),
                "CONST_STR",
                "=\"hello\""
            )
            .unwrap(),
            Constant {
                ty: Type::BuiltIn(BuiltInType::String(None)),
                name: "CONST_STR".to_owned(),
                value: Literal::String("hello".to_owned())
            }
        );

        assert_eq!(
            Constant::parse(Type::BuiltIn(BuiltInType::Float64), "CONST_FLOAT", "=3.1").unwrap(),
            Constant {
                ty: Type::BuiltIn(BuiltInType::Float64),
                name: "CONST_FLOAT".to_owned(),
                value: Literal::Float(3.1)
            }
        );
    }

    #[test]
    fn invalid_constant() {
        assert!(Constant::parse(Type::BuiltIn(BuiltInType::Int32), "CONST_NAME", "").is_err()); // missing '=' and value
        assert!(Constant::parse(Type::BuiltIn(BuiltInType::Int32), "CONST_NAME", "=abc").is_err()); // invalid int value
        assert!(
            Constant::parse(
                Type::Array {
                    ty: Box::new(Type::BuiltIn(BuiltInType::Int32)),
                    size: ArraySize::Unbounded
                },
                "CONST_NAME",
                "=42"
            )
            .is_err()
        ); // array type not allowed
        assert!(Constant::parse(Type::BuiltIn(BuiltInType::Int32), "const_name", "=42").is_err()); // invalid name (not all-caps)
        assert!(Constant::parse(Type::BuiltIn(BuiltInType::Int32), "CONST__NAME", "=42").is_err()); // invalid name (consecutive underscores
        assert!(Constant::parse(Type::BuiltIn(BuiltInType::Int32), "_CONSTNAME", "=42").is_err()); // invalid name (doesn't start with letter)
        assert!(Constant::parse(Type::BuiltIn(BuiltInType::Int32), "CONSTNAME_", "=42").is_err()); // invalid name (ends with underscore)
    }

    #[test]
    fn valid_field() {
        assert_eq!(
            Field::parse(Type::BuiltIn(BuiltInType::Int32), "field_name", "").unwrap(),
            Field {
                ty: Type::BuiltIn(BuiltInType::Int32),
                name: "field_name".to_owned(),
                default: None
            }
        );

        assert_eq!(
            Field::parse(
                Type::BuiltIn(BuiltInType::String(Some(10))),
                "name",
                "\"default\""
            )
            .unwrap(),
            Field {
                name: "name".to_owned(),
                ty: Type::BuiltIn(BuiltInType::String(Some(10))),
                default: Some(Literal::String("default".to_owned()))
            }
        );

        assert_eq!(
            Field::parse(
                Type::Array {
                    ty: Box::new(Type::BuiltIn(BuiltInType::Float64)),
                    size: ArraySize::Fixed(3)
                },
                "position",
                "[0.0, 1.0, 2.0]"
            )
            .unwrap(),
            Field {
                name: "position".to_owned(),
                ty: Type::Array {
                    ty: Box::new(Type::BuiltIn(BuiltInType::Float64)),
                    size: ArraySize::Fixed(3)
                },
                default: Some(Literal::Array(vec![
                    Literal::Float(0.0),
                    Literal::Float(1.0),
                    Literal::Float(2.0)
                ]))
            }
        );

        assert_eq!(
            Field::parse(
                Type::Array {
                    ty: Box::new(Type::Complex(ComplexType::Absolute {
                        package: "geometry_msgs".to_owned(),
                        name: "Point".to_owned()
                    })),
                    size: ArraySize::Unbounded
                },
                "points",
                ""
            )
            .unwrap(),
            Field {
                name: "points".to_owned(),
                ty: Type::Array {
                    ty: Box::new(Type::Complex(ComplexType::Absolute {
                        package: "geometry_msgs".to_owned(),
                        name: "Point".to_owned()
                    })),
                    size: ArraySize::Unbounded
                },
                default: None
            }
        );

        assert_eq!(
            Field::parse(Type::BuiltIn(BuiltInType::Bool), "enabled", "true").unwrap(),
            Field {
                ty: Type::BuiltIn(BuiltInType::Bool),
                name: "enabled".to_owned(),
                default: Some(Literal::Bool(true))
            }
        );
    }

    #[test]
    fn invalid_field() {
        assert!(Field::parse(Type::BuiltIn(BuiltInType::Bool), "enabled", "maybe").is_err()); // invalid bool literal
    }

    #[test]
    fn strip_comment_works() {
        assert_eq!(strip_comment("int32 field # comment"), "int32 field ");
        assert_eq!(
            strip_comment("string name \"value # not a comment\" # comment"),
            "string name \"value # not a comment\" "
        );
        assert_eq!(
            strip_comment("string name 'value # not a comment' # comment"),
            "string name 'value # not a comment' "
        );
        assert_eq!(
            strip_comment("string name \"value \\\" # still not a comment\" # comment"),
            "string name \"value \\\" # still not a comment\" "
        );
        assert_eq!(
            strip_comment("string name 'value \\' # still not a comment' # comment"),
            "string name 'value \\' # still not a comment' "
        );
        assert_eq!(strip_comment("int32 field"), "int32 field");
        assert_eq!(strip_comment("# full line comment"), "");
        assert_eq!(strip_comment(""), "");
    }

    #[test]
    fn valid_message_spec() {
        let input = r#"
# "Enum"-like constants for mode/state
uint8 MODE_IDLE=0
uint8 MODE_INIT=1
uint8 MODE_RUN=2
uint8 MODE_PAUSE=3
uint8 MODE_ERROR=255

# Bitmask-style flags
uint32 FLAG_NONE=0
uint32 FLAG_VERBOSE=1
uint32 FLAG_RECORD=2
uint32 FLAG_DEBUG=4
uint32 FLAG_ALL=7  # FLAG_VERBOSE|FLAG_RECORD|FLAG_DEBUG

# Misc constants of various types
int32 MAX_RETRIES=5
float32 DEFAULT_SCALE=1.5
string DEFAULT_LABEL="calib_v1"

# Header & basic identification
std_msgs/Header header # standard ROS header
uint8 mode 1 # default: MODE_INIT
uint32 flags 0 # default bitmask

# Scalar defaults & bounded string default
bool enabled true # default: true
float32 scale 1.5 # mirrors DEFAULT_SCALE
string<=32 label "default_label" # bounded string with default

# Free-form description (unbounded string, optional empty)
string description

# Times & durations
builtin_interfaces/Time last_update
builtin_interfaces/Duration timeout

# Arrays & sequences
# Fixed-size numeric arrays
float32[9] K # 3x3 intrinsics (row-major)
float32[9] R # 3x3 rectification matrix
float32[12] P # 3x4 projection matrix

# Unbounded arrays
int32[] indices
float64[] residuals

# Upper-bounded sequence
uint8[<=16] small_buffer  # capacity-limited byte buffer

# Nested types & arrays of nested types
geometry_msgs/Pose[] trajectory          # a path as poses
geometry_msgs/Pose   goal_pose           # single nested message

# Example of "message-like" content with scattered comments

# Camera model parameters (assorted scalars with defaults)
float32 fx 0.0
float32 fy 0.0
float32 cx 0.0
float32 cy 0.0

# Distortion coefficients (variable length)
float64[] D

# Optional tags (strings). Bounded to keep memory in check.
string<=16 frame_id "map"
string<=16[] child_frame_id ["base_link", "camera_link", "lidar_link"]

float32 quality 0.0

# Retry counters
uint16 retry_count 0
uint16 max_retry   5

# Edge cases: blank lines, odd spacing, trailing comments, etc.

# (blank line below)

#    leading spaces before a field
    bool    has_calibration   true    # default with extra spaces

# weird spacing between type/name/default
int32     status_code      0

# comment after an unbounded array
string[]  notes    # foobar
    "#;

        let spec = MessageSpecification::parse("test", input).unwrap();
        assert_eq!(spec.fields.len(), 30);
        assert_eq!(spec.constants.len(), 13);

        // check first constant
        assert_eq!(spec.constants[0].name, "MODE_IDLE");
        assert_eq!(spec.constants[0].value, Literal::UInt(0));

        // check float constant
        assert_eq!(spec.constants[11].name, "DEFAULT_SCALE");
        assert_eq!(spec.constants[11].value, Literal::Float(1.5));

        // check string constant with quotes
        assert_eq!(spec.constants[12].name, "DEFAULT_LABEL");
        assert_eq!(
            spec.constants[12].value,
            Literal::String("calib_v1".to_owned())
        );

        // check first field
        assert_eq!(spec.fields[0].name, "header");
        assert_eq!(
            spec.fields[0].ty,
            Type::Complex(ComplexType::Absolute {
                package: "std_msgs".to_owned(),
                name: "Header".to_owned()
            })
        );

        // check bounded string field with default
        assert_eq!(spec.fields[5].name, "label");
        assert_eq!(
            spec.fields[5].ty,
            Type::BuiltIn(BuiltInType::String(Some(32)))
        );
        assert_eq!(
            spec.fields[5].default,
            Some(Literal::String("default_label".to_owned()))
        );

        // check unbounded array field
        assert_eq!(spec.fields[12].name, "indices");
        assert_eq!(
            spec.fields[12].ty,
            Type::Array {
                ty: Box::new(Type::BuiltIn(BuiltInType::Int32)),
                size: ArraySize::Unbounded
            }
        );

        // check fixed-size array field
        assert_eq!(spec.fields[9].name, "K");
        assert_eq!(
            spec.fields[9].ty,
            Type::Array {
                ty: Box::new(Type::BuiltIn(BuiltInType::Float32)),
                size: ArraySize::Fixed(9)
            }
        );

        // check bounded string array field
        assert_eq!(spec.fields[23].name, "child_frame_id");
        assert_eq!(
            spec.fields[23].ty,
            Type::Array {
                ty: Box::new(Type::BuiltIn(BuiltInType::String(Some(16)))),
                size: ArraySize::Unbounded
            }
        );
        assert_eq!(
            spec.fields[23].default,
            Some(Literal::Array(vec![
                Literal::String("base_link".to_owned()),
                Literal::String("camera_link".to_owned()),
                Literal::String("lidar_link".to_owned()),
            ]))
        );
    }
}
