use arrow2::buffer::Buffer;

/// Convenience-wrapper around an arrow [`Buffer`] that is known to contain a
/// UTF-8 encoded string.
///
/// The arrow2 [`Buffer`] object is internally reference-counted and can be
/// easily converted back to a `&str` referencing the underlying storage.
/// This avoids some of the lifetime complexities that would otherwise
/// arise from returning a `&str` directly, but is significantly more
/// performant than doing the full allocation necessary to return a `String`.
#[derive(Clone, Debug, Default)]
pub struct ArrowString(Buffer<u8>);

impl re_byte_size::SizeBytes for ArrowString {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self(buf) = self;
        std::mem::size_of_val(buf.as_slice()) as _
    }
}

impl PartialEq for ArrowString {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for ArrowString {}

impl PartialOrd for ArrowString {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.as_str().cmp(other.as_str()))
    }
}

impl Ord for ArrowString {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl std::hash::Hash for ArrowString {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl ArrowString {
    #[inline]
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(self.0.as_ref()).unwrap_or("INVALID UTF-8")
    }

    #[inline]
    pub fn into_arrow_buffer(self) -> arrow::buffer::Buffer {
        self.0.into()
    }

    #[inline]
    pub fn into_arrow2_buffer(self) -> arrow2::buffer::Buffer<u8> {
        self.0
    }
}

impl From<arrow::buffer::Buffer> for ArrowString {
    #[inline]
    fn from(buf: arrow::buffer::Buffer) -> Self {
        Self(buf.into())
    }
}

impl From<arrow2::buffer::Buffer<u8>> for ArrowString {
    #[inline]
    fn from(buf: arrow2::buffer::Buffer<u8>) -> Self {
        Self(buf)
    }
}

impl From<String> for ArrowString {
    #[inline]
    fn from(value: String) -> Self {
        Self(value.as_bytes().to_vec().into())
    }
}

impl From<&str> for ArrowString {
    #[inline]
    fn from(value: &str) -> Self {
        Self(value.as_bytes().to_vec().into())
    }
}

impl std::fmt::Display for ArrowString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_str().fmt(f)
    }
}

impl AsRef<str> for ArrowString {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for ArrowString {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

#[test]
fn borrow_hash_is_self_hash() {
    use std::borrow::Borrow as _;
    use std::hash::{Hash as _, Hasher as _};

    let s = ArrowString::from("hello world");

    let self_hash = {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    };

    let borrowed_hash = {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let s: &str = s.borrow();
        s.hash(&mut hasher);
        hasher.finish()
    };

    assert_eq!(self_hash, borrowed_hash);
}

impl std::ops::Deref for ArrowString {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}
