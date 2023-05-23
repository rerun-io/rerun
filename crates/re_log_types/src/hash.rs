// ----------------------------------------------------------------------------

use std::hash::BuildHasher;

/// 64-bit hash.
///
/// 10^-12 collision risk with   6k values.
/// 10^-9  collision risk with 190k values.
/// 10^-6  collision risk with   6M values.
/// 10^-3  collision risk with 200M values.
#[derive(Copy, Clone, Eq, PartialOrd, Ord)]
pub struct Hash64(u64);

impl Hash64 {
    pub const ZERO: Hash64 = Hash64(0);

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

/// 128-bit hash. Negligible risk for collision.
#[derive(Copy, Clone, Eq)]
pub struct Hash128([u64; 2]);

impl Hash128 {
    pub const ZERO: Hash128 = Hash128([0; 2]);

    pub fn hash(value: impl std::hash::Hash + Copy) -> Self {
        Self(double_hash(value))
    }

    #[inline]
    pub fn hash64(&self) -> u64 {
        self.0[0]
    }

    #[inline]
    pub fn first64(&self) -> u64 {
        self.0[0]
    }

    #[inline]
    pub fn second64(&self) -> u64 {
        self.0[1]
    }
}

impl std::hash::Hash for Hash128 {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.0[0]);
    }
}

impl std::cmp::PartialEq for Hash128 {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl nohash_hasher::IsEnabled for Hash128 {}

impl std::fmt::Debug for Hash128 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("Hash128({:016X}{:016X})", self.0[0], self.0[1]))
    }
}

// ----------------------------------------------------------------------------

pub const HASH_RANDOM_STATE: ahash::RandomState = ahash::RandomState::with_seeds(0, 1, 2, 3);

#[inline]
fn double_hash(value: impl std::hash::Hash + Copy) -> [u64; 2] {
    [hash_with_seed(value, 123), hash_with_seed(value, 456)]
}

/// Hash the given value.
#[inline]
fn hash_with_seed(value: impl std::hash::Hash, seed: u128) -> u64 {
    use std::hash::Hash as _;
    use std::hash::Hasher as _;

    // Don't use ahash::AHasher::default() since it uses a random number for seeding the hasher on every application start.
    let mut hasher = HASH_RANDOM_STATE.build_hasher();
    seed.hash(&mut hasher);
    value.hash(&mut hasher);
    hasher.finish()
}

/// Hash the given value.
#[inline]
fn hash(value: impl std::hash::Hash) -> u64 {
    use std::hash::Hasher as _;

    // Don't use ahash::AHasher::default() since it uses a random number for seeding the hasher on every application start.
    let mut hasher = HASH_RANDOM_STATE.build_hasher();
    value.hash(&mut hasher);
    hasher.finish()
}
