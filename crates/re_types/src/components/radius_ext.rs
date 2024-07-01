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
            (0.0..=f32::INFINITY).contains(&radius_in_scene_units),
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
            (0.0..=f32::INFINITY).contains(&radius_in_ui_points),
            "Bad radius: {radius_in_ui_points}"
        );
        Self(-radius_in_ui_points)
    }

    /// If this radius is in scene units, returns the radius in scene units.
    #[inline]
    pub fn scene_units(&self) -> Option<f32> {
        (self.0 >= 0.0).then_some(self.0)
    }

    /// If this radius is in ui points, returns the radius in ui points.
    #[inline]
    pub fn ui_points(&self) -> Option<f32> {
        (self.0 < 0.0).then_some(-self.0)
    }
}
