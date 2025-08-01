use crate::components::MediaType;
use std::path::PathBuf;
use thiserror::Error;

use super::TextDocument;

#[derive(Debug, Error)]
pub enum TextDocumentResult {
    #[error("failed to read file contents: {path:?}: {source}")]
    ReadError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid UTF-8 sequence in file contents: {source}")]
    Utf8Error {
        #[source]
        source: std::string::FromUtf8Error,
    },

    #[error("failed to parse file contents: {path:?}: {source}")]
    ParseError {
        path: PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

impl TextDocument {
    /// Creates a new [`TextDocument`] from a utf8 file.
    ///
    /// The media type will be inferred from the path (extension), or the contents if that fails.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_file_path(
        filepath: impl AsRef<std::path::Path>,
    ) -> Result<Self, TextDocumentResult> {
        let filepath = filepath.as_ref();
        let contents = std::fs::read(filepath).map_err(|e| TextDocumentResult::ReadError {
            path: filepath.to_path_buf(),
            source: e,
        })?;

        Self::from_file_contents(contents, MediaType::guess_from_path(filepath)).map_err(|e| {
            TextDocumentResult::ParseError {
                path: filepath.to_path_buf(),
                source: Box::new(e),
            }
        })
    }

    /// Creates a new [`TextDocument`] from the contents of a utf8 file.
    ///
    /// If unspecified, the media type will be inferred from the contents.
    #[inline]
    pub fn from_file_contents(
        contents: Vec<u8>,
        media_type: Option<impl Into<MediaType>>,
    ) -> Result<Self, TextDocumentResult> {
        let media_type = MediaType::or_guess_from_data(media_type.map(Into::into), &contents);
        let result = Self::new(
            String::from_utf8(contents).map_err(|e| TextDocumentResult::Utf8Error { source: e })?,
        );
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
