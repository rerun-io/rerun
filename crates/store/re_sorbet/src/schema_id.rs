/// Identifies an Arrow schema by the SHA-256 of its canonical form.
///
/// The canonical form has its fields sorted, its top-level metadata dropped, and is encoded as
/// Arrow IPC. Equal schemas have equal hashes, whichever layer or dataset they come from.
#[repr(transparent)]
#[derive(
    Clone,
    Copy,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    re_byte_size::SizeBytes,
    bytemuck::AnyBitPattern,
    bytemuck::NoUninit,
)]
pub struct SchemaId([u8; 32]);

/// A map keyed by [`SchemaId`].
///
/// The hasher is randomized: IDs can come from unverified input such as uploaded footers, and
/// IDs crafted to collide would make every insertion slow.
pub type SchemaIdMap<T> = ahash::HashMap<SchemaId, T>;

/// A set of [`SchemaId`]s, with the same randomized hasher as [`SchemaIdMap`].
pub type SchemaIdSet = ahash::HashSet<SchemaId>;

// Unverified IDs must not be hashed without a seed: see `SchemaIdMap`.
static_assertions::assert_not_impl_any!(SchemaId: nohash_hasher::IsEnabled);

impl SchemaId {
    /// Length of a SHA-256 digest.
    pub const NUM_BYTES: usize = 32;

    /// Wraps a digest computed elsewhere, e.g. read back from storage or the wire.
    #[inline]
    pub fn from_sha256(sha256: [u8; Self::NUM_BYTES]) -> Self {
        Self(sha256)
    }

    /// The raw 32-byte digest.
    #[inline]
    pub fn sha256(&self) -> &[u8; Self::NUM_BYTES] {
        &self.0
    }
}

impl From<[u8; Self::NUM_BYTES]> for SchemaId {
    #[inline]
    fn from(sha256: [u8; Self::NUM_BYTES]) -> Self {
        Self(sha256)
    }
}

impl From<SchemaId> for [u8; SchemaId::NUM_BYTES] {
    #[inline]
    fn from(schema_id: SchemaId) -> Self {
        schema_id.0
    }
}

// Make `quiver::Column<SchemaId>` work (backed by a `FixedSizeBinary(32)` column):
quiver::newtype_data_type!(SchemaId, quiver::FixedSizeBinary<32>, primitive);

impl std::fmt::Display for SchemaId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}

impl std::fmt::Debug for SchemaId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SchemaId({self})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiver_roundtrip() {
        let (low, high) = (SchemaId::from([1; 32]), SchemaId::from([2; 32]));
        let array = quiver::TypedArray::<SchemaId>::from_values([low, high]);
        assert_eq!(array.as_slice(), &[low, high]);
    }
}
