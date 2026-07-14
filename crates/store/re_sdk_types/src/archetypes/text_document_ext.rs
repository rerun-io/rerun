use super::TextDocument;
use crate::components::MediaType;

#[derive(thiserror::Error, Debug)]
pub enum TextDocumentError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("Invalid UTF-8: {0}")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),
}

impl TextDocument {
    /// Creates a new [`TextDocument`] from a utf8 file.
    ///
    /// The media type will be inferred from the path (extension), or the contents if that fails.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_file_path(
        filepath: impl AsRef<std::path::Path>,
    ) -> Result<Self, TextDocumentError> {
        let filepath = filepath.as_ref();
        let contents = std::fs::read(filepath)?;

        Self::from_file_contents(contents, MediaType::guess_from_path(filepath))
    }

    /// Creates a new [`TextDocument`] from the contents of a utf8 file.
    ///
    /// If unspecified, the media type will be inferred from the contents.
    #[inline]
    pub fn from_file_contents(
        contents: Vec<u8>,
        media_type: Option<impl Into<MediaType>>,
    ) -> Result<Self, TextDocumentError> {
        let media_type = MediaType::or_guess_from_data(media_type.map(Into::into), &contents);
        let result = Self::new(String::from_utf8(contents)?);

        Ok(if let Some(media_type) = media_type {
            result.with_media_type(media_type)
        } else {
            result
        })
    }

    /// Creates a new [`TextDocument`] containing Markdown.
    ///
    /// Equivalent to `TextDocument::new(markdown).with_media_type(MediaType::markdown())`.
    #[inline]
    pub fn from_markdown(markdown: impl Into<crate::components::Text>) -> Self {
        Self::new(markdown).with_media_type(MediaType::markdown())
    }
}
