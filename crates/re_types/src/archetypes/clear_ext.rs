use super::Clear;

impl Clear {
    /// Returns a non-recursive clear.
    ///
    /// This will empty all components of the associated entity at the logged timepoint.
    /// Children will be left untouched.
    #[inline]
    pub fn flat() -> Self {
        Self {
            is_recursive: crate::components::ClearIsRecursive(false),
        }
    }

    /// Returns a recursive clear.
    ///
    /// This will empty all components of the associated entity at the logged timepoint, as well as
    /// all components of all its recursive children.
    pub fn recursive() -> Self {
        Self {
            is_recursive: crate::components::ClearIsRecursive(true),
        }
    }
}
