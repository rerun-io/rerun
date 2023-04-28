//! [`ArrowMsg`] is the [`crate::LogMsg`] sub-type containing an Arrow payload.
//!
//! We have custom implementations of [`serde::Serialize`] and [`serde::Deserialize`] that wraps
//! the inner Arrow serialization of [`Schema`] and [`Chunk`].

use crate::{TableId, TimePoint};
use arrow2::{array::Array, chunk::Chunk, datatypes::Schema};

/// Message containing an Arrow payload
#[must_use]
#[derive(Clone, Debug, PartialEq)]
pub struct ArrowMsg {
    /// Unique identifier for the [`crate::DataTable`] in this message.
    pub table_id: TableId,

    /// The maximum values for all timelines across the entire batch of data.
    ///
    /// Used to timestamp the batch as a whole for e.g. latency measurements without having to
    /// deserialize the arrow payload.
    pub timepoint_max: TimePoint,

    /// Schema for all control & data columns.
    pub schema: Schema,

    /// Data for all control & data columns.
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
            .map_err(|err| serde::ser::Error::custom(err.to_string()))?;
        writer
            .write(&self.chunk, None)
            .map_err(|err| serde::ser::Error::custom(err.to_string()))?;
        writer
            .finish()
            .map_err(|err| serde::ser::Error::custom(err.to_string()))?;

        let mut inner = serializer.serialize_tuple(3)?;
        inner.serialize_element(&self.table_id)?;
        inner.serialize_element(&self.timepoint_max)?;
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
                formatter.write_str("(table_id, timepoint, buf)")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let table_id: Option<TableId> = seq.next_element()?;
                let timepoint_min: Option<TimePoint> = seq.next_element()?;
                let buf: Option<serde_bytes::ByteBuf> = seq.next_element()?;

                if let (Some(table_id), Some(timepoint_min), Some(buf)) =
                    (table_id, timepoint_min, buf)
                {
                    let mut cursor = std::io::Cursor::new(buf);
                    let metadata = match read_stream_metadata(&mut cursor) {
                        Ok(metadata) => metadata,
                        Err(err) => {
                            return Err(serde::de::Error::custom(format!(
                                "Failed to read stream metadata: {err}"
                            )))
                        }
                    };
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
                        table_id,
                        timepoint_max: timepoint_min,
                        schema: stream.metadata().schema.clone(),
                        chunk,
                    })
                } else {
                    Err(serde::de::Error::custom(
                        "Expected (table_id, timepoint, buf)",
                    ))
                }
            }
        }

        deserializer.deserialize_tuple(3, FieldVisitor)
    }
}

// ----------------------------------------------------------------------------

#[cfg(test)]
#[cfg(feature = "serde")]
mod tests {
    use super::*;

    use crate::{
        datagen::{build_frame_nr, build_some_point2d, build_some_rects},
        DataRow, DataTable, RowId,
    };

    #[test]
    fn arrow_msg_roundtrip() {
        let row = DataRow::from_cells2(
            RowId::random(),
            "world/rects",
            [build_frame_nr(0.into())],
            1,
            (build_some_point2d(1), build_some_rects(1)),
        );

        let table_in = {
            let mut table = row.into_table();
            table.compute_all_size_bytes();
            table
        };
        let msg_in = table_in.to_arrow_msg().unwrap();
        let buf = rmp_serde::to_vec(&msg_in).unwrap();
        let msg_out: ArrowMsg = rmp_serde::from_slice(&buf).unwrap();
        let table_out = {
            let mut table = DataTable::from_arrow_msg(&msg_out).unwrap();
            table.compute_all_size_bytes();
            table
        };

        assert_eq!(msg_in, msg_out);
        assert_eq!(table_in, table_out);
    }
}
