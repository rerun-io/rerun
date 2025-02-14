use arrow::array::Array;

use crate::Loggable as _;

/// A unique ID for a `Chunk`.
///
/// `Chunk`s are the atomic unit of ingestion, transport, storage, events and GC in Rerun.
///
/// Internally, a `Chunk` is made up of rows, which are themselves uniquely identified by
/// their [`RowId`].
///
/// There is no relationship whatsoever between a [`ChunkId`] and the [`RowId`]s within that chunk.
///
/// ### Uniqueness
///
/// [`ChunkId`] are assumed unique within a single Recording.
///
/// The chunk store will treat two chunks with the same [`ChunkId`] as the same, and only keep one
/// of them (which one is kept is an arbitrary and unstable implementation detail).
///
/// This makes it easy to build and maintain secondary indices around [`RowId`]s with few to no
/// extraneous state tracking.
///
/// ### Garbage collection
///
/// Garbage collection is handled at the chunk level by first ordering the chunks based on the minimum
/// [`RowId`] present in each of them.
/// Garbage collection therefore happens (roughly) in the logger's wall-clock order.
///
/// This has very important implications when inserting data far into the past or into the future:
/// think carefully about your `RowId`s in these cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ChunkId(pub(crate) re_tuid::Tuid);

impl std::fmt::Display for ChunkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::str::FromStr for ChunkId {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        re_tuid::Tuid::from_str(s).map(Self)
    }
}

impl ChunkId {
    pub const ZERO: Self = Self(re_tuid::Tuid::ZERO);
    pub const MAX: Self = Self(re_tuid::Tuid::MAX);

    /// Create a new unique [`ChunkId`] based on the current time.
    #[allow(clippy::new_without_default)]
    #[inline]
    pub fn new() -> Self {
        Self(re_tuid::Tuid::new())
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

crate::delegate_arrow_tuid!(ChunkId as "rerun.controls.ChunkId"); // Used in the dataplatform

// ---

/// A unique ID for a row's worth of data within a chunk.
///
/// There is no relationship whatsoever between a [`ChunkId`] and the [`RowId`]s within that chunk.
///
/// ### Uniqueness
///
/// Duplicated [`RowId`]s within a single recording is considered undefined behavior.
///
/// While it is benign in most cases, care has to be taken when manually crafting [`RowId`]s.
/// Ideally: don't do so and stick to [`RowId::new`] instead to avoid bad surprises.
///
/// This makes it easy to build and maintain secondary indices around [`RowId`]s with few to no
/// extraneous state tracking.
///
/// ### Query
///
/// Queries (both latest-at & range semantics) will defer to `RowId` order as a tie-breaker when
/// looking at several rows worth of data that rest at the exact same timestamp.
///
/// In pseudo-code:
/// ```text
/// rr.set_time_sequence("frame", 10)
///
/// rr.log("my_entity", point1, row_id=#1)
/// rr.log("my_entity", point2, row_id=#0)
///
/// rr.query("my_entity", at=("frame", 10))  # returns `point1`
/// ```
///
/// Think carefully about your `RowId`s when logging a lot of data at the same timestamp.
///
/// ### Garbage collection
///
/// Garbage collection is handled at the chunk level by first ordering the chunks based on the minimum
/// [`RowId`] present in each of them.
/// Garbage collection therefore happens (roughly) in the logger's wall-clock order.
///
/// This has very important implications when inserting data far into the past or into the future:
/// think carefully about your `RowId`s in these cases.
#[repr(C, align(1))]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    bytemuck::AnyBitPattern,
    bytemuck::NoUninit,
)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct RowId(pub(crate) re_tuid::Tuid);

impl std::fmt::Display for RowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::str::FromStr for RowId {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        re_tuid::Tuid::from_str(s).map(Self)
    }
}

impl RowId {
    pub const ZERO: Self = Self(re_tuid::Tuid::ZERO);
    pub const MAX: Self = Self(re_tuid::Tuid::MAX);

    /// Create a new unique [`RowId`] based on the current time.
    #[allow(clippy::new_without_default)]
    #[inline]
    pub fn new() -> Self {
        Self(re_tuid::Tuid::new())
    }

    #[inline]
    pub fn from_tuid(tuid: re_tuid::Tuid) -> Self {
        Self(tuid)
    }

    /// Returns the next logical [`RowId`].
    ///
    /// Beware: wrong usage can easily lead to conflicts.
    /// Prefer [`RowId::new`] when unsure.
    #[must_use]
    #[inline]
    pub fn next(&self) -> Self {
        Self(self.0.next())
    }

    /// Returns the `n`-next logical [`RowId`].
    ///
    /// This is equivalent to calling [`RowId::next`] `n` times.
    /// Wraps the monotonically increasing back to zero on overflow.
    ///
    /// Beware: wrong usage can easily lead to conflicts.
    /// Prefer [`RowId::new`] when unsure.
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

    /// Panics if the array is of the wrong width
    pub fn slice_from_arrow(array: &arrow::array::FixedSizeBinaryArray) -> &[Self] {
        debug_assert_eq!(array.data_type(), &Self::arrow_datatype());
        bytemuck::cast_slice(array.value_data())
    }
}

impl re_byte_size::SizeBytes for RowId {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}

impl std::ops::Deref for RowId {
    type Target = re_tuid::Tuid;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for RowId {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

crate::delegate_arrow_tuid!(RowId as "rerun.controls.RowId");
