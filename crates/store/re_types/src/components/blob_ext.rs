use super::Blob;

impl From<Vec<u8>> for Blob {
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes.into())
    }
}

impl Default for Blob {
    #[inline]
    fn default() -> Self {
        Self(Vec::new().into())
    }
}
