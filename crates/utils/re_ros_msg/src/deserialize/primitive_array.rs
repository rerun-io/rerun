use std::fmt;

use serde::de::{self, DeserializeSeed, Visitor};

use crate::message_spec::BuiltInType;

#[derive(Clone, PartialEq)]
pub enum PrimitiveArray {
    Bool(Vec<bool>),
    I8(Vec<i8>),
    U8(Vec<u8>),
    I16(Vec<i16>),
    U16(Vec<u16>),
    I32(Vec<i32>),
    U32(Vec<u32>),
    I64(Vec<i64>),
    U64(Vec<u64>),
    F32(Vec<f32>),
    F64(Vec<f64>),
    String(Vec<String>),
}

impl std::fmt::Debug for PrimitiveArray {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(v) => write!(f, "BoolArray({})", v.len()),
            Self::I8(v) => write!(f, "I8Array({})", v.len()),
            Self::U8(v) => write!(f, "U8Array({})", v.len()),
            Self::I16(v) => write!(f, "I16Array({})", v.len()),
            Self::U16(v) => write!(f, "U16Array({})", v.len()),
            Self::I32(v) => write!(f, "I32Array({})", v.len()),
            Self::U32(v) => write!(f, "U32Array({})", v.len()),
            Self::I64(v) => write!(f, "I64Array({})", v.len()),
            Self::U64(v) => write!(f, "U64Array({})", v.len()),
            Self::F32(v) => write!(f, "F32Array({})", v.len()),
            Self::F64(v) => write!(f, "F64Array({})", v.len()),
            Self::String(v) => write!(f, "StringArray({})", v.len()),
        }
    }
}

/// Specialized seed for primitive arrays (arrays/sequences of built-in types).
pub struct PrimitiveArraySeed<'a> {
    pub elem: &'a BuiltInType,
    pub fixed_len: Option<usize>,
}

macro_rules! impl_primitive_array_visitor {
    ($prim_type:ty, $array_variant:ident, $visit_method:ident) => {
        struct $array_variant;

        impl<'de> Visitor<'de> for $array_variant {
            type Value = Vec<$prim_type>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "array of {}", stringify!($prim_type))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let size_hint = seq.size_hint().unwrap_or(0);
                let mut vec = Vec::with_capacity(size_hint);

                while let Some(elem) = seq.next_element()? {
                    vec.push(elem);
                }

                Ok(vec)
            }
        }
    };
}

impl_primitive_array_visitor!(bool, BoolArrayVisitor, visit_bool);
impl_primitive_array_visitor!(i8, I8ArrayVisitor, visit_i8);
impl_primitive_array_visitor!(u8, U8ArrayVisitor, visit_u8);
impl_primitive_array_visitor!(i16, I16ArrayVisitor, visit_i16);
impl_primitive_array_visitor!(u16, U16ArrayVisitor, visit_u16);
impl_primitive_array_visitor!(i32, I32ArrayVisitor, visit_i32);
impl_primitive_array_visitor!(u32, U32ArrayVisitor, visit_u32);
impl_primitive_array_visitor!(i64, I64ArrayVisitor, visit_i64);
impl_primitive_array_visitor!(u64, U64ArrayVisitor, visit_u64);
impl_primitive_array_visitor!(f32, F32ArrayVisitor, visit_f32);
impl_primitive_array_visitor!(f64, F64ArrayVisitor, visit_f64);
impl_primitive_array_visitor!(String, StringArrayVisitor, visit_string);

impl<'de> DeserializeSeed<'de> for PrimitiveArraySeed<'_> {
    type Value = PrimitiveArray;

    fn deserialize<D>(self, de: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        use BuiltInType::{
            Bool, Byte, Char, Float32, Float64, Int8, Int16, Int32, Int64, String, UInt8, UInt16,
            UInt32, UInt64, WString,
        };

        macro_rules! deserialize_array {
            ($de:expr, $visitor:expr) => {
                match self.fixed_len {
                    Some(n) => $de.deserialize_tuple(n, $visitor),
                    None => $de.deserialize_seq($visitor),
                }
            };
        }

        match self.elem {
            Bool => deserialize_array!(de, BoolArrayVisitor).map(PrimitiveArray::Bool),
            Byte | UInt8 => deserialize_array!(de, U8ArrayVisitor).map(PrimitiveArray::U8),
            Char | Int8 => deserialize_array!(de, I8ArrayVisitor).map(PrimitiveArray::I8),
            Float32 => deserialize_array!(de, F32ArrayVisitor).map(PrimitiveArray::F32),
            Float64 => deserialize_array!(de, F64ArrayVisitor).map(PrimitiveArray::F64),
            Int16 => deserialize_array!(de, I16ArrayVisitor).map(PrimitiveArray::I16),
            Int32 => deserialize_array!(de, I32ArrayVisitor).map(PrimitiveArray::I32),
            Int64 => deserialize_array!(de, I64ArrayVisitor).map(PrimitiveArray::I64),
            UInt16 => deserialize_array!(de, U16ArrayVisitor).map(PrimitiveArray::U16),
            UInt32 => deserialize_array!(de, U32ArrayVisitor).map(PrimitiveArray::U32),
            UInt64 => deserialize_array!(de, U64ArrayVisitor).map(PrimitiveArray::U64),
            String(_) => deserialize_array!(de, StringArrayVisitor).map(PrimitiveArray::String),
            WString(_) => Err(de::Error::custom("wstring arrays not supported")),
        }
    }
}
