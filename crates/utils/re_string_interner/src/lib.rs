//! Yet another string interning library.
//!
//! The main thing that makes this library different is that
//! [`InternedString`] stores the hash of the string, which makes
//! using it in lookups is really fast, especially when using [`nohash_hasher::IntMap`].
//!
//! The hash is assumed to be perfect, which means this library accepts the risk of hash collisions!
//!
//! The interned strings are never freed, so don't intern too many things.

pub mod external {
    pub use nohash_hasher;
    pub use paste;
    pub use serde;
}

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

#[derive(Copy, Clone, Eq, re_byte_size::SizeBytes)]
pub struct InternedString {
    hash: u64, // TODO(emilk): consider removing the hash from the `InternedString` (benchmark!)
    string: &'static str,
}

static_assertions::assert_not_impl_any!(InternedString: std::borrow::Borrow<str>);

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

impl From<&String> for InternedString {
    #[inline]
    fn from(string: &String) -> Self {
        Self::new(string)
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
        state.write_u64(self.hash);
    }
}

impl nohash_hasher::IsEnabled for InternedString {}

impl std::cmp::PartialOrd for InternedString {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::cmp::Ord for InternedString {
    #[inline]
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

impl serde::Serialize for InternedString {
    #[inline]
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_str().serialize(serializer)
    }
}

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
    #[cfg_attr(not(test), expect(dead_code))] // only used in tests
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

/// Intern a string literal once and return the cached value on subsequent calls.
///
/// Use for hot paths that produce the same interned identifier every call. Without this,
/// each call hashes the literal and locks the global interner, which adds up on per-frame
/// invocations from visualizer `execute` methods, codegen accessors, etc.
///
/// `$ty` must be a type declared via [`declare_new_type!`] (or anything constructible via
/// `From<&'static str>`).
///
/// ```ignore
/// fn identifier() -> ViewSystemIdentifier {
///     re_string_interner::intern_static!(ViewSystemIdentifier, "Ellipsoids3D")
/// }
/// ```
#[macro_export]
macro_rules! intern_static {
    ($ty:ty, $lit:literal) => {{
        static CACHED: ::std::sync::LazyLock<$ty> =
            ::std::sync::LazyLock::new(|| <$ty as ::std::convert::From<&str>>::from($lit));
        *CACHED
    }};
}

/// Like [`intern_static!`], but for types declared via [`declare_new_type_nonempty!`].
///
/// Those types have no infallible `From<&str>`; instead they expose
/// `from_static_str`. The empty string is rejected **at compile time** here (the
/// literal is checked in a `const` context), so an empty literal is a build error
/// rather than a runtime panic.
///
/// ```ignore
/// fn identifier() -> ViewSystemIdentifier {
///     re_string_interner::intern_static_nonempty!(ViewSystemIdentifier, "Ellipsoids3D")
/// }
/// ```
///
/// A non-empty literal compiles:
/// ```
/// re_string_interner::declare_new_type_nonempty!(
///     /// A test identifier.
///     pub struct MyString;
/// );
/// let _ = re_string_interner::intern_static_nonempty!(MyString, "non_empty");
/// ```
///
/// An empty literal fails to compile:
/// ```compile_fail
/// re_string_interner::declare_new_type_nonempty!(
///     /// A test identifier.
///     pub struct MyString;
/// );
/// let _ = re_string_interner::intern_static_nonempty!(MyString, "");
/// ```
#[macro_export]
macro_rules! intern_static_nonempty {
    ($ty:ty, $lit:literal) => {{
        const _: () = assert!(!$lit.is_empty(), "empty string literal");
        static CACHED: ::std::sync::LazyLock<$ty> =
            ::std::sync::LazyLock::new(|| <$ty>::from_static_str($lit));
        *CACHED
    }};
}

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

        impl $crate::external::nohash_hasher::IsEnabled for $StructName {}

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

        impl std::ops::Deref for $StructName {
            type Target = str;

            #[inline]
            fn deref(&self) -> &str {
                self.as_str()
            }
        }

        impl std::fmt::Debug for $StructName {
            #[inline]
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.as_str().fmt(f)
            }
        }

        impl std::fmt::Display for $StructName {
            #[inline]
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.as_str().fmt(f)
            }
        }

        impl<'a> PartialEq<&'a str> for $StructName {
            #[inline]
            fn eq(&self, other: &&'a str) -> bool {
                self.as_str() == *other
            }
        }

        impl<'a> PartialEq<&'a str> for &$StructName {
            #[inline]
            fn eq(&self, other: &&'a str) -> bool {
                self.as_str() == *other
            }
        }

        impl<'a> PartialEq<$StructName> for &'a str {
            #[inline]
            fn eq(&self, other: &$StructName) -> bool {
                *self == other.as_str()
            }
        }

        impl re_byte_size::SizeBytes for $StructName {
            const IS_POD: bool = true;

            #[inline]
            fn heap_size_bytes(&self) -> u64 {
                0
            }
        }
    };
}

