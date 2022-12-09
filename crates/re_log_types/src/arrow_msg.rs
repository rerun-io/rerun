//! [`ArrowMsg`] is the [`crate::LogMsg`] sub-type containing an Arrow payload.
//!
//! We have custom implementations of [`serde::Serialize`] and [`serde::Deserialize`] that wraps
//! the inner Arrow serialization of [`Schema`] and [`Chunk`].

use crate::{msg_bundle::MessageBundle, MsgId};
use arrow2::{array::Array, chunk::Chunk, datatypes::Schema};

/// Message containing an Arrow payload
#[must_use]
#[derive(Clone, Debug, PartialEq)]
pub struct ArrowMsg {
    /// A unique id per [`crate::LogMsg`].
    pub msg_id: MsgId,
    /// Arrow schema
    pub schema: Schema,
    /// Arrow chunk
    pub chunk: Chunk<Box<dyn Array>>,
}

#[cfg(feature = "serde")]
impl serde::Serialize for ArrowMsg {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use arrow2::io::ipc::write::StreamWriter;
        use serde::ser::SerializeTuple;

        let mut buf = Vec::<u8>::new();
        let mut writer = StreamWriter::new(&mut buf, Default::default());
        writer
            .start(&self.schema, None)
            .map_err(|e| serde::ser::Error::custom(e.to_string()))?;
        writer
            .write(&self.chunk, None)
            .map_err(|e| serde::ser::Error::custom(e.to_string()))?;
        writer
            .finish()
            .map_err(|e| serde::ser::Error::custom(e.to_string()))?;

        let mut inner = serializer.serialize_tuple(2)?;
        inner.serialize_element(&self.msg_id)?;
        inner.serialize_element(&serde_bytes::ByteBuf::from(buf))?;
        inner.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ArrowMsg {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use arrow2::io::ipc::read::{read_stream_metadata, StreamReader, StreamState};

        struct FieldVisitor;
        impl<'de> serde::de::Visitor<'de> for FieldVisitor {
            type Value = ArrowMsg;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("Tuple Data")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let msg_id: Option<MsgId> = seq.next_element()?;
                let buf: Option<serde_bytes::ByteBuf> = seq.next_element()?;

                if let (Some(msg_id), Some(buf)) = (msg_id, buf) {
                    let mut cursor = std::io::Cursor::new(buf);
                    let metadata = read_stream_metadata(&mut cursor).unwrap();
                    let mut stream = StreamReader::new(cursor, metadata, None);
                    let chunk = stream
                        .find_map(|state| match state {
                            Ok(StreamState::Some(chunk)) => Some(chunk),
                            Ok(StreamState::Waiting) => {
                                unreachable!("cannot be waiting on a fixed buffer")
                            }
                            _ => None,
                        })
                        .ok_or_else(|| serde::de::Error::custom("No Chunk found in stream"))?;

                    Ok(ArrowMsg {
                        msg_id,
                        schema: stream.metadata().schema.clone(),
                        chunk,
                    })
                } else {
                    Err(serde::de::Error::custom("Expected msg_id and buf"))
                }
            }
        }

        deserializer.deserialize_tuple(2, FieldVisitor)
    }
}

// ----------------------------------------------------------------------------

impl TryFrom<MessageBundle<'static>> for ArrowMsg {
    type Error = anyhow::Error;

    fn try_from(bundle: MessageBundle<'static>) -> Result<Self, Self::Error> {
        let (schema, chunk) = bundle.try_into()?;

        println!(
            "ArrowMsg chunk from MessageBundle:\n{}",
            arrow2::io::print::write(
                &[chunk.clone()],
                schema
                    .fields
                    .iter()
                    .map(|f| &f.name)
                    .collect::<Vec<_>>()
                    .as_slice(),
            )
        );

        Ok(ArrowMsg {
            msg_id: MsgId::random(),
            schema,
            chunk,
        })
    }
}

// ----------------------------------------------------------------------------

#[cfg(test)]
#[cfg(feature = "serde")]
mod tests {
    use serde_test::{assert_tokens, Token};

    use super::{ArrowMsg, Chunk, MsgId, Schema};
    use crate::{
        datagen::{build_frame_nr, build_some_point2d, build_some_rects},
        msg_bundle::MessageBundle,
        TimePoint,
    };

    #[test]
    fn test_serialized_tokens() {
        let schema = Schema::default();
        let chunk = Chunk::new(vec![]);
        let msg = ArrowMsg {
            msg_id: MsgId::ZERO,
            schema,
            chunk,
        };

        assert_tokens(
            &msg,
            &[
                Token::Tuple { len: 2 },
                // MsgId portion
                Token::NewtypeStruct { name: "MsgId" },
                Token::Struct {
                    name: "Tuid",
                    len: 2,
                },
                Token::Str("time_ns"),
                Token::U64(0),
                Token::Str("inc"),
                Token::U64(0),
                Token::StructEnd,
                // Arrow buffer portion. This is flatbuffers encoded schema+chunk.
                Token::Bytes(&[
                    255, 255, 255, 255, 48, 0, 0, 0, 4, 0, 0, 0, 242, 255, 255, 255, 20, 0, 0, 0,
                    4, 0, 1, 0, 0, 0, 10, 0, 11, 0, 8, 0, 10, 0, 4, 0, 248, 255, 255, 255, 12, 0,
                    0, 0, 8, 0, 8, 0, 0, 0, 4, 0, 0, 0, 0, 0, 255, 255, 255, 255, 72, 0, 0, 0, 8,
                    0, 0, 0, 0, 0, 0, 0, 242, 255, 255, 255, 20, 0, 0, 0, 4, 0, 3, 0, 0, 0, 10, 0,
                    11, 0, 8, 0, 10, 0, 4, 0, 242, 255, 255, 255, 28, 0, 0, 0, 16, 0, 0, 0, 0, 0,
                    10, 0, 12, 0, 0, 0, 4, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    255, 255, 255, 255, 0, 0, 0, 0,
                ]),
                Token::TupleEnd,
            ],
        );
    }

    #[test]
    fn test_roundtrip_payload() {
        let mut bundle =
            MessageBundle::new("world/rects".into(), TimePoint::from([build_frame_nr(0)]));
        bundle
            .try_append_component(build_some_point2d(1).iter())
            .unwrap();
        bundle
            .try_append_component(build_some_rects(1).iter())
            .unwrap();

        let msg_in: ArrowMsg = bundle.try_into().unwrap();
        let buf = rmp_serde::to_vec(&msg_in).unwrap();
        let msg_out: ArrowMsg = rmp_serde::from_slice(&buf).unwrap();
        assert_eq!(msg_in, msg_out);
    }
}
