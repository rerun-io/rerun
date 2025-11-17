use super::TextLogColumnKind;

impl TextLogColumnKind {
    /// The name for what type of column this is.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Timeline => "Timeline",
            Self::EntityPath => "Entity path",
            Self::LogLevel => "Level",
            Self::Body => "Body",
        }
    }
}
