use super::TextLogColumn;

impl Default for TextLogColumn {
    #[inline]
    fn default() -> Self {
        Self::EntityPath
    }
}

impl TextLogColumn {
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
