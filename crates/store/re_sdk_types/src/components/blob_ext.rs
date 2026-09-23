use super::Blob;

impl Default for Blob {
    /// An empty blob, which is also the placeholder the viewer shows for a missing blob.
    fn default() -> Self {
        Self(crate::encodings::Blob::from(Vec::<u8>::new()))
    }
}
