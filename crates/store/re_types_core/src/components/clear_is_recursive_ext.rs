use super::ClearIsRecursive;

impl Default for ClearIsRecursive {
    #[inline]
    fn default() -> Self {
        // Clear only the element itself by default since this is less intrusive.
        // (Clearing recursively can be emulated with many clears, but the reverse is not true.)
        Self(false.into())
    }
}
