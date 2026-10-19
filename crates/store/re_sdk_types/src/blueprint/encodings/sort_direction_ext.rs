use super::SortDirection;

impl SortDirection {
    /// Whether the smallest values come first.
    pub fn is_ascending(&self) -> bool {
        matches!(self, Self::Ascending)
    }
}