/// Like [`declare_new_type!`], but the string is validated.
///
/// Currently the only rule is that the string must not be empty, but validation is centralized in
/// one place (a private `validate` fn) so further rules (e.g. no whitespace) can be added later
/// without changing the public API. The generated `Invalid<StructName>Error` carries the reason
/// the string was rejected.
///
/// Compared to [`declare_new_type!`], this:
/// - does **not** implement the infallible `From<&str>` (any lifetime) / `From<String>`, nor an
///   infallible `new`;
/// - instead exposes fallible `try_new(impl AsRef<str>)` (for any borrowed or owned string) and
///   `TryFrom<String>`, returning an `Invalid<StructName>Error` on an invalid string;
/// - generates that `Invalid<StructName>Error` error type (implements [`std::error::Error`]);
/// - exposes `from_static_str(&'static str)` which **panics** on an invalid string, for use with
///   [`intern_static_nonempty!`] and other trusted compile-time literals;
/// - implements `From<&'static str>` (delegating to `from_static_str`, so it **panics** on empty),
///   which keeps `impl Into<StructName>` parameters ergonomic for trusted string literals/consts
///   while still forcing dynamic `&str`/`String` through the fallible constructors;
/// - implements a validating [`serde::Deserialize`] (empty string ⇒ error), so empty values cannot
///   sneak back in through deserialization. **Do not** add a `serde::Deserialize`/`serde::Serialize`
///   derive in the passed-in attributes — they are provided here.
///
/// Usage:
/// ```
/// re_string_interner::declare_new_type_nonempty!(
///     /// My non-empty typesafe string
///     pub struct MyString;
/// );
/// assert!(MyString::try_new("").is_err());
/// assert_eq!(MyString::try_new("hi").unwrap().as_str(), "hi");
/// assert_eq!(MyString::from("hi").as_str(), "hi"); // `From<&'static str>`, for `impl Into` ergonomics
/// ```
#[macro_export]
macro_rules! declare_new_type_nonempty {
    (
        $(#[$meta:meta])* // capture docstrings; see https://stackoverflow.com/questions/33999341/generating-documentation-in-macros
        $vis:vis struct $StructName:ident;
    ) => {
        $crate::external::paste::paste! {
            $(#[$meta])*
            #[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
            pub struct $StructName($crate::InternedString);

            #[doc = "Error returned when constructing an invalid [`" $StructName "`]."]
            #[derive(Clone, Copy, PartialEq, Eq)]
            pub struct [<Invalid $StructName Error>] {
                /// Why the string was rejected, e.g. `"must not be empty"`.
                reason: &'static str,
            }

            impl std::fmt::Display for [<Invalid $StructName Error>] {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, concat!("Invalid `", stringify!($StructName), "`: {}"), self.reason)
                }
            }

            impl std::fmt::Debug for [<Invalid $StructName Error>] {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, concat!("Invalid", stringify!($StructName), "Error({:?})"), self.reason)
                }
            }

            impl std::error::Error for [<Invalid $StructName Error>] {}

            impl $StructName {
                /// The single place where the naming rules are enforced.
                ///
                /// Currently only forbids the empty string, but this is where future rules
                /// (e.g. no whitespace) would go.
                #[inline]
                fn validate(string: &str) -> Result<(), [<Invalid $StructName Error>]> {
                    if string.is_empty() {
                        return Err([<Invalid $StructName Error>] { reason: "must not be empty" });
                    }
                    Ok(())
                }

                /// Create a new instance, failing if the string is invalid (e.g. empty).
                #[inline]
                pub fn try_new(string: impl AsRef<str>) -> Result<Self, [<Invalid $StructName Error>]> {
                    let string = string.as_ref();
                    Self::validate(string)?;
                    Ok(Self($crate::InternedString::new(string)))
                }

                /// Create from a trusted compile-time string literal.
                ///
                /// # Panics
                /// Panics if `string` is invalid (e.g. empty).
                #[inline]
                pub fn from_static_str(string: &'static str) -> Self {
                    match Self::validate(string) {
                        Ok(()) => Self($crate::InternedString::new(string)),
                        Err(err) => panic!("{err} (got {string:?})"),
                    }
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

            impl $crate::external::nohash_hasher::IsEnabled for $StructName {}

            // NOTE: no `TryFrom<&str>` / `TryFrom<&String>`: those would collide with the blanket
            // `impl<U: Into<T>> TryFrom<U> for T` in `core` once we implement `From<&'static str>`
            // below. Use the inherent `try_new` for fallible construction from borrowed strings.
            impl TryFrom<String> for $StructName {
                type Error = [<Invalid $StructName Error>];

                #[inline]
                fn try_from(string: String) -> Result<Self, Self::Error> {
                    Self::try_new(string)
                }
            }

            // Only `&'static str` (string literals / consts), so `impl Into<Self>` parameters stay
            // ergonomic for trusted compile-time values. Dynamic `&str`/`String` must go through
            // the fallible `try_new`/`TryFrom` instead.
            impl From<&'static str> for $StructName {
                /// # Panics
                /// Panics if `string` is empty.
                #[inline]
                fn from(string: &'static str) -> Self {
                    Self::from_static_str(string)
                }
            }

            impl AsRef<str> for $StructName {
                #[inline]
                fn as_ref(&self) -> &str {
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
                #[inline]
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    self.as_str().fmt(f)
                }
            }

            impl std::fmt::Display for $StructName {
                #[inline]
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    self.as_str().fmt(f)
                }
            }

            impl<'a> PartialEq<&'a str> for $StructName {
                #[inline]
                fn eq(&self, other: &&'a str) -> bool {
                    self.as_str() == *other
                }
            }

            impl<'a> PartialEq<&'a str> for &$StructName {
                #[inline]
                fn eq(&self, other: &&'a str) -> bool {
                    self.as_str() == *other
                }
            }

            impl<'a> PartialEq<$StructName> for &'a str {
                #[inline]
                fn eq(&self, other: &$StructName) -> bool {
                    *self == other.as_str()
                }
            }

            impl re_byte_size::SizeBytes for $StructName {
                const IS_POD: bool = true;

                #[inline]
                fn heap_size_bytes(&self) -> u64 {
                    0
                }
            }

            impl $crate::external::serde::Serialize for $StructName {
                #[inline]
                fn serialize<S: $crate::external::serde::Serializer>(
                    &self,
                    serializer: S,
                ) -> Result<S::Ok, S::Error> {
                    $crate::external::serde::Serialize::serialize(self.as_str(), serializer)
                }
            }

            impl<'de> $crate::external::serde::Deserialize<'de> for $StructName {
                #[inline]
                fn deserialize<D: $crate::external::serde::Deserializer<'de>>(
                    deserializer: D,
                ) -> Result<Self, D::Error> {
                    use $crate::external::serde::de::Error as _;
                    let string = <String as $crate::external::serde::Deserialize>::deserialize(
                        deserializer,
                    )?;
                    Self::try_new(string).map_err(D::Error::custom)
                }
            }
        }
    };
}

