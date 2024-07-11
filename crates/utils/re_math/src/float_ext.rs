/// Extensions to floating-point primitives.
///
/// Adds additional math-related functionality to floats
pub trait FloatExt {
    /// Returns `0.0` if `value < self` and 1.0 otherwise.
    ///
    /// Similar to glsl's step(edge, x), which translates into edge.step(x)
    #[must_use]
    fn step(self, value: Self) -> Self;

    /// Selects between `less` and `greater_or_equal` based on the result of `value < self`
    #[must_use]
    fn step_select(self, value: Self, less: Self, greater_or_equal: Self) -> Self;

    /// Performs a linear interpolation between `self` and `other` using `a` to weight between them.
    /// The return value is computed as `self * (1−a) + other * a`.
    #[must_use]
    fn lerp(self, other: Self, a: Self) -> Self;

    /// Clamp `self` within the range `[0.0, 1.0]`
    #[must_use]
    fn saturate(self) -> Self;
}

impl FloatExt for f32 {
    fn step(self, value: Self) -> Self {
        if value < self {
            0.0
        } else {
            1.0
        }
    }

    fn step_select(self, value: Self, less: Self, greater_or_equal: Self) -> Self {
        if value < self {
            less
        } else {
            greater_or_equal
        }
    }

    fn lerp(self, other: Self, a: Self) -> Self {
        self + (other - self) * a
    }

    fn saturate(self) -> Self {
        self.clamp(0.0, 1.0)
    }
}
