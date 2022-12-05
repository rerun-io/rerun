use serde::{de::Visitor, ser::SerializeTuple, Deserializer, Serializer};

use arrow2::{
    array::Array,
    chunk::Chunk,
    datatypes::Schema,
    io::ipc::{
        read::{read_stream_metadata, StreamReader, StreamState},
        write::StreamWriter,
    },
};

use crate::MsgId;

/// The message sent to specify the data of a single field of an object.
#[must_use]
#[derive(Clone, Debug)]
pub struct ArrowMsg {
    /// A unique id per [`LogMsg`].
    pub msg_id: MsgId,
    /// Arrow schema
    pub schema: Schema,
    /// Arrow chunk
    pub chunk: Chunk<Box<dyn Array>>,
}

impl PartialEq for ArrowMsg {
    fn eq(&self, other: &Self) -> bool {
        self.msg_id == other.msg_id
    }
}

pub fn serialize<S>(msg: &ArrowMsg, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut buf = Vec::<u8>::new();
    let mut writer = StreamWriter::new(&mut buf, Default::default());
    writer
        .start(&msg.schema, None)
        .map_err(|e| serde::ser::Error::custom(e.to_string()))?;
    writer
        .write(&msg.chunk, None)
        .map_err(|e| serde::ser::Error::custom(e.to_string()))?;
    writer
        .finish()
        .map_err(|e| serde::ser::Error::custom(e.to_string()))?;

    let mut inner = serializer.serialize_tuple(2)?;
    inner.serialize_element(&msg.msg_id)?;
    inner.serialize_element(buf.as_slice())?;
    inner.end()
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<ArrowMsg, D::Error>
where
    D: Deserializer<'de>,
{
    struct FieldVisitor;
    impl<'de> Visitor<'de> for FieldVisitor {
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
