use super::TextLogColumnKind;

impl Default for TextLogColumnKind {
    #[inline]
    fn default() -> Self {
        Self::EntityPath
    }
}

impl TextLogColumnKind {
    /// The name for what type of column this is.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Timeline(_) => "Timeline",
            Self::EntityPath => "Entity path",
            Self::LogLevel => "Level",
            Self::Body => "Body",
        }
    }
}
