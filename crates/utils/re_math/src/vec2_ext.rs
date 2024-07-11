use super::prelude::*;

use glam::{vec2, Vec2};

/// Extensions to [`Vec2`]
///
/// Adds additional functionality to [`Vec2`] that [`glam`] doesn't have.
pub trait Vec2Ext {
    /// `x: cos(angle), y: sin(angle)` (in radians)
    #[must_use]
    fn from_angle(angle: f32) -> Self;

    /// Angle of the vector: `y.atan2(x)`
    #[must_use]
    fn angle(&self) -> f32;

    /// For element `i` of `self`, return `v[i].trunc()`
    #[must_use]
    fn trunc(self) -> Self;

    /// For element `i` of the return value, returns 0.0 if `value[i] < self[i]` and 1.0 otherwise.
    ///
    /// Similar to glsl's step(edge, x), which translates into edge.step(x)
    #[must_use]
    fn step(self, value: Self) -> Self;

    /// Selects between `true` and `false` based on the result of `value[i] < self[i]`
    #[must_use]
    fn step_select(self, value: Self, true_: Self, false_: Self) -> Self;

    /// Return only the fractional parts of each component.
    #[must_use]
    fn fract(self) -> Self;

    /// Clamp all components of `self` to the range `[0.0, 1.0]`
    #[must_use]
    fn saturate(self) -> Self;

    /// Get the mean value of the two components
    #[must_use]
    fn mean(self) -> f32;

    /// Returns true if both components of the vector is the same within an absolute difference of `max_abs_diff`
    #[must_use]
    fn has_equal_components(self, max_abs_diff: f32) -> bool;
}

impl Vec2Ext for Vec2 {
    #[inline]
    fn trunc(self) -> Self {
        vec2(self.x.trunc(), self.y.trunc())
    }

    fn from_angle(angle: f32) -> Self {
        Self::new(angle.cos(), angle.sin())
    }

    fn angle(&self) -> f32 {
        self.y.atan2(self.x)
    }

    fn step(self, value: Self) -> Self {
        vec2(self.x.step(value.x), self.y.step(value.y))
    }

    fn step_select(self, value: Self, less: Self, greater_or_equal: Self) -> Self {
        vec2(
            self.x.step_select(value.x, less.x, greater_or_equal.x),
            self.y.step_select(value.y, less.y, greater_or_equal.y),
        )
    }

    fn fract(self) -> Self {
        vec2(self.x.fract(), self.y.fract())
    }

    fn saturate(self) -> Self {
        vec2(self.x.saturate(), self.y.saturate())
    }

    fn mean(self) -> f32 {
        (self.x + self.y) / 2.0
    }

    fn has_equal_components(self, max_abs_diff: f32) -> bool {
        (self.x - self.y).abs() < max_abs_diff
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_mean() {
        assert!((Vec2::ONE.mean() - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_mean_2() {
        assert!((vec2(1.0, 3.0).mean() - 2.0).abs() < 0.0001);
    }

    #[test]
    fn test_has_equal_components() {
        assert!(Vec2::ONE.has_equal_components(0.001));
    }

    #[test]
    fn test_has_equal_components_2() {
        assert!(vec2(0.0, 0.00001).has_equal_components(0.001));
    }

    #[test]
    fn test_has_equal_components_3() {
        assert!(!vec2(1.0, 0.0).has_equal_components(0.0001));
    }
}
