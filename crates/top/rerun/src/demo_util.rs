//! This module contains utilities to support Rerun examples.

use std::ops::{Add, Mul, Sub};

#[cfg(feature = "glam")]
use crate::external::glam;

// ---

/// Linear interpolator.
#[inline]
pub fn lerp<T>(
    a: T,
    b: T,
    t: f32,
) -> <<T as Sub<<f32 as Mul<T>>::Output>>::Output as Add<<f32 as Mul<T>>::Output>>::Output
where
    T: Copy + Mul<f32> + Sub<<f32 as Mul<T>>::Output>,
    f32: Mul<T>,
    <T as Sub<<f32 as Mul<T>>::Output>>::Output: Add<<f32 as Mul<T>>::Output>,
{
    a - t * a + t * b
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
    if (t as u32).is_multiple_of(2) {
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
) -> impl Iterator<
    Item = <<T as Sub<<f32 as Mul<T>>::Output>>::Output as Add<<f32 as Mul<T>>::Output>>::Output,
>
where
    T: Copy + Mul<f32> + Sub<<f32 as Mul<T>>::Output>,
    f32: Mul<T>,
    <T as Sub<<f32 as Mul<T>>::Output>>::Output: Add<<f32 as Mul<T>>::Output>,
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

/// Create a spiral of points with colors along the Z axis.
///
/// * `num_points`: Total number of points.
/// * `radius`: The radius of the spiral.
/// * `angular_step`: The factor applied between each step along the trigonometric circle.
/// * `angular_offset`: Offsets the starting position on the trigonometric circle.
/// * `z_step`: The factor applied between each step along the Z axis.
#[cfg(feature = "glam")]
pub fn color_spiral(
    num_points: usize,
    radius: f32,
    angular_step: f32,
    angular_offset: f32,
    z_step: f32,
) -> (Vec<glam::Vec3>, Vec<[u8; 4]>) {
    use std::f32::consts::TAU;
    let points = (0..num_points)
        .map(move |i| {
            let angle = i as f32 * angular_step * TAU + angular_offset;
            glam::Vec3::new(
                angle.cos() * radius,
                angle.sin() * radius,
                i as f32 * z_step,
            )
        })
        .collect();

    let colors = (0..num_points)
        .map(move |i| colormap_turbo_srgb(i as f32 / num_points as f32))
        .collect();

    (points, colors)
}

/// Returns sRGB polynomial approximation from Turbo color map, assuming `t` is normalized.
fn colormap_turbo_srgb(t: f32) -> [u8; 4] {
    #![expect(clippy::excessive_precision)]
    use glam::{Vec2, Vec4, Vec4Swizzles as _};

    const R4: Vec4 = Vec4::new(0.13572138, 4.61539260, -42.66032258, 132.13108234);
    const G4: Vec4 = Vec4::new(0.09140261, 2.19418839, 4.84296658, -14.18503333);
    const B4: Vec4 = Vec4::new(0.10667330, 12.64194608, -60.58204836, 110.36276771);

    const R2: Vec2 = Vec2::new(-152.94239396, 59.28637943);
    const G2: Vec2 = Vec2::new(4.27729857, 2.82956604);
    const B2: Vec2 = Vec2::new(-89.90310912, 27.34824973);

    debug_assert!((0.0..=1.0).contains(&t));

    let v4 = glam::vec4(1.0, t, t * t, t * t * t);
    let v2 = v4.zw() * v4.z;

    [
        ((v4.dot(R4) + v2.dot(R2)) * 255.0) as u8,
        ((v4.dot(G4) + v2.dot(G2)) * 255.0) as u8,
        ((v4.dot(B4) + v2.dot(B2)) * 255.0) as u8,
        255,
    ]
}
