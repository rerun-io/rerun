//! [`ArrowMsg`] is the [`crate::LogMsg`] sub-type containing an Arrow payload.
//!
//! We have custom implementations of [`serde::Serialize`] and [`serde::Deserialize`] that wraps
//! the inner Arrow serialization of [`Schema`] and [`Chunk`].

use std::sync::Arc;

use crate::{TableId, TimePoint};
use arrow2::{array::Array, chunk::Chunk, datatypes::Schema};

/// An arbitrary callback to be run when an [`ArrowMsg`], and more specifically the
/// Arrow [`Chunk`] within it, goes out of scope.
///
/// If the [`ArrowMsg`] has been cloned in a bunch of places, the callback will run for each and
/// every instance.
/// It is up to the callback implementer to handle this, if needed.
#[allow(clippy::type_complexity)]
#[derive(Clone)]
pub struct ArrowChunkReleaseCallback(Arc<dyn Fn(Chunk<Box<dyn Array>>) + Send + Sync>);

impl std::ops::Deref for ArrowChunkReleaseCallback {
    type Target = dyn Fn(Chunk<Box<dyn Array>>) + Send + Sync;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

impl<F> From<F> for ArrowChunkReleaseCallback
where
    F: Fn(Chunk<Box<dyn Array>>) + Send + Sync + 'static,
{
    #[inline]
    fn from(f: F) -> Self {
        Self(Arc::new(f))
    }
}

impl ArrowChunkReleaseCallback {
    #[inline]
    pub fn as_ptr(&self) -> *const () {
        Arc::as_ptr(&self.0).cast::<()>()
    }
}

impl PartialEq for ArrowChunkReleaseCallback {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.as_ptr(), other.as_ptr())
    }
}

impl Eq for ArrowChunkReleaseCallback {}

impl std::fmt::Debug for ArrowChunkReleaseCallback {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ArrowChunkReleaseCallback")
            .field(&format!("{:p}", self.as_ptr()))
            .finish()
    }
}

/// Message containing an Arrow payload
#[derive(Clone, Debug, PartialEq)]
#[must_use]
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

    // pub on_release: Option<Arc<dyn FnOnce() + Send + Sync>>,
    pub on_release: Option<ArrowChunkReleaseCallback>,
}

impl Drop for ArrowMsg {
    fn drop(&mut self) {
        if let Some(on_release) = self.on_release.take() {
            (*on_release)(self.chunk.clone() /* shallow */);
        }
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for ArrowMsg {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        re_tracing::profile_scope!("ArrowMsg::serialize");

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
                re_tracing::profile_scope!("ArrowMsg::deserialize");

                let table_id: Option<TableId> = seq.next_element()?;
                let timepoint_max: Option<TimePoint> = seq.next_element()?;
                let buf: Option<serde_bytes::ByteBuf> = seq.next_element()?;

                if let (Some(table_id), Some(timepoint_max), Some(buf)) =
                    (table_id, timepoint_max, buf)
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
                    let schema = metadata.schema.clone();
                    let stream = StreamReader::new(cursor, metadata, None);
                    let chunks: Result<Vec<_>, _> = stream
                        .map(|state| match state {
                            Ok(StreamState::Some(chunk)) => Ok(chunk),
                            Ok(StreamState::Waiting) => {
                                unreachable!("cannot be waiting on a fixed buffer")
                            }
                            Err(err) => Err(err),
                        })
                        .collect();

                    let chunks = chunks
                        .map_err(|err| serde::de::Error::custom(format!("Arrow error: {err}")))?;

                    if chunks.is_empty() {
                        return Err(serde::de::Error::custom("No Chunk found in stream"));
                    }
                    if chunks.len() > 1 {
                        return Err(serde::de::Error::custom(format!(
                            "Found {} chunks in stream - expected just one.",
                            chunks.len()
                        )));
                    }
                    let chunk = chunks.into_iter().next().unwrap();

                    Ok(ArrowMsg {
                        table_id,
                        timepoint_max,
                        schema,
                        chunk,
                        on_release: None,
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
