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

impl std::ops::Deref for Blob {
    type Target = re_types_core::ArrowBuffer<u8>;

    #[inline]
    fn deref(&self) -> &re_types_core::ArrowBuffer<u8> {
        &self.0
    }
}
