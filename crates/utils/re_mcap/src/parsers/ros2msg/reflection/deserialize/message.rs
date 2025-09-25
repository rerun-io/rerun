use serde::de::{self, DeserializeSeed};

use crate::parsers::ros2msg::reflection::message_spec::MessageSpecification;

use super::schema::SchemaSeed;
use super::{TypeResolver, Value};

// Whole message (struct) in field order.
pub(super) struct MessageSeed<'a, R: TypeResolver> {
    spec: &'a MessageSpecification,
    type_resolver: &'a R,
}

impl<'a, R: TypeResolver> MessageSeed<'a, R> {
    pub fn new(spec: &'a MessageSpecification, type_resolver: &'a R) -> Self {
        Self {
            spec,
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
            self.spec.fields.len(),
            MessageVisitor {
                spec: self.spec,
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
        for f in &self.spec.fields {
            let v = seq
                .next_element_seed(SchemaSeed::new(&f.ty, self.type_resolver))?
                .ok_or_else(|| serde::de::Error::custom("missing struct field"))?;
            out.insert(f.name.clone(), v);
        }
        Ok(Value::Message(out))
    }
}
