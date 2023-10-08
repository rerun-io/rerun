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
    pub fn from_file(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        use anyhow::Context as _;
        let path = path.as_ref();
        let data = std::fs::read(path)
            .with_context(|| format!("could not read file contents: {path:?}"))?;
        Ok(Self::from_bytes(data, MediaType::guess_from_path(path)))
    }

    /// Creates a new [`Asset3D`] from the given `bytes`.
    ///
    /// The [`MediaType`] will be guessed from magic bytes in the data.
    ///
    /// If no [`MediaType`] can be guessed at the moment, the Rerun Viewer will try to guess one
    /// from the data at render-time. If it can't, rendering will fail with an error.
    #[inline]
    pub fn from_bytes(bytes: impl AsRef<[u8]>, media_type: Option<impl Into<MediaType>>) -> Self {
        let bytes = bytes.as_ref();
        let media_type = media_type.map(Into::into);
        Self {
            blob: bytes.to_vec().into(),
            media_type: MediaType::or_guess_from_data(media_type, bytes),
            transform: None,
        }
    }
}
