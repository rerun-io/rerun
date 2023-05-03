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
    // Don't use ahash::AHasher::default() since it uses a random number for seeding the hasher on every application start.
    let mut hasher =
        std::hash::BuildHasher::build_hasher(&ahash::RandomState::with_seeds(0, 1, 2, 3));
    value.hash(&mut hasher);
    hasher.finish()
}

// ----------------------------------------------------------------------------

#[derive(Copy, Clone, Eq)]
pub struct InternedString {
    hash: u64, // TODO(emilk): consider removing the hash from the `InternedString` (benchmark!)
    string: &'static str,
}

impl InternedString {
    #[inline]
    pub fn new(string: &str) -> Self {
        global_intern(string)
    }

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

impl From<&str> for InternedString {
    #[inline]
    fn from(string: &str) -> Self {
        Self::new(string)
    }
}

impl From<String> for InternedString {
    #[inline]
    fn from(string: String) -> Self {
        Self::new(&string)
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
        self.as_str()
    }
}

impl std::ops::Deref for InternedString {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
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
    #[inline]
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_str().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for InternedString {
    #[inline]
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

    pub fn bytes_used(&self) -> usize {
        // size_of_val takes references to what it wants to measure,
        // and that is wat `ier()` gives us, so this is all correct.
        self.map
            .iter()
            .map(|(k, v): (_, &&str)| {
                std::mem::size_of_val(k) + std::mem::size_of::<&str>() + v.len()
            })
            .sum()
    }
}

// ----------------------------------------------------------------------------

/// Declare a newtype wrapper around [`InternedString`] with
/// all the convenience methods you would want.
///
/// Usage:
/// ```
/// re_string_interner::declare_new_type!(
///     /// My typesafe string
///     pub struct MyString;
/// );
/// ```
#[macro_export]
macro_rules! declare_new_type {
    (
        $(#[$meta:meta])* // capture docstrings; see https://stackoverflow.com/questions/33999341/generating-documentation-in-macros
        $vis:vis struct $StructName:ident;
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
        #[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
        pub struct $StructName($crate::InternedString);

        impl $StructName {
            #[inline]
            pub fn new(string: &str) -> Self {
                Self($crate::InternedString::new(string))
            }

            #[inline]
            pub fn as_str(&self) -> &'static str {
                self.0.as_str()
            }

            /// Precomputed hash of the string.
            #[inline]
            pub fn hash(&self) -> u64 {
                self.0.hash()
            }
        }

        impl nohash_hasher::IsEnabled for $StructName {}

        impl From<&str> for $StructName {
            #[inline]
            fn from(string: &str) -> Self {
                Self::new(string)
            }
        }

        impl From<String> for $StructName {
            #[inline]
            fn from(string: String) -> Self {
                Self::new(&string)
            }
        }

        impl AsRef<str> for $StructName {
            #[inline]
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl std::borrow::Borrow<str> for $StructName {
            #[inline]
            fn borrow(&self) -> &str {
                self.as_str()
            }
        }

        impl std::ops::Deref for $StructName {
            type Target = str;
            #[inline]
            fn deref(&self) -> &str {
                self.as_str()
            }
        }

        impl std::fmt::Debug for $StructName {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.as_str().fmt(f)
            }
        }

        impl std::fmt::Display for $StructName {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.as_str().fmt(f)
            }
        }

        impl<'a> PartialEq<&'a str> for $StructName {
            fn eq(&self, other: &&'a str) -> bool {
                self.as_str() == *other
            }
        }

        impl<'a> PartialEq<$StructName> for &'a str {
            fn eq(&self, other: &$StructName) -> bool {
                *self == other.as_str()
            }
        }
    };
}

// ----------------------------------------------------------------------------

use once_cell::sync::Lazy;
use parking_lot::Mutex;
static GLOBAL_INTERNER: Lazy<Mutex<StringInterner>> =
    Lazy::new(|| Mutex::new(StringInterner::default()));

pub fn bytes_used() -> usize {
    GLOBAL_INTERNER.lock().bytes_used()
}

/// global interning function.
fn global_intern(string: &str) -> InternedString {
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

#[test]
fn test_newtype_macro() {
    #![allow(dead_code)]

    declare_new_type!(
        /// My typesafe string
        pub struct MyString;
    );
    let a = MyString::new("test");
    let b = MyString::new("test");
    assert_eq!(a, b);
    assert_eq!(a.as_str(), "test");
}
