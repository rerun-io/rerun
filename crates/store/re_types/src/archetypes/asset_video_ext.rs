use crate::components::MediaType;

use super::AssetVideo;

/// Errors that can occur when calling [`AssetVideo::extract_frame_timestamps`].
#[cfg(feature = "video")]
#[derive(thiserror::Error, Debug)]
pub enum TimeStampExtractionError {
    #[error("Failed to determine media type from data")]
    FailedToDetermineMediaTypeFromData,

    #[error("Media type {0} is not supported.")]
    UnsupportedMediaType(String),

    #[error(transparent)]
    VideoLoadError(#[from] re_video::VideoLoadError),
}

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
        let media_type = MediaType::or_guess_from_data(media_type, &contents);
        Self {
            blob: contents.into(),
            media_type,
        }
    }

    /// Determines the presentation timestamps of all frames inside the video.
    ///
    /// Returned timestamps are in nanoseconds since start and are guaranteed to be monotonically increasing.
    #[cfg(feature = "video")]
    pub fn read_frame_timestamps_ns(
        &self,
    ) -> Result<Vec<crate::components::VideoTimestamp>, TimeStampExtractionError> {
        let media_type = if let Some(media_type) = self.media_type.as_ref() {
            media_type.clone()
        } else {
            MediaType::guess_from_data(self.blob.as_slice())
                .ok_or(TimeStampExtractionError::FailedToDetermineMediaTypeFromData)?
        };

        let video = if media_type == MediaType::mp4() {
            // TODO(andreas, jan): Should not copy all the contents just to determine the samples.
            // -> should provide a mode that doesn't do that or (even better!) only store slices into a shared buffer.
            re_video::load_mp4(self.blob.as_slice())?
        } else {
            return Err(TimeStampExtractionError::UnsupportedMediaType(
                media_type.to_string(),
            ));
        };

        Ok(video
            .segments
            .iter()
            .flat_map(|seg| {
                seg.samples.iter().map(|sample| {
                    crate::components::VideoTimestamp::from_nanoseconds(
                        sample.timestamp.as_nanoseconds(),
                    )
                })
            })
            .collect())
    }
}
