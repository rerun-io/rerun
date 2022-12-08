//! [`ArrowMsg`] is the [`crate::LogMsg`] sub-type containing an Arrow payload.
//!
//! We have custom implementations of [`serde::Serialize`] and [`serde::Deserialize`] that wraps
//! the inner Arrow serialization of [`Schema`] and [`Chunk`].

use std::collections::BTreeMap;

use arrow2::{
    array::{Array, StructArray},
    chunk::Chunk,
    datatypes::{DataType, Field, Schema},
};
use arrow2_convert::{field::ArrowField, serialize::TryIntoArrow};

use crate::{ComponentNameRef, MsgId, ObjPath, TimeInt, TimePoint, Timeline, ENTITY_PATH_KEY};

use anyhow::{anyhow, ensure};

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

pub fn pack_components(
    components: impl Iterator<Item = (Schema, Box<dyn Array>)>,
) -> (Schema, StructArray) {
    let (component_schemas, component_cols): (Vec<_>, Vec<_>) = components.unzip();
    let component_fields = component_schemas
        .into_iter()
        .flat_map(|schema| schema.fields)
        .collect();

    let packed = StructArray::new(DataType::Struct(component_fields), component_cols, None);

    let schema = Schema {
        fields: [Field::new("components", packed.data_type().clone(), false)].to_vec(),
        ..Default::default()
    };

    (schema, packed)
}

/// Build a single log message
pub fn build_message(
    ent_path: &ObjPath,
    timepoint: &TimePoint,
    components: impl IntoIterator<Item = (ComponentNameRef<'static>, Schema, Box<dyn Array>)>,
) -> (Schema, Chunk<Box<dyn Array>>) {
    let mut schema = Schema::default();
    let mut cols: Vec<Box<dyn Array>> = Vec::new();

    schema.metadata = BTreeMap::from([(ENTITY_PATH_KEY.into(), ent_path.to_string())]);

    // Build & pack timelines
    let timelines_field = Field::new("timelines", TimePoint::data_type(), false);
    let timelines_col = [timepoint].try_into_arrow().unwrap();

    schema.fields.push(timelines_field);
    cols.push(timelines_col);

    // Build & pack components
    let (components_schema, components_data) = pack_components(
        components
            .into_iter()
            .map(|(_, schema, data)| (schema, data)),
    );
    schema.fields.extend(components_schema.fields);
    schema.metadata.extend(components_schema.metadata);
    cols.push(components_data.boxed());

    (schema, Chunk::new(cols))
}

pub fn extract_message<'data>(
    schema: &'_ Schema,
    msg: &'data Chunk<Box<dyn Array>>,
) -> anyhow::Result<(
    ObjPath,
    TimePoint,
    Vec<(ComponentNameRef<'data>, &'data dyn Array)>,
)> {
    let ent_path = schema
        .metadata
        .get(ENTITY_PATH_KEY)
        .ok_or_else(|| anyhow!("expect entity path in top-level message's metadata"))
        .map(|path| ObjPath::from(path.as_str()))?;

    let time_point = extract_timelines(schema, msg)?;
    let components = extract_components(schema, msg)?;

    Ok((ent_path, time_point, components))
}

/// Extract a [`TimePoint`] from the "timelines" column
pub fn extract_timelines(
    schema: &Schema,
    msg: &Chunk<Box<dyn Array>>,
) -> anyhow::Result<TimePoint> {
    use arrow2_convert::deserialize::arrow_array_deserialize_iterator;

    let timelines = schema
        .fields
        .iter()
        .position(|f| f.name == "timelines") // TODO(cmc): maybe at least a constant or something
        .and_then(|idx| msg.columns().get(idx))
        .ok_or_else(|| anyhow!("expect top-level `timelines` field`"))?;

    let mut timepoints_iter = arrow_array_deserialize_iterator::<TimePoint>(timelines.as_ref())?;

    let timepoint = timepoints_iter
        .next()
        .ok_or_else(|| anyhow!("No rows in timelines."))?;

    ensure!(
        timepoints_iter.next().is_none(),
        "Expected a single TimePoint, but found more!"
    );

    Ok(timepoint)
}

/// Extract the components from the message
pub fn extract_components<'data>(
    schema: &Schema,
    msg: &'data Chunk<Box<dyn Array>>,
) -> anyhow::Result<Vec<(ComponentNameRef<'data>, &'data dyn Array)>> {
    let components = schema
        .fields
        .iter()
        .position(|f| f.name == "components") // TODO(cmc): maybe at least a constant or something
        .and_then(|idx| msg.columns().get(idx))
        .ok_or_else(|| anyhow!("expect top-level `components` field`"))?;

    let components = components
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or_else(|| anyhow!("expect component values to be `StructArray`s"))?;

    Ok(components
        .fields()
        .iter()
        .zip(components.values())
        .map(|(field, comp)| (field.name.as_str(), comp.as_ref()))
        .collect())
}

// ----------------------------------------------------------------------------

#[cfg(test)]
#[cfg(feature = "serde")]
mod tests {
    use serde_test::{assert_tokens, Token};

    use super::{build_message, ArrowMsg, Chunk, MsgId, Schema};
    use crate::{
        datagen::{build_frame_nr, build_positions, build_rects},
        ObjPath, TimePoint,
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
        let (schema, chunk) = build_message(
            &ObjPath::from("rects"),
            &TimePoint::from([build_frame_nr(0)]),
            [build_positions(1), build_rects(1)],
        );

        let msg_in = ArrowMsg {
            msg_id: MsgId::random(),
            schema,
            chunk,
        };

        let buf = rmp_serde::to_vec(&msg_in).unwrap();
        let msg_out: ArrowMsg = rmp_serde::from_slice(&buf).unwrap();
        assert_eq!(msg_in, msg_out);
    }
}
