use std::collections::{BTreeMap, HashMap};

use serde::de::{self, DeserializeSeed};

use crate::deserialize::primitive_array::PrimitiveArraySeed;
use crate::message_spec::{ComplexType, MessageSpecification, Type};

pub mod primitive;
pub mod primitive_array;

use primitive::{PrimitiveVisitor, StringVisitor};

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
    fn resolve(&self, ty: &ComplexType) -> Option<&MessageSpecification>;
}

/// Efficient type resolver with separate maps for absolute and relative lookups.
pub struct MapResolver<'a> {
    /// Maps "pkg/Type" -> [`MessageSpecification`]
    absolute: HashMap<String, &'a MessageSpecification>,

    /// Maps "Type" -> [`MessageSpecification`]
    relative: HashMap<String, &'a MessageSpecification>,
}

impl<'a> MapResolver<'a> {
    pub fn new(specs: impl IntoIterator<Item = (String, &'a MessageSpecification)>) -> Self {
        let mut absolute = HashMap::new();
        let mut relative = HashMap::new();

        for (full_name, spec) in specs {
            if let Some((_, name)) = full_name.rsplit_once('/') {
                // This is an absolute type like "pkg/Type"
                absolute.insert(full_name.clone(), spec);
                relative.insert(name.to_owned(), spec);
            } else {
                // This is already a relative type like "Type"
                relative.insert(full_name, spec);
            }
        }

        Self { absolute, relative }
    }
}

impl TypeResolver for MapResolver<'_> {
    fn resolve(&self, ty: &ComplexType) -> Option<&MessageSpecification> {
        match ty {
            ComplexType::Absolute { package, name } => {
                let full_name = format!("{package}/{name}");
                self.absolute.get(&full_name).copied()
            }
            ComplexType::Relative { name } => self.relative.get(name).copied(),
        }
    }
}

/// Whole message (struct) in field order.
pub struct MessageSeed<'a, R: TypeResolver> {
    specification: &'a MessageSpecification,
    type_resolver: &'a R,
}

impl<'a, R: TypeResolver> MessageSeed<'a, R> {
    pub fn new(spec: &'a MessageSpecification, type_resolver: &'a R) -> Self {
        Self {
            specification: spec,
            type_resolver,
        }
    }
}

impl<'de, R: TypeResolver> DeserializeSeed<'de> for MessageSeed<'_, R> {
    type Value = Value;

    fn deserialize<D>(self, de: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        de.deserialize_tuple(
            self.specification.fields.len(),
            MessageVisitor {
                spec: self.specification,
                type_resolver: self.type_resolver,
            },
        )
    }
}

struct MessageVisitor<'a, R: TypeResolver> {
    spec: &'a MessageSpecification,
    type_resolver: &'a R,
}

impl<'de, R: TypeResolver> serde::de::Visitor<'de> for MessageVisitor<'_, R> {
    type Value = Value;

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cdr struct as fixed-length tuple")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut out = std::collections::BTreeMap::new();
        for field in &self.spec.fields {
            let v = seq
                .next_element_seed(SchemaSeed::new(&field.ty, self.type_resolver))?
                .ok_or_else(|| serde::de::Error::custom("missing struct field"))?;
            out.insert(field.name.clone(), v);
        }
        Ok(Value::Message(out))
    }
}

/// One value, driven by a [`Type`] + resolver.
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
        use crate::message_spec::ArraySize::{Bounded, Fixed, Unbounded};
        use crate::message_spec::BuiltInType::{
            Bool, Byte, Char, Float32, Float64, Int8, Int16, Int32, Int64, String, UInt8, UInt16,
            UInt32, UInt64, WString,
        };
        use crate::message_spec::Type;

        match self.ty {
            Type::BuiltIn(primitive_type) => match primitive_type {
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
                String(_bound) | WString(_bound) => {
                    de.deserialize_string(StringVisitor).map(Value::String)
                }
            },
            Type::Array { ty, size } => match size {
                Fixed(len) => {
                    // Check if this is a primitive array and use optimized path
                    if let Type::BuiltIn(prim_type) = ty.as_ref() {
                        PrimitiveArraySeed {
                            elem: prim_type,
                            fixed_len: Some(*len),
                        }
                        .deserialize(de)
                        .map(Value::PrimitiveArray)
                    } else {
                        SequenceSeed::new(ty, Some(*len), self.resolver)
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
                            .map(Value::Sequence)
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

// Sequence/array of elements.
pub(super) struct SequenceSeed<'a, R: TypeResolver> {
    elem: &'a Type,
    fixed_len: Option<usize>,
    resolver: &'a R,
}

impl<'a, R: TypeResolver> SequenceSeed<'a, R> {
    pub fn new(elem: &'a Type, fixed_len: Option<usize>, resolver: &'a R) -> Self {
        Self {
            elem,
            fixed_len,
            resolver,
        }
    }
}

impl<'de, R: TypeResolver> DeserializeSeed<'de> for SequenceSeed<'_, R> {
    type Value = Vec<Value>;

    fn deserialize<D>(self, de: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        match self.fixed_len {
            Some(len) => de.deserialize_tuple(
                len,
                SequenceVisitor {
                    elem: self.elem,
                    fixed_len: Some(len),
                    type_resolver: self.resolver,
                },
            ),
            None => de.deserialize_seq(SequenceVisitor {
                elem: self.elem,
                fixed_len: None,
                type_resolver: self.resolver,
            }),
        }
    }
}

struct SequenceVisitor<'a, R: TypeResolver> {
    elem: &'a Type,
    fixed_len: Option<usize>,
    type_resolver: &'a R,
}

impl<'de, R: TypeResolver> serde::de::Visitor<'de> for SequenceVisitor<'_, R> {
    type Value = Vec<Value>;

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cdr-encoded sequence/array")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let len = self.fixed_len.or_else(|| seq.size_hint());
        let mut out = Vec::with_capacity(len.unwrap_or(0));

        if let Some(len) = len {
            for _ in 0..len {
                let v = seq
                    .next_element_seed(SchemaSeed::new(self.elem, self.type_resolver))?
                    .ok_or_else(|| serde::de::Error::custom("short sequence"))?;
                out.push(v);
            }
        } else {
            // Fallback for truly unbounded streams
            while let Some(v) =
                seq.next_element_seed(SchemaSeed::new(self.elem, self.type_resolver))?
            {
                out.push(v);
            }
        }
        Ok(out)
    }
}
