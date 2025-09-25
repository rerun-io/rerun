use std::collections::{BTreeMap, HashMap};

use anyhow::Context as _;
use cdr_encoding::CdrDeserializer;
use serde::de::DeserializeSeed as _;

use crate::parsers::{
    dds,
    ros2msg::reflection::{
        MessageSchema,
        message_spec::{ComplexType, MessageSpecification},
    },
};

pub mod message;
pub mod primitive;
pub mod primitive_array;
pub mod schema;
pub mod sequence;

pub fn decode_bytes(top: &MessageSchema, buf: &[u8]) -> anyhow::Result<Value> {
    // 4-byte encapsulation header
    if buf.len() < 4 {
        anyhow::bail!("short encapsulation");
    }

    let representation_identifier = dds::RepresentationIdentifier::from_bytes([buf[0], buf[1]])
        .with_context(|| "failed to parse CDR representation identifier")?;

    let resolver = MapResolver::new(top.dependencies.iter().map(|dep| (dep.name.clone(), dep)));

    let seed = message::MessageSeed::new(&top.spec, &resolver);

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
    Array(Vec<Value>), // fixed-size [N] - for complex types
    Seq(Vec<Value>),   // variable-size [] or [<=N] - for complex types
    PrimitiveArray(primitive_array::PrimitiveArray), // fixed-size [N] - for primitives
    PrimitiveSeq(primitive_array::PrimitiveArray), // variable-size [] or [<=N] - for primitives
    Message(BTreeMap<String, Value>),
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
            Self::Seq(v) => write!(f, "Seq({})", v.len()),
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
    fn resolve(&self, ct: &ComplexType) -> Option<&MessageSpecification>;
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
    fn resolve(&self, ct: &ComplexType) -> Option<&MessageSpecification> {
        match ct {
            ComplexType::Absolute { package, name } => {
                let full_name = format!("{package}/{name}");
                self.absolute.get(&full_name).copied()
            }
            ComplexType::Relative { name } => self.relative.get(name).copied(),
        }
    }
}
