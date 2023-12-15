use crate::components::MediaType;

use super::Asset3D;

impl Asset3D {
    /// Creates a new [`Asset3D`] from the file contents at `path`.
    ///
    /// The [`MediaType`] will first be guessed from the file extension, then from the file
    /// contents if needed.
    ///
    /// If no [`MediaType`] can be guessed at the moment, the Rerun Viewer will try to guess one
    /// from the data at render-time. If it can't, rendering will fail with an error.
    #[cfg(not(target_arch = "wasm32"))]
    #[inline]
    pub fn from_file(filepath: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        use anyhow::Context as _;
        let filepath = filepath.as_ref();
        let contents = std::fs::read(filepath)
            .with_context(|| format!("could not read file contents: {filepath:?}"))?;
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
        let media_type = MediaType::or_guess_from_data(media_type, &contents);
        Self {
            blob: contents.into(),
            media_type,
            transform: None,
        }
    }
}
