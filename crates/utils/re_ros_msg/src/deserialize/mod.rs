use std::collections::{BTreeMap, HashMap};

use re_cdr::{CdrEndian, CdrReader, Error, Result};

use crate::deserialize::primitive_array::PrimitiveArray;
use crate::message_spec::{
    ArraySize, BuiltInType, ComplexType, MessageSpecification, Type, message_package,
};

pub mod primitive_array;

/// A single deserialized value of any type that can appear in a ROS message.
#[derive(Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    I8(i8),
    U8(u8),
    I16(i16),
    U16(u16),
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
    F32(f32),
    F64(f64),
    String(String),

    /// Fixed-size array of values.
    Array(Vec<Self>),

    /// Variable-size or bounded array of values.
    Sequence(Vec<Self>),

    /// Fixed-size array of primitive values.
    PrimitiveArray(primitive_array::PrimitiveArray),

    /// Variable-size or bounded array of primitive values.
    PrimitiveSeq(primitive_array::PrimitiveArray),

    /// Nested message.
    Message(BTreeMap<String, Self>),
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bool(v) => write!(f, "Bool({v})"),
            Self::I8(v) => write!(f, "I8({v})"),
            Self::U8(v) => write!(f, "U8({v})"),
            Self::I16(v) => write!(f, "I16({v})"),
            Self::U16(v) => write!(f, "U16({v})"),
            Self::I32(v) => write!(f, "I32({v})"),
            Self::U32(v) => write!(f, "U32({v})"),
            Self::I64(v) => write!(f, "I64({v})"),
            Self::U64(v) => write!(f, "U64({v})"),
            Self::F32(v) => write!(f, "F32({v})"),
            Self::F64(v) => write!(f, "F64({v})"),
            Self::String(v) => write!(f, "String({v:?})"),
            Self::Array(v) => write!(f, "Array({})", v.len()),
            Self::Sequence(v) => write!(f, "Seq({})", v.len()),
            Self::PrimitiveArray(v) | Self::PrimitiveSeq(v) => write!(f, "{v:?}"),
            Self::Message(v) => {
                write!(f, "Message({{")?;
                for (i, (key, value)) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{key}: {value:?}")?;
                }
                write!(f, "}}")
            }
        }
    }
}

/// How we resolve a [`ComplexType`] at runtime.
pub trait TypeResolver {
    fn resolve(
        &self,
        scope: &MessageSpecification,
        ty: &ComplexType,
    ) -> Option<&MessageSpecification>;
}

/// Efficient type resolver for fully-qualified ROS message names.
pub struct MapResolver<'a> {
    /// Maps "pkg/Type" -> [`MessageSpecification`]
    absolute: HashMap<String, &'a MessageSpecification>,
}

impl<'a> MapResolver<'a> {
    pub fn new(specs: impl IntoIterator<Item = (String, &'a MessageSpecification)>) -> Self {
        let mut absolute = HashMap::new();

        for (full_name, spec) in specs {
            absolute.insert(full_name, spec);
        }

        Self { absolute }
    }
}

impl TypeResolver for MapResolver<'_> {
    fn resolve(
        &self,
        scope: &MessageSpecification,
        ty: &ComplexType,
    ) -> Option<&MessageSpecification> {
        match ty {
            ComplexType::Absolute { package, name } => {
                let full_name = format!("{package}/{name}");
                self.absolute.get(&full_name).copied()
            }
            ComplexType::Relative { name } => {
                let full_name = if let Some(package) = message_package(&scope.name) {
                    format!("{package}/{name}")
                } else {
                    name.clone()
                };
                self.absolute.get(&full_name).copied()
            }
        }
    }
}

/// Decode a CDR-encoded message into a [`Value`] by walking its [`MessageSpecification`].
pub fn decode_message<BO: CdrEndian, R: TypeResolver>(
    reader: &mut CdrReader<'_, BO>,
    spec: &MessageSpecification,
    resolver: &R,
) -> Result<Value> {
    let mut fields = BTreeMap::new();
    for field in &spec.fields {
        fields.insert(
            field.name.clone(),
            decode_value(reader, spec, &field.ty, resolver)?,
        );
    }
    Ok(Value::Message(fields))
}

