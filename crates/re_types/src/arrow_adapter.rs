use arrow2::buffer::Buffer;

#[derive(Clone, Debug, Default)]
pub struct ArrowString(pub Buffer<u8>);

impl PartialEq for ArrowString {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for ArrowString {}

impl PartialOrd for ArrowString {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.as_str().partial_cmp(other.as_str())
    }
}

impl Ord for ArrowString {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl ArrowString {
    #[inline]
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(self.0.as_ref()).unwrap()
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
