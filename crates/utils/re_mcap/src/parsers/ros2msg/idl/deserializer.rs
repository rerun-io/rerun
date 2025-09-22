use anyhow::{Context, bail};
use cdr_encoding::CdrDeserializer;
use serde::de::{self, DeserializeSeed, Visitor};
use std::collections::BTreeMap;
use std::fmt;

use crate::parsers::{
    dds,
    ros2msg::idl::{ComplexType, MessageSchema, MessageSpec, Type},
};

pub fn decode_bytes(top: &MessageSchema, buf: &[u8]) -> anyhow::Result<Value> {
    // 4-byte encapsulation header
    if buf.len() < 4 {
        return bail!("short encapsulation");
    }

    let representation_identifier = dds::RepresentationIdentifier::from_bytes([buf[0], buf[1]])
        .with_context(|| "failed to parse CDR representation identifier")?;

    let mut resolver = std::collections::HashMap::new();
    for dep in &top.dependencies {
        resolver.insert(dep.name.clone(), dep);
    }
    let resolver = MapResolver(resolver);

    let seed = MessageSeed {
        spec: &top.spec,
        type_resolver: &resolver,
    };

    if representation_identifier.is_big_endian() {
        let mut de = CdrDeserializer::<byteorder::BigEndian>::new(&buf[4..]);
        seed.deserialize(&mut de)
            .with_context(|| "failed to deserialize CDR message")
    } else {
        let mut de = CdrDeserializer::<byteorder::LittleEndian>::new(&buf[4..]);
        seed.deserialize(&mut de)
            .with_context(|| "failed to deserialize CDR message")
    }
}

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
    Array(Vec<Value>), // fixed-size [N]
    Seq(Vec<Value>),   // variable-size [] or [<=N]
    Message(BTreeMap<String, Value>),
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Bool(v) => write!(f, "Bool({})", v),
            Value::I8(v) => write!(f, "I8({})", v),
            Value::U8(v) => write!(f, "U8({})", v),
            Value::I16(v) => write!(f, "I16({})", v),
            Value::U16(v) => write!(f, "U16({})", v),
            Value::I32(v) => write!(f, "I32({})", v),
            Value::U32(v) => write!(f, "U32({})", v),
            Value::I64(v) => write!(f, "I64({})", v),
            Value::U64(v) => write!(f, "U64({})", v),
            Value::F32(v) => write!(f, "F32({})", v),
            Value::F64(v) => write!(f, "F64({})", v),
            Value::String(v) => write!(f, "String({:?})", v),
            Value::Array(v) => write!(f, "Array({})", v.len()),
            Value::Seq(v) => write!(f, "Seq({})", v.len()),
            Value::Message(v) => {
                write!(f, "Message({{")?;
                for (i, (key, value)) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {:?}", key, value)?;
                }
                write!(f, "}}")
            }
        }
    }
}

/// How we resolve a [`ComplexType`] at runtime.
pub trait TypeResolver {
    fn resolve(&self, ct: &ComplexType) -> Option<&MessageSpec>;
}

/// A simple resolver backed by a map of full names ("pkg/Type" and/or "Type").
pub struct MapResolver<'a>(pub std::collections::HashMap<String, &'a MessageSpec>);

impl TypeResolver for MapResolver<'_> {
    fn resolve(&self, ct: &ComplexType) -> Option<&MessageSpec> {
        match ct {
            ComplexType::Absolute { package, name } => {
                self.0.get(&format!("{package}/{name}")).copied()
            }
            ComplexType::Relative { name } => self.0.get(name).copied(),
        }
    }
}

// One value, driven by a Type + resolver.
struct SchemaSeed<'a, R: TypeResolver> {
    ty: &'a Type,
    r: &'a R,
}
// Whole message (struct) in field order.
struct MessageSeed<'a, R: TypeResolver> {
    spec: &'a MessageSpec,
    type_resolver: &'a R,
}
// Sequence/array of elements.
struct SeqSeed<'a, R: TypeResolver> {
    elem: &'a Type,
    fixed_len: Option<usize>,
    r: &'a R,
}

struct PrimitiveVisitor<T>(std::marker::PhantomData<T>);

impl<'de> Visitor<'de> for PrimitiveVisitor<bool> {
    type Value = bool;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bool")
    }
    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E> {
        Ok(v)
    }
    fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(v != 0)
    }
}
macro_rules! impl_primitive_visitor {
    ($t:ty, $m:ident) => {
        impl<'de> Visitor<'de> for PrimitiveVisitor<$t> {
            type Value = $t;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, stringify!($t))
            }
            fn $m<E>(self, v: $t) -> Result<$t, E> {
                Ok(v)
            }
        }
    };
}
impl_primitive_visitor!(i8, visit_i8);
impl_primitive_visitor!(u8, visit_u8);
impl_primitive_visitor!(i16, visit_i16);
impl_primitive_visitor!(u16, visit_u16);
impl_primitive_visitor!(i32, visit_i32);
impl_primitive_visitor!(u32, visit_u32);
impl_primitive_visitor!(i64, visit_i64);
impl_primitive_visitor!(u64, visit_u64);
impl_primitive_visitor!(f32, visit_f32);
impl_primitive_visitor!(f64, visit_f64);

struct StringVisitor;