fn decode_value<BO: CdrEndian, R: TypeResolver>(
    reader: &mut CdrReader<'_, BO>,
    scope: &MessageSpecification,
    ty: &Type,
    resolver: &R,
) -> Result<Value> {
    match ty {
        Type::BuiltIn(builtin) => decode_scalar(reader, builtin),

        Type::Array { ty, size } => {
            let count = match size {
                ArraySize::Fixed(len) => *len,
                ArraySize::Bounded(_) | ArraySize::Unbounded => reader.read_sequence_length()?,
            };
            let fixed = matches!(size, ArraySize::Fixed(_));
            let elem = ty.as_ref();

            if let Type::BuiltIn(builtin) = elem {
                let array = decode_primitive_array(reader, builtin, count)?;
                Ok(if fixed {
                    Value::PrimitiveArray(array)
                } else {
                    Value::PrimitiveSeq(array)
                })
            } else {
                let mut values = Vec::with_capacity(count);
                for _ in 0..count {
                    values.push(decode_value(reader, scope, elem, resolver)?);
                }
                Ok(if fixed {
                    Value::Array(values)
                } else {
                    Value::Sequence(values)
                })
            }
        }

        Type::Complex(complex) => {
            let msg = resolver
                .resolve(scope, complex)
                .ok_or_else(|| Error::Custom(format!("unknown ComplexType: {complex:?}")))?;

            // Some ROS2 schemas model enums as separate messages containing only constants.
            // On the wire, fields of those types are encoded as a single primitive value.
            match msg
                .underlying_type_if_enum_like()
                .map_err(|err| Error::Custom(err.to_string()))?
            {
                Some(builtin) => decode_scalar(reader, builtin),
                None => decode_message(reader, msg, resolver),
            }
        }
    }
}

fn decode_scalar<BO: CdrEndian>(reader: &mut CdrReader<'_, BO>, ty: &BuiltInType) -> Result<Value> {
    Ok(match ty {
        BuiltInType::Bool => Value::Bool(reader.read_bool()?),
        BuiltInType::Byte | BuiltInType::Char | BuiltInType::UInt8 => Value::U8(reader.read_u8()?),
        BuiltInType::Int8 => Value::I8(reader.read_i8()?),
        BuiltInType::Int16 => Value::I16(reader.read_i16()?),
        BuiltInType::UInt16 => Value::U16(reader.read_u16()?),
        BuiltInType::Int32 => Value::I32(reader.read_i32()?),
        BuiltInType::UInt32 => Value::U32(reader.read_u32()?),
        BuiltInType::Int64 => Value::I64(reader.read_i64()?),
        BuiltInType::UInt64 => Value::U64(reader.read_u64()?),
        BuiltInType::Float32 => Value::F32(reader.read_f32()?),
        BuiltInType::Float64 => Value::F64(reader.read_f64()?),
        BuiltInType::String(_) => Value::String(reader.read_string()?),
        // `wstring` is UTF-16 on the wire, a different layout than `string`. Decoding it as UTF-8
        // would corrupt the rest of the message, so reject it. Channels with `wstring` are normally
        // kept as raw data before reaching here.
        BuiltInType::WString(_) => {
            return Err(Error::Custom(
                "ROS 2 `wstring` decoding is not supported".to_owned(),
            ));
        }
    })
}

fn decode_primitive_array<BO: CdrEndian>(
    reader: &mut CdrReader<'_, BO>,
    elem: &BuiltInType,
    count: usize,
) -> Result<PrimitiveArray> {
    Ok(match elem {
        BuiltInType::Bool => PrimitiveArray::Bool(
            (0..count)
                .map(|_| reader.read_bool())
                .collect::<Result<_>>()?,
        ),
        BuiltInType::Byte | BuiltInType::Char | BuiltInType::UInt8 => {
            PrimitiveArray::U8(reader.read_numeric_vec(count)?)
        }
        BuiltInType::Int8 => PrimitiveArray::I8(reader.read_numeric_vec(count)?),
        BuiltInType::Int16 => PrimitiveArray::I16(reader.read_numeric_vec(count)?),
        BuiltInType::UInt16 => PrimitiveArray::U16(reader.read_numeric_vec(count)?),
        BuiltInType::Int32 => PrimitiveArray::I32(reader.read_numeric_vec(count)?),
        BuiltInType::UInt32 => PrimitiveArray::U32(reader.read_numeric_vec(count)?),
        BuiltInType::Int64 => PrimitiveArray::I64(reader.read_numeric_vec(count)?),
        BuiltInType::UInt64 => PrimitiveArray::U64(reader.read_numeric_vec(count)?),
        BuiltInType::Float32 => PrimitiveArray::F32(reader.read_numeric_vec(count)?),
        BuiltInType::Float64 => PrimitiveArray::F64(reader.read_numeric_vec(count)?),
        BuiltInType::String(_) => PrimitiveArray::String(
            (0..count)
                .map(|_| reader.read_string())
                .collect::<Result<_>>()?,
        ),
        BuiltInType::WString(_) => {
            return Err(Error::Custom(
                "ROS 2 `wstring` decoding is not supported".to_owned(),
            ));
        }
    })
}
