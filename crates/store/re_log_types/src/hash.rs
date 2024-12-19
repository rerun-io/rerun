/// 64-bit hash.
///
/// 10^-12 collision risk with   6k values.
/// 10^-9  collision risk with 190k values.
/// 10^-6  collision risk with   6M values.
/// 10^-3  collision risk with 200M values.
#[derive(Copy, Clone, Eq, PartialOrd, Ord)]
pub struct Hash64(u64);

impl re_byte_size::SizeBytes for Hash64 {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}

impl Hash64 {
    pub const ZERO: Self = Self(0);

    pub fn hash(value: impl std::hash::Hash + Copy) -> Self {
        Self(hash(value))
    }

    /// From an existing u64. Use this only for data conversions.
    #[inline]
    pub fn from_u64(i: u64) -> Self {
        Self(i)
    }

    #[inline]
    pub fn hash64(&self) -> u64 {
        self.0
    }
}

impl std::hash::Hash for Hash64 {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.0);
    }
}

impl std::cmp::PartialEq for Hash64 {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl nohash_hasher::IsEnabled for Hash64 {}

impl std::fmt::Debug for Hash64 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("Hash64({:016X})", self.0))
    }
}

// ----------------------------------------------------------------------------

pub const HASH_RANDOM_STATE: ahash::RandomState = ahash::RandomState::with_seeds(0, 1, 2, 3);

/// Hash the given value.
#[inline]
fn hash(value: impl std::hash::Hash) -> u64 {
    // Don't use ahash::AHasher::default() since it uses a random number for seeding the hasher on every application start.
    HASH_RANDOM_STATE.hash_one(value)
}
