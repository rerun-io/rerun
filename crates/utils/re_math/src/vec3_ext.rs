use glam::{vec3, Vec3};

use super::prelude::*;

/// Extensions to [`Vec3`]
///
/// Adds additional functionality to [`Vec3`] that [`glam`] doesn't have.
pub trait Vec3Ext {
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

    /// Square root of all three components.
    #[must_use]
    fn sqrt(self) -> Self;

    /// Natural logarithm of all three components.
    #[must_use]
    fn ln(self) -> Self;

    /// The reflection of a incident vector and surface normal.
    #[must_use]
    fn reflect(self, normal: Self) -> Self;

    /// Get the mean value of all three components
    #[must_use]
    fn mean(self) -> f32;

    /// Returns true if all components of the vector is the same within an absolute difference of `max_abs_diff`
    #[must_use]
    fn has_equal_components(self, max_abs_diff: f32) -> bool;
}

impl Vec3Ext for Vec3 {
    /// For element `i` of `self`, return `v[i].trunc()`
    #[inline]
    fn trunc(self) -> Self {
        vec3(self.x.trunc(), self.y.trunc(), self.z.trunc())
    }

    fn step(self, value: Self) -> Self {
        vec3(
            self.x.step(value.x),
            self.y.step(value.y),
            self.z.step(value.z),
        )
    }

    fn step_select(self, value: Self, less: Self, greater_or_equal: Self) -> Self {
        vec3(
            self.x.step_select(value.x, less.x, greater_or_equal.x),
            self.y.step_select(value.y, less.y, greater_or_equal.y),
            self.z.step_select(value.z, less.z, greater_or_equal.z),
        )
    }

    fn fract(self) -> Self {
        vec3(self.x.fract(), self.y.fract(), self.z.fract())
    }

    fn saturate(self) -> Self {
        vec3(self.x.saturate(), self.y.saturate(), self.z.saturate())
    }

    fn sqrt(self) -> Self {
        vec3(self.x.sqrt(), self.y.sqrt(), self.z.sqrt())
    }

    fn ln(self) -> Self {
        vec3(self.x.ln(), self.y.ln(), self.z.ln())
    }

    fn reflect(self, normal: Self) -> Self {
        self - 2.0 * normal * self.dot(normal)
    }

    fn mean(self) -> f32 {
        (self.x + self.y + self.z) / 3.0
    }

    fn has_equal_components(self, max_abs_diff: f32) -> bool {
        (self.x - self.y).abs() < max_abs_diff
            && (self.y - self.z).abs() < max_abs_diff
            && (self.x - self.z).abs() < max_abs_diff
    }
}

/// Coordinate system extension to [`Vec3`]
///
/// This crate is opinionated with what coordinate system it uses and this adds
/// additional functions to access the coordinate system axis
///
/// The exact coordinate system we use is right-handed with +X = right, +Y = up, -Z = forward, +Z = back
pub trait CoordinateSystem {
    /// A unit length vector pointing in the canonical up direction.
    fn up() -> Self;

    /// A unit length vector pointing in the canonical down direction.
    fn down() -> Self;

    /// A unit length vector pointing in the canonical right direction.
    ///
    /// This is the right hand side of a first person character.
    fn right() -> Self;

    /// A unit length vector pointing in the canonical left direction.
    fn left() -> Self;

    /// A unit length vector pointing in the canonical forward direction.
    ///
    /// This is the direction a character faces, or a car drives towards.
    fn forward() -> Self;

    /// A unit length vector pointing in the canonical back direction.
    fn back() -> Self;
}

impl CoordinateSystem for Vec3 {
    fn up() -> Self {
        Self::new(0.0, 1.0, 0.0)
    }

    fn down() -> Self {
        Self::new(0.0, -1.0, 0.0)
    }

    fn right() -> Self {
        Self::new(1.0, 0.0, 0.0)
    }

    fn left() -> Self {
        Self::new(-1.0, 0.0, 0.0)
    }

    fn forward() -> Self {
        Self::new(0.0, 0.0, -1.0)
    }

    fn back() -> Self {
        Self::new(0.0, 0.0, 1.0)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_mean() {
        assert!((Vec3::ONE.mean() - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_mean_2() {
        assert!((vec3(1.0, 2.0, 3.0).mean() - 2.0).abs() < 0.0001);
    }

    #[test]
    fn test_has_equal_components() {
        assert!(Vec3::ONE.has_equal_components(0.001));
    }

    #[test]
    fn test_has_equal_components_2() {
        assert!(vec3(0.0, 0.00001, -0.00001).has_equal_components(0.001));
    }

    #[test]
    fn test_has_equal_components_3() {
        assert!(!vec3(1.0, 0.0, 0.0).has_equal_components(0.0001));
    }
}
