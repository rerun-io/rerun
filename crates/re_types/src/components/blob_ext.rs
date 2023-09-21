use super::Blob;

impl<B: Into<crate::ArrowBuffer<u8>>> From<B> for Blob {
    fn from(bytes: B) -> Self {
        Self(bytes.into())
    }
}
