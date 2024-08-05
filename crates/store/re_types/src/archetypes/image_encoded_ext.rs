use crate::components::Blob;

use super::ImageEncoded;

impl ImageEncoded {
    /// Creates a new image from the file contents at `path`.
    ///
    /// The [`MediaType`][crate::components::MediaType] will first be guessed from the file contents.
    ///
    /// Returns an error if the file cannot be read.
    #[cfg(not(target_arch = "wasm32"))]
    #[inline]
    pub fn from_file(filepath: impl AsRef<std::path::Path>) -> std::io::Result<Self> {
        let filepath = filepath.as_ref();
        let contents = std::fs::read(filepath)?;
        Ok(Self::from_file_contents(contents))
    }

    /// Construct an image given the encoded content of some image file, e.g. a PNG or JPEG.
    ///
    /// [`Self::media_type`] will be guessed from the bytes.
    pub fn from_file_contents(bytes: Vec<u8>) -> Self {
        #[allow(clippy::unnecessary_struct_initialization)]
        Self {
            #[cfg(feature = "image")]
            media_type: image::guess_format(&bytes)
                .ok()
                .map(|format| crate::components::MediaType::from(format.to_mime_type())),

            ..Self::new(Blob::from(bytes))
        }
    }
}