impl<'de> Visitor<'de> for StringVisitor {
    type Value = String;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "string")
    }
    fn visit_string<E>(self, v: String) -> Result<String, E> {
        Ok(v)
    }
    fn visit_str<E>(self, v: &str) -> Result<String, E>
    where
        E: de::Error,
    {
        Ok(v.to_owned())
    }
}

impl<'a, 'de, R: TypeResolver> DeserializeSeed<'de> for SchemaSeed<'a, R> {
    type Value = Value;
    fn deserialize<D>(self, de: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        use super::ArraySize::*;
        use super::PrimitiveType::*;
        use super::Type;

        match self.ty {
            Type::Primitive(p) => match p {
                Bool => de
                    .deserialize_bool(PrimitiveVisitor::<bool>(Default::default()))
                    .map(Value::Bool),
                Byte | UInt8 => de
                    .deserialize_u8(PrimitiveVisitor::<u8>(Default::default()))
                    .map(Value::U8), // ROS2: octet
                Char | Int8 => de
                    .deserialize_i8(PrimitiveVisitor::<i8>(Default::default()))
                    .map(Value::I8), // ROS2: char (int8)
                Float32 => de
                    .deserialize_f32(PrimitiveVisitor::<f32>(Default::default()))
                    .map(Value::F32),
                Float64 => de
                    .deserialize_f64(PrimitiveVisitor::<f64>(Default::default()))
                    .map(Value::F64),
                Int16 => de
                    .deserialize_i16(PrimitiveVisitor::<i16>(Default::default()))
                    .map(Value::I16),
                Int32 => de
                    .deserialize_i32(PrimitiveVisitor::<i32>(Default::default()))
                    .map(Value::I32),
                Int64 => de
                    .deserialize_i64(PrimitiveVisitor::<i64>(Default::default()))
                    .map(Value::I64),
                UInt16 => de
                    .deserialize_u16(PrimitiveVisitor::<u16>(Default::default()))
                    .map(Value::U16),
                UInt32 => de
                    .deserialize_u32(PrimitiveVisitor::<u32>(Default::default()))
                    .map(Value::U32),
                UInt64 => de
                    .deserialize_u64(PrimitiveVisitor::<u64>(Default::default()))
                    .map(Value::U64),
            },
            Type::String(_bound) => de.deserialize_string(StringVisitor).map(Value::String),
            Type::Array { ty, size } => match size {
                Fixed(n) => {
                    // CDR: fixed array has NO length prefix; `cdr_encoding` can be driven via tuple.
                    SeqSeed {
                        elem: ty,
                        fixed_len: Some(*n),
                        r: self.r,
                    }
                    .deserialize(de)
                    .map(Value::Array)
                }
                Bounded(_) | Unbounded => {
                    // CDR: length-prefixed sequence; serde side is a seq.
                    SeqSeed {
                        elem: ty,
                        fixed_len: None,
                        r: self.r,
                    }
                    .deserialize(de)
                    .map(Value::Seq)
                }
            },
            Type::Complex(complex_ty) => {
                let msg = self.r.resolve(complex_ty).ok_or_else(|| {
                    de::Error::custom(format!("unknown ComplexType: {complex_ty:?}"))
                })?;

                MessageSeed {
                    spec: msg,
                    type_resolver: self.r,
                }
                .deserialize(de)
            }
        }
    }
}

impl<'de, R: TypeResolver> DeserializeSeed<'de> for MessageSeed<'_, R> {
    type Value = Value;
    fn deserialize<D>(self, de: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct MessageVisitor<'a, R: TypeResolver> {
            spec: &'a MessageSpec,
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
                for f in &self.spec.fields {
                    let v = seq
                        .next_element_seed(SchemaSeed {
                            ty: &f.ty,
                            r: self.type_resolver,
                        })?
                        .ok_or_else(|| serde::de::Error::custom("missing struct field"))?;
                    out.insert(f.name.clone(), v);
                }
                Ok(Value::Message(out))
            }
        }

        de.deserialize_tuple(
            self.spec.fields.len(),
            MessageVisitor {
                spec: self.spec,
                type_resolver: self.type_resolver,
            },
        )
    }
}

// ---- Sequence/array ----
impl<'de, R: TypeResolver> DeserializeSeed<'de> for SeqSeed<'_, R> {
    type Value = Vec<Value>;
    fn deserialize<D>(self, de: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct SeqVisitor<'a, R: TypeResolver> {
            elem: &'a Type,
            fixed_len: Option<usize>,
            type_resolver: &'a R,
        }
        impl<'de, R: TypeResolver> serde::de::Visitor<'de> for SeqVisitor<'_, R> {
            type Value = Vec<Value>;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "cdr seq/array")
            }
            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut out = self.fixed_len.map(Vec::with_capacity).unwrap_or_default();
                while let Some(v) = seq.next_element_seed(SchemaSeed {
                    ty: self.elem,
                    r: self.type_resolver,
                })? {
                    out.push(v);
                }
                Ok(out)
            }
        }
        match self.fixed_len {
            Some(n) => de.deserialize_tuple(
                n,
                SeqVisitor {
                    elem: self.elem,
                    fixed_len: Some(n),
                    type_resolver: self.r,
                },
            ),
            None => de.deserialize_seq(SeqVisitor {
                elem: self.elem,
                fixed_len: None,
                type_resolver: self.r,
            }),
        }
    }
}
