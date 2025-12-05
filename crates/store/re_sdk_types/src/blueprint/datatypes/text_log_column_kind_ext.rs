use super::TextLogColumnKind;

impl TextLogColumnKind {
    /// The name for this kind of column.
    pub fn name(&self) -> &'static str {
        match self {
            Self::EntityPath => "Entity path",
            Self::LogLevel => "Level",
            Self::Body => "Body",
        }
    }
}
