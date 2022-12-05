use arrow2::{array::Array, chunk::Chunk, datatypes::Schema};

use crate::MsgId;

/// The message sent to specify the data of a single field of an object.
#[must_use]
#[derive(Clone, Debug)]
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
        inner.serialize_element(buf.as_slice())?;
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
                let buf: Option<&[u8]> = seq.next_element()?;

                if let (Some(msg_id), Some(buf)) = (msg_id, buf) {
                    let mut cursor = std::io::Cursor::new(buf);
                    let metadata = read_stream_metadata(&mut cursor).unwrap();
                    let mut stream = StreamReader::new(cursor, metadata, None);
                    let chunk = stream
                        .find_map(|state| match state {
                            Ok(StreamState::Some(chunk)) => Some(chunk),
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

#[cfg(test)]
impl PartialEq for ArrowMsg {
    fn eq(&self, other: &Self) -> bool {
        self.msg_id == other.msg_id
    }
}
