//! This module contains utilities to support Rerun examples.

use std::ops::{Add, Mul};

#[cfg(feature = "glam")]
use crate::external::glam;

// ---

/// Linear interpolator.
#[inline]
pub fn lerp<T>(a: T, b: T, t: f32) -> <<f32 as Mul<T>>::Output as std::ops::Add>::Output
where
    T: Mul<f32>,
    f32: Mul<T>,
    <T as Mul<f32>>::Output: Add<<f32 as Mul<T>>::Output>,
    <f32 as Mul<T>>::Output: Add,
{
    (1.0 - t) * a + t * b
}

/// A linear interpolator that bounces between `a` and `b` as `t` goes above `1.0`.
#[inline]
pub fn bounce_lerp<T>(a: T, b: T, t: f32) -> <<f32 as Mul<T>>::Output as std::ops::Add>::Output
where
    T: Mul<f32>,
    f32: Mul<T>,
    <T as Mul<f32>>::Output: Add<<f32 as Mul<T>>::Output>,
    <f32 as Mul<T>>::Output: Add,
{
    let tf = t.fract();
    if t as u32 % 2 == 0 {
        (1.0 - tf) * a + tf * b
    } else {
        tf * a + (1.0 - tf) * b
    }
}

/// Linearly interpolates from `a` through `b` in `n` steps, returning the intermediate result at
/// each step.
#[inline]
pub fn linspace<T>(
    a: T,
    b: T,
    n: usize,
) -> impl Iterator<Item = <<f32 as Mul<T>>::Output as std::ops::Add>::Output>
where
    T: Copy + Mul<f32>,
    f32: Mul<T>,
    <T as Mul<f32>>::Output: Add<<f32 as Mul<T>>::Output>,
    <f32 as Mul<T>>::Output: Add,
{
    (0..n).map(move |t| lerp(a, b, t as f32 / (n - 1) as f32))
}

/// Given two 3D vectors `from` and `to`, linearly interpolates between them in `n` steps along
/// the three axes, returning the intermediate result at each step.
#[cfg(feature = "glam")]
pub fn grid(from: glam::Vec3, to: glam::Vec3, n: usize) -> impl Iterator<Item = glam::Vec3> {
    linspace(from.z, to.z, n).flat_map(move |z| {
        linspace(from.y, to.y, n)
            .flat_map(move |y| linspace(from.x, to.x, n).map(move |x| (x, y, z).into()))
    })
}
