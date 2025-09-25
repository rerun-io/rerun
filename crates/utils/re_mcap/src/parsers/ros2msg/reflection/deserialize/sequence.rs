use serde::de::{self, DeserializeSeed};

use crate::parsers::ros2msg::reflection::message_spec::Type;

use super::{TypeResolver, Value, schema::SchemaSeed};

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

// ---- Sequence/array ----
impl<'de, R: TypeResolver> DeserializeSeed<'de> for SequenceSeed<'_, R> {
    type Value = Vec<Value>;
    fn deserialize<D>(self, de: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        match self.fixed_len {
            Some(n) => de.deserialize_tuple(
                n,
                SequenceVisitor {
                    elem: self.elem,
                    fixed_len: Some(n),
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
        write!(f, "cdr seq/array")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let cap = self.fixed_len.or_else(|| seq.size_hint()).unwrap_or(0);
        let mut out = Vec::with_capacity(cap);

        if let Some(n) = self.fixed_len.or_else(|| seq.size_hint()) {
            for _ in 0..n {
                let v = seq
                    .next_element_seed(SchemaSeed::new(self.elem, self.type_resolver))?
                    .ok_or_else(|| serde::de::Error::custom("short sequence"))?;
                out.push(v);
            }
            Ok(out)
        } else {
            // Fallback for truly unbounded streams
            while let Some(v) =
                seq.next_element_seed(SchemaSeed::new(self.elem, self.type_resolver))?
            {
                out.push(v);
            }
            Ok(out)
        }
    }
}
