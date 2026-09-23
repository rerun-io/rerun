use super::AssetAudio;
use crate::components::MediaType;

impl AssetAudio {
    /// Creates a new [`AssetAudio`] from the file contents at `path`.
    ///
    /// The [`MediaType`] will first be guessed from the file extension, then from the file
    /// contents if needed.
    ///
    /// If no [`MediaType`] can be guessed at the moment, the Rerun Viewer will try to guess one
    /// from the data at playback-time. If it can't, playback will fail with an error.
    ///
    /// Returns an error if the file cannot be read.
    #[cfg(not(target_arch = "wasm32"))]
    #[inline]
    pub fn from_file_path(filepath: impl AsRef<std::path::Path>) -> std::io::Result<Self> {
        let filepath = filepath.as_ref();
        let contents = std::fs::read(filepath)?;
        Ok(Self::from_file_contents(
            contents,
            MediaType::guess_from_path(filepath),
        ))
    }

    /// Creates a new [`AssetAudio`] from the given `contents`.
    ///
    /// The [`MediaType`] will be guessed from magic bytes in the data if not provided.
    ///
    /// If no [`MediaType`] can be guessed at the moment, the Rerun Viewer will try to guess one
    /// from the data at playback-time. If it can't, playback will fail with an error.
    #[inline]
    pub fn from_file_contents(contents: Vec<u8>, media_type: Option<impl Into<MediaType>>) -> Self {
        let media_type = media_type.map(Into::into);
        if let Some(media_type) = MediaType::or_guess_from_data(media_type, &contents) {
            Self::new(contents).with_media_type(media_type)
        } else {
            Self::new(contents)
        }
    }
}
