use core::ops::Add;
use core::ops::Mul;
use core::ops::RangeInclusive;

/// Linear interpolation between a range
pub fn lerp<T>(range: RangeInclusive<T>, t: f32) -> T
where
    f32: Mul<T, Output = T>,
    T: Add<T, Output = T> + Copy,
{
    (1.0 - t) * *range.start() + t * *range.end()
}

/// Remap a value from one range to another, e.g. do a linear transform.
///
/// # Example
///
/// ```
/// # use macaw::remap;
/// let ocean_height = remap(0.2, -1.0..=1.0, 2.0..=3.1);
/// ```
///
/// # From range requirement
///
/// The range has to be from a low value to a high value, such as 0..=1.0, NOT 1.0..=0.0.
/// If it is outside of that range the results will be undefined
pub fn remap(x: f32, from: RangeInclusive<f32>, to: RangeInclusive<f32>) -> f32 {
    let t = (x - from.start()) / (from.end() - from.start());
    lerp(to, t)
}

/// Remap a value from one range to another, clamps the input value to be in the from range first.
///
/// # Example
///
/// ```
/// # use macaw::remap_clamp;
/// let ocean_height = remap_clamp(0.2, -1.0..=1.0, 2.0..=3.1);
/// ```
///
/// # From range requirement
///
/// The range has to be from a low value to a high value, such as 0..=1.0, NOT 1.0..=0.0.
/// If it is outside of that range the results will be undefined
pub fn remap_clamp(x: f32, from: RangeInclusive<f32>, to: RangeInclusive<f32>) -> f32 {
    if x <= *from.start() {
        *to.start()
    } else if *from.end() <= x {
        *to.end()
    } else {
        let t = (x - from.start()) / (from.end() - from.start());
        // Ensure no numerical inaccurcies sneak in:
        if 1.0 <= t {
            *to.end()
        } else {
            lerp(to, t)
        }
    }
}

#[allow(clippy::float_cmp)]
#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn remapping() {
        // verify simple case
        assert_eq!(remap(0.2, -1.0..=1.0, -2.0..=2.0), 0.4000001);
        assert_eq!(remap_clamp(0.2, -1.0..=1.0, -2.0..=2.0), 0.4000001);
        // out of range
        assert_eq!(remap(3.0, -1.0..=1.0, -2.0..=2.0), 6.0);
        assert_eq!(remap_clamp(3.0, -1.0..=1.0, -2.0..=2.0), 2.0);
        // invalid remapping
        assert!(remap(0.0, 0.0..=0.0, -2.0..=2.0).is_nan());
        assert_eq!(remap_clamp(0.0, 0.0..=0.0, -2.0..=2.0), -2.0);
    }
}
