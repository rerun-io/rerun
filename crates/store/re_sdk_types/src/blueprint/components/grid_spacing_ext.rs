use super::GridSpacing;

impl Default for GridSpacing {
    #[inline]
    fn default() -> Self {
        // Default to a unit grid.
        1.0.into()
    }
}
