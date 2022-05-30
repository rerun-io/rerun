//! Yet another string interning library.
//!
//! The main thing that makes this library different is that
//! [`InternedString`] stores the hash of the string, which makes
//! using it in lookups is really fast, especially when using [`nohash_hasher::IntMap`].
//!
//! The hash is assumed to be perfect, which means this library accepts the risk of hash collisions!
//!
//! The interned strings are never freed, so don't intern too many things.

/// Fast but high quality string hash
#[inline]
fn hash(value: impl std::hash::Hash) -> u64 {
    use std::hash::Hasher as _;
    let mut hasher = ahash::AHasher::default();
    value.hash(&mut hasher);
    hasher.finish()
}

// ----------------------------------------------------------------------------

#[derive(Copy, Clone, Eq)]
pub struct InternedString {
    hash: u64,
    string: &'static str,
}

impl InternedString {
    #[inline]
    pub fn as_str(&self) -> &'static str {
        self.string
    }

    /// Precomputed hash of the string.
    #[inline]
    pub fn hash(&self) -> u64 {
        self.hash
    }
}

impl std::cmp::PartialEq for InternedString {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl std::hash::Hash for InternedString {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

impl nohash_hasher::IsEnabled for InternedString {}

impl std::cmp::PartialOrd for InternedString {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.string.partial_cmp(other.string)
    }
}

impl std::cmp::Ord for InternedString {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.string.cmp(other.string)
    }
}

impl AsRef<str> for InternedString {
    #[inline]
    fn as_ref(&self) -> &str {
        self.string
    }
}

impl std::borrow::Borrow<str> for InternedString {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_ref()
    }
}

impl std::ops::Deref for InternedString {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        self.as_ref()
    }
}

impl std::fmt::Debug for InternedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_str().fmt(f)
    }
}

impl std::fmt::Display for InternedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_str().fmt(f)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for InternedString {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_ref().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for InternedString {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(|s| global_intern(&s))
    }
}

// ----------------------------------------------------------------------------

#[derive(Default)]
struct StringInterner {
    map: nohash_hasher::IntMap<u64, &'static str>,
}

impl StringInterner {
    #[allow(dead_code)] // used in tests
    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn intern(&mut self, string: &str) -> InternedString {
        let hash = hash(string);

        let static_ref_string = self
            .map
            .entry(hash)
            .or_insert_with(|| Box::leak(Box::<str>::from(string)));

        InternedString {
            hash,
            string: static_ref_string,
        }
    }
}

// ----------------------------------------------------------------------------

/// global interning function.
pub fn global_intern(string: &str) -> InternedString {
    use once_cell::sync::Lazy;
    use parking_lot::Mutex;
    static GLOBAL_INTERNER: Lazy<Mutex<StringInterner>> =
        Lazy::new(|| Mutex::new(StringInterner::default()));

    GLOBAL_INTERNER.lock().intern(string)
}

// ----------------------------------------------------------------------------

#[test]
fn test_interner() {
    let mut interner = StringInterner::default();
    assert_eq!(interner.len(), 0);

    let a = interner.intern("Hello World!");
    assert_eq!(interner.len(), 1);

    let b = interner.intern("Hello World!");
    assert_eq!(interner.len(), 1);

    assert_eq!(a, b);

    let c = interner.intern("Another string");
    assert_eq!(interner.len(), 2);

    assert!(a.hash == b.hash);
    assert!(a.hash != c.hash);
}
