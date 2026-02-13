use arrow::array::Array as _;

use crate::Loggable as _;

/// A unique ID for a row's worth of data within a chunk.
///
/// There is no relationship whatsoever between a [`ChunkId`](crate::ChunkId) and the [`RowId`]s within that chunk.
///
/// ### String format
/// Example: `row_182342300C5F8C327a7b4a6e5a379ac4`.
/// The "row_" prefix is optional when parsing.
/// See [`re_tuid`] docs for explanations of TUID namespaces.
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
/// rr.set_time("frame", sequence=10)
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
        write!(f, "row_{}", self.0)
    }
}

#[derive(thiserror::Error, Debug)]
#[error("Invalid RowId: {0}")]
pub struct InvalidRowIdError(String);

impl std::str::FromStr for RowId {
    type Err = InvalidRowIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let tuid_str = if let Some((namespace, tuid_str)) = s.split_once('_') {
            if namespace == "row" {
                tuid_str
            } else {
                return Err(InvalidRowIdError(format!(
                    "Expected row_ prefix, got {s:?}"
                )));
            }
        } else {
            s
        };

        re_tuid::Tuid::from_str(tuid_str)
            .map(Self)
            .map_err(|err| InvalidRowIdError(format!("Invalid TUID: {err}")))
    }
}

impl RowId {
    pub const ZERO: Self = Self(re_tuid::Tuid::ZERO);
    pub const MAX: Self = Self(re_tuid::Tuid::MAX);

    /// Create a new unique [`RowId`] based on the current time.
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
        re_log::debug_assert_eq!(array.data_type(), &Self::arrow_datatype());
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

#[test]
fn test_row_id_parse() {
    let tuid: re_tuid::Tuid = "182342300C5F8C327a7b4a6e5a379ac4".parse().unwrap();

    assert_eq!(
        RowId(tuid).to_string(),
        "row_182342300C5F8C327a7b4a6e5a379ac4"
    );

    assert_eq!(
        "182342300C5F8C327a7b4a6e5a379ac4"
            .parse::<RowId>()
            .unwrap()
            .0,
        tuid
    );
    assert_eq!(
        "row_182342300C5F8C327a7b4a6e5a379ac4"
            .parse::<RowId>()
            .unwrap()
            .0,
        tuid
    );
    assert!(
        "chunk_182342300C5F8C327a7b4a6e5a379ac4"
            .parse::<RowId>()
            .is_err()
    );
}
