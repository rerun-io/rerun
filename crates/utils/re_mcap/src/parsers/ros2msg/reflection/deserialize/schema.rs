use serde::de::{self, DeserializeSeed};

use crate::parsers::ros2msg::reflection::message_spec::Type;

use super::message::MessageSeed;
use super::primitive::{PrimitiveVisitor, StringVisitor};
use super::primitive_array::PrimitiveArraySeed;
use super::sequence::SequenceSeed;
use super::{TypeResolver, Value};

/// One value, driven by a Type + resolver.
pub(super) struct SchemaSeed<'a, R: TypeResolver> {
    ty: &'a Type,
    resolver: &'a R,
}

impl<'a, R: TypeResolver> SchemaSeed<'a, R> {
    pub fn new(ty: &'a Type, resolver: &'a R) -> Self {
        Self { ty, resolver }
    }
}

impl<'de, R: TypeResolver> DeserializeSeed<'de> for SchemaSeed<'_, R> {
    type Value = Value;

    fn deserialize<D>(self, de: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        use crate::parsers::ros2msg::reflection::message_spec::{
            ArraySize::{Bounded, Fixed, Unbounded},
            BuiltInType::{
                Bool, Byte, Char, Float32, Float64, Int8, Int16, Int32, Int64, String, UInt8,
                UInt16, UInt32, UInt64, WString,
            },
            Type,
        };

        match self.ty {
            Type::BuiltIn(p) => match p {
                Bool => de
                    .deserialize_bool(PrimitiveVisitor::<bool>::new())
                    .map(Value::Bool),
                Byte | UInt8 => de
                    .deserialize_u8(PrimitiveVisitor::<u8>::new())
                    .map(Value::U8), // ROS2: octet
                Char | Int8 => de
                    .deserialize_i8(PrimitiveVisitor::<i8>::new())
                    .map(Value::I8), // ROS2: char (int8)
                Float32 => de
                    .deserialize_f32(PrimitiveVisitor::<f32>::new())
                    .map(Value::F32),
                Float64 => de
                    .deserialize_f64(PrimitiveVisitor::<f64>::new())
                    .map(Value::F64),
                Int16 => de
                    .deserialize_i16(PrimitiveVisitor::<i16>::new())
                    .map(Value::I16),
                Int32 => de
                    .deserialize_i32(PrimitiveVisitor::<i32>::new())
                    .map(Value::I32),
                Int64 => de
                    .deserialize_i64(PrimitiveVisitor::<i64>::new())
                    .map(Value::I64),
                UInt16 => de
                    .deserialize_u16(PrimitiveVisitor::<u16>::new())
                    .map(Value::U16),
                UInt32 => de
                    .deserialize_u32(PrimitiveVisitor::<u32>::new())
                    .map(Value::U32),
                UInt64 => de
                    .deserialize_u64(PrimitiveVisitor::<u64>::new())
                    .map(Value::U64),
                String(_bound) => de.deserialize_string(StringVisitor).map(Value::String),
                WString(_) => Err(de::Error::custom("wstring not supported")),
            },
            Type::Array { ty, size } => match size {
                Fixed(n) => {
                    // Check if this is a primitive array and use optimized path
                    if let Type::BuiltIn(prim_type) = ty.as_ref() {
                        PrimitiveArraySeed {
                            elem: prim_type,
                            fixed_len: Some(*n),
                        }
                        .deserialize(de)
                        .map(Value::PrimitiveArray)
                    } else {
                        // CDR: fixed array has NO length prefix; `cdr_encoding` can be driven via tuple.
                        SequenceSeed::new(ty, Some(*n), self.resolver)
                            .deserialize(de)
                            .map(Value::Array)
                    }
                }
                Bounded(_) | Unbounded => {
                    // Check if this is a primitive sequence and use optimized path
                    if let Type::BuiltIn(prim_type) = ty.as_ref() {
                        PrimitiveArraySeed {
                            elem: prim_type,
                            fixed_len: None,
                        }
                        .deserialize(de)
                        .map(Value::PrimitiveSeq)
                    } else {
                        // CDR: length-prefixed sequence; serde side is a seq.
                        SequenceSeed::new(ty, None, self.resolver)
                            .deserialize(de)
                            .map(Value::Seq)
                    }
                }
            },
            Type::Complex(complex_ty) => {
                let msg = self.resolver.resolve(complex_ty).ok_or_else(|| {
                    de::Error::custom(format!("unknown ComplexType: {complex_ty:?}"))
                })?;

                MessageSeed::new(msg, self.resolver).deserialize(de)
            }
        }
    }
}
