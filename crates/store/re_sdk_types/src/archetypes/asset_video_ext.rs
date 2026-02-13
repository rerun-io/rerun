use super::AssetVideo;
use crate::components::MediaType;

impl AssetVideo {
    /// Creates a new [`AssetVideo`] from the file contents at `path`.
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
    pub fn from_file_path(filepath: impl AsRef<std::path::Path>) -> std::io::Result<Self> {
        let filepath = filepath.as_ref();
        let contents = std::fs::read(filepath)?;
        Ok(Self::from_file_contents(
            contents,
            MediaType::guess_from_path(filepath),
        ))
    }

    /// Creates a new [`AssetVideo`] from the given `contents`.
    ///
    /// The [`MediaType`] will be guessed from magic bytes in the data.
    ///
    /// If no [`MediaType`] can be guessed at the moment, the Rerun Viewer will try to guess one
    /// from the data at render-time. If it can't, rendering will fail with an error.
    #[inline]
    pub fn from_file_contents(contents: Vec<u8>, media_type: Option<impl Into<MediaType>>) -> Self {
        let media_type = media_type.map(Into::into);
        if let Some(media_type) = MediaType::or_guess_from_data(media_type, &contents) {
            Self::new(contents).with_media_type(media_type)
        } else {
            Self::new(contents)
        }
    }

    /// Determines the presentation timestamps of all frames inside the video.
    ///
    /// Returned timestamps are in nanoseconds since start and are guaranteed to be monotonically increasing.
    ///
    /// Panics if the serialized blob data doesn't have the right datatype.
    #[cfg(feature = "video")]
    pub fn read_frame_timestamps_nanos(&self) -> Result<Vec<i64>, re_video::VideoLoadError> {
        use re_types_core::Loggable as _;

        use crate::datatypes::Blob;

        re_tracing::profile_function!();

        let Some(blob_bytes) = self.blob.as_ref().and_then(Blob::serialized_blob_as_slice) else {
            return Err(re_video::VideoLoadError::NoVideoTrack); // Error type is close enough
        };

        let Some(media_type) = self
            .media_type
            .as_ref()
            .and_then(|mt| MediaType::from_arrow(&mt.array).ok()?.first().cloned())
            .or_else(|| MediaType::guess_from_data(blob_bytes))
        else {
            return Err(re_video::VideoLoadError::UnrecognizedMimeType);
        };

        Ok(re_video::VideoDataDescription::load_from_bytes(
            blob_bytes,
            media_type.as_str(),
            "AssetVideo",
            re_log_types::external::re_tuid::Tuid::new(),
        )?
        .frame_timestamps_nanos()
        .ok_or(re_video::VideoLoadError::NoTimescale)?
        .collect())
    }
}
