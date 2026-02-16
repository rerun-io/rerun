use arrow::array::Array as _;
use re_arrow_util::WrongDatatypeError;

use crate::Loggable as _;

/// A unique ID for a `Chunk`.
///
/// `Chunk`s are the atomic unit of ingestion, transport, storage, events and GC in Rerun.
///
/// Internally, a `Chunk` is made up of rows, which are themselves uniquely identified by
/// their [`RowId`](crate::RowId).
///
/// There is no relationship whatsoever between a [`ChunkId`] and the [`RowId`](crate::RowId)s within that chunk.
///
/// ### String format
/// Example: `chunk_182342300C5F8C327a7b4a6e5a379ac4`.
/// The "chunk_" prefix is optional when parsing.
/// See [`re_tuid`] docs for explanations of TUID namespaces.
///
/// ### Uniqueness
///
/// [`ChunkId`] are assumed unique within a single Recording.
///
/// The chunk store will treat two chunks with the same [`ChunkId`] as the same, and only keep one
/// of them (which one is kept is an arbitrary and unstable implementation detail).
///
/// This makes it easy to build and maintain secondary indices around [`RowId`](crate::RowId)s with few to no
/// extraneous state tracking.
///
/// ### Garbage collection
///
/// Garbage collection is handled at the chunk level by first ordering the chunks based on the minimum
/// [`RowId`](crate::RowId) present in each of them.
/// Garbage collection therefore happens (roughly) in the logger's wall-clock order.
///
/// This has very important implications when inserting data far into the past or into the future:
/// think carefully about your `RowId`s in these cases.
#[repr(C, align(1))]
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, bytemuck::AnyBitPattern, bytemuck::NoUninit,
)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ChunkId(pub(crate) re_tuid::Tuid);

impl std::fmt::Debug for ChunkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "chunk_{}", self.0)
    }
}

impl std::fmt::Display for ChunkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "chunk_{}", self.0)
    }
}

#[derive(thiserror::Error, Debug)]
#[error("Invalid ChunkId: {0}")]
pub struct InvalidChunkIdError(String);

impl std::str::FromStr for ChunkId {
    type Err = InvalidChunkIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let tuid_str = if let Some((namespace, tuid_str)) = s.split_once('_') {
            if namespace == "chunk" {
                tuid_str
            } else {
                return Err(InvalidChunkIdError(format!(
                    "Expected chunk_ prefix, got {s:?}"
                )));
            }
        } else {
            s
        };

        re_tuid::Tuid::from_str(tuid_str)
            .map(Self)
            .map_err(|err| InvalidChunkIdError(format!("Invalid TUID: {err}")))
    }
}

impl ChunkId {
    pub const ZERO: Self = Self(re_tuid::Tuid::ZERO);
    pub const MAX: Self = Self(re_tuid::Tuid::MAX);

    /// Create a new unique [`ChunkId`] based on the current time.
    #[expect(clippy::new_without_default)]
    #[inline]
    pub fn new() -> Self {
        Self(re_tuid::Tuid::new())
    }

    #[inline]
    pub fn from_tuid(tuid: re_tuid::Tuid) -> Self {
        Self(tuid)
    }

    #[inline]
    pub fn as_tuid(&self) -> re_tuid::Tuid {
        self.0
    }

    /// Returns the next logical [`ChunkId`].
    ///
    /// Beware: wrong usage can easily lead to conflicts.
    /// Prefer [`ChunkId::new`] when unsure.
    #[must_use]
    #[inline]
    pub fn next(&self) -> Self {
        Self(self.0.next())
    }

    /// Returns the `n`-next logical [`ChunkId`].
    ///
    /// This is equivalent to calling [`ChunkId::next`] `n` times.
    /// Wraps the monotonically increasing back to zero on overflow.
    ///
    /// Beware: wrong usage can easily lead to conflicts.
    /// Prefer [`ChunkId::new`] when unsure.
    #[must_use]
    #[inline]
    pub fn incremented_by(&self, n: u64) -> Self {
        Self(self.0.incremented_by(n))
    }

    #[inline]
    pub fn from_u128(id: u128) -> Self {
        Self(re_tuid::Tuid::from_u128(id))
    }

    pub fn arrow_from_slice(slice: &[Self]) -> arrow::array::FixedSizeBinaryArray {
        crate::tuids_to_arrow(bytemuck::cast_slice(slice))
    }

    /// None if it is the wrong datatype
    pub fn try_slice_from_arrow(
        array: &arrow::array::FixedSizeBinaryArray,
    ) -> Result<&[Self], WrongDatatypeError> {
        if array.data_type() == &Self::arrow_datatype() {
            Ok(bytemuck::cast_slice(array.value_data()))
        } else {
            Err(WrongDatatypeError {
                column_name: None,
                expected: Self::arrow_datatype().into(),
                actual: array.data_type().clone().into(),
            })
        }
    }
}

impl re_byte_size::SizeBytes for ChunkId {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}

impl std::ops::Deref for ChunkId {
    type Target = re_tuid::Tuid;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for ChunkId {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

crate::delegate_arrow_tuid!(ChunkId as "rerun.controls.ChunkId"); // Used in the Data Platform
