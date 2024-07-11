use glam::{uvec4, vec4, UVec4, Vec4};

use super::prelude::*;

/// Extensions to [`Vec4`]
///
/// Adds additional functionality to [`Vec4`] that [`glam`] doesn't have.
pub trait Vec4Ext {
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

    /// Return the square root of each component.
    #[must_use]
    fn sqrt(self) -> Self;

    /// Raw transmute each component to u32.
    #[must_use]
    fn to_bits(self) -> UVec4;

    /// Get the mean value of all four components
    #[must_use]
    fn mean(self) -> f32;

    /// Returns true if all components of the vector is the same within an absolute difference of `max_abs_diff`
    #[must_use]
    fn has_equal_components(self, max_abs_diff: f32) -> bool;
}

impl Vec4Ext for Vec4 {
    #[inline]
    fn trunc(self) -> Self {
        vec4(
            self.x.trunc(),
            self.y.trunc(),
            self.z.trunc(),
            self.w.trunc(),
        )
    }

    fn step(self, value: Self) -> Self {
        vec4(
            self.x.step(value.x),
            self.y.step(value.y),
            self.z.step(value.z),
            self.w.step(value.z),
        )
    }

    fn step_select(self, value: Self, less: Self, greater_or_equal: Self) -> Self {
        vec4(
            self.x.step_select(value.x, less.x, greater_or_equal.x),
            self.y.step_select(value.y, less.y, greater_or_equal.y),
            self.z.step_select(value.z, less.z, greater_or_equal.z),
            self.w.step_select(value.w, less.w, greater_or_equal.w),
        )
    }

    #[inline]
    fn fract(self) -> Self {
        vec4(
            self.x.fract(),
            self.y.fract(),
            self.z.fract(),
            self.w.fract(),
        )
    }

    fn sqrt(self) -> Self {
        vec4(self.x.sqrt(), self.y.sqrt(), self.z.sqrt(), self.w.sqrt())
    }

    fn to_bits(self) -> UVec4 {
        uvec4(
            self.x.to_bits(),
            self.y.to_bits(),
            self.z.to_bits(),
            self.w.to_bits(),
        )
    }

    fn mean(self) -> f32 {
        (self.x + self.y + self.z + self.w) / 4.0
    }

    fn has_equal_components(self, max_abs_diff: f32) -> bool {
        (self.x - self.y).abs() < max_abs_diff
            && (self.y - self.z).abs() < max_abs_diff
            && (self.x - self.z).abs() < max_abs_diff
            && (self.w - self.x).abs() < max_abs_diff
            && (self.w - self.y).abs() < max_abs_diff
            && (self.w - self.z).abs() < max_abs_diff
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_mean() {
        assert!((Vec4::ONE.mean() - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_mean_2() {
        assert!((vec4(1.0, 2.0, 3.0, 4.0).mean() - 2.5).abs() < 0.0001);
    }

    #[test]
    fn test_has_equal_components() {
        assert!(Vec4::ONE.has_equal_components(0.001));
    }

    #[test]
    fn test_has_equal_components_2() {
        assert!(vec4(0.0, 0.00001, -0.00001, 0.0).has_equal_components(0.001));
    }

    #[test]
    fn test_has_equal_components_3() {
        assert!(!vec4(1.0, 0.0, 0.0, 0.0).has_equal_components(0.0001));
    }
}