// ----------------------------------------------------------------------------

use parking_lot::Mutex;
static GLOBAL_INTERNER: std::sync::LazyLock<Mutex<StringInterner>> =
    std::sync::LazyLock::new(|| Mutex::new(StringInterner::default()));

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
    declare_new_type!(
        /// My typesafe string
        pub struct MyString;
    );
    let a = MyString::new("test");
    let b = MyString::new("test");
    assert_eq!(a, b);
    assert_eq!(a.as_str(), "test");
}

// This should never implement `Borrow`.
// See <https://github.com/rerun-io/rerun/pull/5243> for more information.
#[test]
fn do_not_implement_borrow() {
    declare_new_type!(
        /// My typesafe string
        pub struct MyString;
    );
    static_assertions::assert_not_impl_any!(MyString: std::borrow::Borrow<str>);
}

#[test]
fn test_nonempty_newtype_macro() {
    declare_new_type_nonempty!(
        /// My non-empty typesafe string
        pub struct MyNonEmptyString;
    );

    // Empty is rejected via the fallible entry points:
    assert!(MyNonEmptyString::try_new("").is_err());
    assert!(MyNonEmptyString::try_new(String::new()).is_err());
    assert!(MyNonEmptyString::try_from(String::new()).is_err());

    // Non-empty round-trips and interns:
    let a = MyNonEmptyString::try_new("test").expect("non-empty");
    let b = MyNonEmptyString::try_from("test".to_owned()).expect("non-empty");
    assert_eq!(a, b);
    assert_eq!(a.as_str(), "test");
    assert_eq!(a, "test");
    assert_eq!("test", a);

    // Trusted literal path:
    let c = MyNonEmptyString::from_static_str("test");
    assert_eq!(a, c);

    // `From<&'static str>` keeps `impl Into<_>` ergonomic:
    let d: MyNonEmptyString = "test".into();
    assert_eq!(a, d);

    fn takes(_: impl Into<MyNonEmptyString>) {}
    takes("test");

    // The error type is a real `std::error::Error` and reports why it was rejected:
    let err = MyNonEmptyString::try_new("").unwrap_err();
    let msg = std::string::ToString::to_string(&err);
    assert!(msg.contains("MyNonEmptyString"), "{msg:?}");
    assert!(msg.contains("must not be empty"), "{msg:?}");
    let _: &dyn std::error::Error = &err;
}

#[test]
#[should_panic(expected = "must not be empty")]
fn test_nonempty_from_static_str_panics_on_empty() {
    declare_new_type_nonempty!(
        /// My non-empty typesafe string
        pub struct MyNonEmptyString;
    );
    let _ = MyNonEmptyString::from_static_str("");
}

#[test]
#[should_panic(expected = "must not be empty")]
fn test_nonempty_from_empty_static_str_panics() {
    declare_new_type_nonempty!(
        /// My non-empty typesafe string
        pub struct MyNonEmptyString;
    );
    let _val: MyNonEmptyString = "".into();
}

// This should never implement `Borrow` (same as the plain macro).
#[test]
fn nonempty_do_not_implement_borrow() {
    declare_new_type_nonempty!(
        /// My non-empty typesafe string
        pub struct MyNonEmptyString;
    );
    static_assertions::assert_not_impl_any!(MyNonEmptyString: std::borrow::Borrow<str>);
}
