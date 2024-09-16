use crate::components::{self, MediaType};

use crate::archetypes;

use super::Asset3D;

impl Asset3D {
    /// Creates a new [`Asset3D`] from the file contents at `path`.
    ///
    /// The [`MediaType`] will first be guessed from the file extension, then from the file
    /// contents if needed.
    ///
    /// If no [`MediaType`] can be guessed at the moment, the Rerun Viewer will try to guess one
    /// from the data at render-time. If it can't, rendering will fail with an error.
    ///
    /// Returns an error if the file cannot be read.
    #[cfg(not(target_arch = "wasm32"))]
    #[inline]
    pub fn from_file(filepath: impl AsRef<std::path::Path>) -> std::io::Result<Self> {
        let filepath = filepath.as_ref();
        let contents = std::fs::read(filepath)?;
        Ok(Self::from_file_contents(
            contents,
            MediaType::guess_from_path(filepath),
        ))
    }

    /// Creates a new [`Asset3D`] from the given `contents`.
    ///
    /// The [`MediaType`] will be guessed from magic bytes in the data.
    ///
    /// If no [`MediaType`] can be guessed at the moment, the Rerun Viewer will try to guess one
    /// from the data at render-time. If it can't, rendering will fail with an error.
    #[inline]
    pub fn from_file_contents(contents: Vec<u8>, media_type: Option<impl Into<MediaType>>) -> Self {
        let media_type = media_type.map(Into::into);
        Self {
            blob: contents.into(),
            media_type,
            albedo_factor: None,
            albedo_texture_buffer: None,
            albedo_texture_format: None,
        }
    }

    /// Use this image as the albedo texture.
    pub fn with_albedo_texture_image(self, image: impl Into<archetypes::Image>) -> Self {
        let image = image.into();
        self.with_albedo_texture_format(image.format)
            .with_albedo_texture_buffer(image.buffer)
    }

    /// Use this image as the albedo texture.
    pub fn with_albedo_texture(
        self,
        image_format: impl Into<components::ImageFormat>,
        image_buffer: impl Into<components::ImageBuffer>,
    ) -> Self {
        self.with_albedo_texture_format(image_format)
            .with_albedo_texture_buffer(image_buffer)
    }
}
