use crate::components::MediaType;

use super::TextDocument;

impl TextDocument {
    /// Creates a new [`TextDocument`] from a utf8 file.
    ///
    /// The media type will be inferred from the path (extension), or the contents if that fails.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_file_path(filepath: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        use anyhow::Context as _;

        let filepath = filepath.as_ref();
        let contents = std::fs::read(filepath)
            .with_context(|| format!("could not read file contents: {filepath:?}"))?;
        Self::from_file_contents(contents, MediaType::guess_from_path(filepath))
            .with_context(|| format!("could not parse file contents: {filepath:?}"))
    }

    /// Creates a new [`TextDocument`] from the contents of a utf8 file.
    ///
    /// If unspecified, the media type will be inferred from the contents.
    #[inline]
    pub fn from_file_contents(
        contents: Vec<u8>,
        media_type: Option<impl Into<MediaType>>,
    ) -> anyhow::Result<Self> {
        let media_type = media_type.map(Into::into);
        let media_type = MediaType::or_guess_from_data(media_type, &contents);
        Ok(Self {
            text: String::from_utf8(contents)?.into(),
            media_type,
        })
    }
}
