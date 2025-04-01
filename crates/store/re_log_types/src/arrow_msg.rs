//! [`ArrowMsg`] is the [`crate::LogMsg`] sub-type containing an Arrow payload.
//!
//! We have custom implementations of [`serde::Serialize`] and [`serde::Deserialize`] that wraps
//! the inner Arrow serialization of an [`ArrowRecordBatch`].

use std::sync::Arc;

use arrow::array::RecordBatch as ArrowRecordBatch;

use crate::TimePoint;

/// An arbitrary callback to be run when an [`ArrowMsg`], and more specifically the
/// [`ArrowRecordBatch`] within it, goes out of scope.
///
/// If the [`ArrowMsg`] has been cloned in a bunch of places, the callback will run for each and
/// every instance.
/// It is up to the callback implementer to handle this, if needed.
//
// TODO(#6412): probably don't need this anymore.
#[allow(clippy::type_complexity)]
#[derive(Clone)]
pub struct ArrowRecordBatchReleaseCallback(Arc<dyn Fn(ArrowRecordBatch) + Send + Sync>);

impl std::ops::Deref for ArrowRecordBatchReleaseCallback {
    type Target = dyn Fn(ArrowRecordBatch) + Send + Sync;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

impl<F> From<F> for ArrowRecordBatchReleaseCallback
where
    F: Fn(ArrowRecordBatch) + Send + Sync + 'static,
{
    #[inline]
    fn from(f: F) -> Self {
        Self(Arc::new(f))
    }
}

impl ArrowRecordBatchReleaseCallback {
    #[inline]
    fn as_ptr(&self) -> *const () {
        Arc::as_ptr(&self.0).cast::<()>()
    }
}

impl PartialEq for ArrowRecordBatchReleaseCallback {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ArrowRecordBatchReleaseCallback {}

impl std::fmt::Debug for ArrowRecordBatchReleaseCallback {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ArrowRecordBatchReleaseCallback")
            .field(&format!("{:p}", self.as_ptr()))
            .finish()
    }
}

/// Message containing an Arrow payload
#[derive(Clone, Debug, PartialEq)]
#[must_use]
pub struct ArrowMsg {
    /// Unique identifier for the chunk in this message.
    pub chunk_id: re_tuid::Tuid,

    /// The maximum values for all timelines across the entire batch of data.
    ///
    /// Used to timestamp the batch as a whole for e.g. latency measurements without having to
    /// deserialize the arrow payload.
    pub timepoint_max: TimePoint,

    /// Schema and data for all control & data columns.
    pub batch: ArrowRecordBatch,

    pub on_release: Option<ArrowRecordBatchReleaseCallback>,
}

impl Drop for ArrowMsg {
    fn drop(&mut self) {
        if let Some(on_release) = self.on_release.take() {
            (*on_release)(self.batch.clone() /* shallow */);
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
        use serde::ser::SerializeTuple as _;

        let mut ipc_bytes = Vec::<u8>::new();

        let mut writer =
            arrow::ipc::writer::StreamWriter::try_new(&mut ipc_bytes, self.batch.schema_ref())
                .map_err(|err| serde::ser::Error::custom(err.to_string()))?;
        writer
            .write(&self.batch)
            .map_err(|err| serde::ser::Error::custom(err.to_string()))?;
        writer
            .finish()
            .map_err(|err| serde::ser::Error::custom(err.to_string()))?;

        let mut inner = serializer.serialize_tuple(3)?;
        inner.serialize_element(&self.chunk_id)?;
        inner.serialize_element(&self.timepoint_max)?;
        inner.serialize_element(&serde_bytes::ByteBuf::from(ipc_bytes))?;
        inner.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ArrowMsg {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
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

                let table_id: Option<re_tuid::Tuid> = seq.next_element()?;
                let timepoint_max: Option<TimePoint> = seq.next_element()?;
                let ipc_bytes: Option<serde_bytes::ByteBuf> = seq.next_element()?;

                if let (Some(chunk_id), Some(timepoint_max), Some(buf)) =
                    (table_id, timepoint_max, ipc_bytes)
                {
                    use arrow::ipc::reader::StreamReader;

                    let stream = StreamReader::try_new(std::io::Cursor::new(buf), None)
                        .map_err(|err| serde::de::Error::custom(format!("Arrow error: {err}")))?;
                    let batches: Result<Vec<_>, _> = stream.collect();

                    let batches = batches
                        .map_err(|err| serde::de::Error::custom(format!("Arrow error: {err}")))?;

                    if batches.is_empty() {
                        return Err(serde::de::Error::custom("No RecordBatch in stream"));
                    }
                    if batches.len() > 1 {
                        return Err(serde::de::Error::custom(format!(
                            "Found {} batches in stream - expected just one.",
                            batches.len()
                        )));
                    }
                    #[allow(clippy::unwrap_used)] // is_empty check above
                    let batch = batches.into_iter().next().unwrap();

                    Ok(ArrowMsg {
                        chunk_id,
                        timepoint_max,
                        batch,
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
