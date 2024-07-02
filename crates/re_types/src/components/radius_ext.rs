use super::Radius;

impl Radius {
    /// Zero radius.
    pub const ZERO: Self = Self(0.0);

    /// Radius of length 1 in ui points.
    pub const ONE_UI_POINTS: Self = Self(-1.0);
}

impl Default for Radius {
    #[inline]
    fn default() -> Self {
        Self::new_ui_points(1.5)
    }
}

impl Radius {
    /// Creates a new radius in scene units.
    ///
    /// Values passed must be finite positive.
    #[inline]
    pub fn new_scene_units(radius_in_scene_units: f32) -> Self {
        debug_assert!(
            0.0 <= radius_in_scene_units,
            "Bad radius: {radius_in_scene_units}"
        );
        Self(radius_in_scene_units)
    }

    /// Creates a new radius in ui points.
    ///
    /// Values passed must be finite positive.
    #[inline]
    pub fn new_ui_points(radius_in_ui_points: f32) -> Self {
        debug_assert!(
            0.0 <= radius_in_ui_points,
            "Bad radius: {radius_in_ui_points}"
        );
        Self(-radius_in_ui_points)
    }

    /// If this radius is in scene units, returns the radius in scene units.
    #[inline]
    pub fn scene_units(&self) -> Option<f32> {
        // Ensure negative zero is treated as a point size.
        self.0.is_sign_positive().then_some(self.0)
    }

    /// If this radius is in ui points, returns the radius in ui points.
    #[inline]
    pub fn ui_points(&self) -> Option<f32> {
        // Ensure negative zero is treated as a point size.
        self.0.is_sign_negative().then_some(-self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::Radius;

    #[test]
    fn scene_point_distinction() {
        let radius = Radius(1.0);
        assert_eq!(radius.scene_units(), Some(1.0));
        assert_eq!(radius.ui_points(), None);

        let radius = Radius(-1.0);
        assert_eq!(radius.scene_units(), None);
        assert_eq!(radius.ui_points(), Some(1.0));

        let radius = Radius(f32::INFINITY);
        assert_eq!(radius.scene_units(), Some(f32::INFINITY));
        assert_eq!(radius.ui_points(), None);

        let radius = Radius(f32::NEG_INFINITY);
        assert_eq!(radius.scene_units(), None);
        assert_eq!(radius.ui_points(), Some(f32::INFINITY));

        let radius = Radius(0.0);
        assert_eq!(radius.scene_units(), Some(0.0));
        assert_eq!(radius.ui_points(), None);

        let radius = Radius(-0.0);
        assert_eq!(radius.scene_units(), None);
        assert_eq!(radius.ui_points(), Some(0.0));
    }
}
