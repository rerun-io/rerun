use super::MediaType;

impl MediaType {
    /// `text/plain`
    #[inline]
    pub fn plain_text() -> Self {
        Self("text/plain".into())
    }

    /// `text/markdown`
    #[inline]
    pub fn markdown() -> Self {
        Self("text/markdown".into())
    }
}
