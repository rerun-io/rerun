use glam::Affine3A;
use glam::Mat4;
use glam::Vec3A;

use crate::{IsoTransform, Quat, Vec3, Vec3Ext, Vec4, Vec4Swizzles};

/// Represents a transform with translation + rotation + uniform scale.
///
/// Preserves local angles.
/// Scale and rotation will be applied first, then translation.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Conformal3 {
    translation_and_scale: Vec4,
    rotation: Quat,
}

impl Conformal3 {
    /// The identity transform: doesn't transform at all. Like multiplying with `1`.
    pub const IDENTITY: Self = Self {
        translation_and_scale: Vec4::W,
        rotation: Quat::IDENTITY,
    };

    #[inline]
    /// A transform that first rotates and scales around the origin and then moves all points by a set amount.
    ///
    /// Equivalent to `Conformal3::from_translation(translation) * (Conformal3::from_scale(rotation) * Conformal3::from_quat(scale))`.
    ///
    /// The given rotation should be normalized.
    pub fn from_scale_rotation_translation(scale: f32, rotation: Quat, translation: Vec3) -> Self {
        Self {
            translation_and_scale: translation.extend(scale),
            rotation,
        }
    }

    /// A transform that first rotates around the origin and then moves all points by a set amount.
    ///
    /// Equivalent to `Conformal3::from_translation(translation) * Conformal3::from_quat(rotation)`.
    ///
    /// The given rotation should be normalized.
    #[inline]
    pub fn from_rotation_translation(rotation: Quat, translation: Vec3) -> Self {
        Self {
            translation_and_scale: translation.extend(1.0),
            rotation,
        }
    }

    /// A pure translation without any rotation or scale.
    #[inline]
    pub fn from_translation(translation: Vec3) -> Self {
        Self {
            translation_and_scale: translation.extend(1.0),
            rotation: Quat::IDENTITY,
        }
    }

    /// Returns this transform decomposed into scale, rotation, translation
    #[inline]
    pub fn to_scale_rotation_translation(&self) -> (f32, Quat, Vec3) {
        (self.scale(), self.rotation(), self.translation())
    }

    /// A pure rotation without any translation or scale.
    #[inline]
    pub fn from_quat(rotation: Quat) -> Self {
        Self::from_scale_rotation_translation(1.0, rotation, Vec3::ZERO)
    }

    /// A pure scale without any translation or rotation.
    #[inline]
    pub fn from_scale(scale: f32) -> Self {
        Self::from_scale_rotation_translation(scale, Quat::IDENTITY, Vec3::ZERO)
    }

    /// Returns the inverse of this transform. `my_transform * my_transform.inverse() = Conformal3::IDENITTY`
    #[inline]
    pub fn inverse(&self) -> Self {
        let inv_scale = self.inv_scale();
        let inv_rotation = self.rotation.inverse();
        let inv_translation = inv_scale * (inv_rotation * -self.translation());
        Self::from_scale_rotation_translation(inv_scale, inv_rotation, inv_translation)
    }

    /// Returns self normalized.
    /// You generally don't need to call this unless you've multiplied A LOT of `Conformal3`.
    #[inline]
    #[must_use]
    pub fn normalize(&self) -> Self {
        let scale = self.scale();
        let rotation = self.rotation().normalize();
        let translation = self.translation();
        Self::from_scale_rotation_translation(scale, rotation, translation)
    }

    /// Will attempt to create a `Conformal3` from an `Affine3A`. Assumes no shearing and uniform scaling.
    /// If the affine transform contains shearing or non-uniform scaling it will be lost.
    #[inline]
    pub fn from_affine3a_lossy(transform: &crate::Affine3A) -> Self {
        let (scale, rotation, translation) = transform.to_scale_rotation_translation();
        Self {
            translation_and_scale: translation.extend(scale.mean()),
            rotation: rotation.normalize(),
        }
    }

    /// Returns this transform as an `Affine3A`
    #[inline]
    pub fn to_affine3a(&self) -> Affine3A {
        Affine3A::from_scale_rotation_translation(
            Vec3::splat(self.scale()),
            self.rotation(),
            self.translation(),
        )
    }

    /// Returns this transform as a `Mat4`
    #[inline]
    pub fn to_mat4(self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            Vec3::splat(self.scale()),
            self.rotation(),
            self.translation(),
        )
    }

    /// Transform a `Vec3` using translation, rotation, scale.
    #[inline]
    pub fn transform_point3(&self, value: Vec3) -> Vec3 {
        self.translation() + self.scale() * (self.rotation() * value)
    }

    /// Transform a `Vec3A` using translation, rotation, scale.
    #[inline]
    pub fn transform_point3a(&self, value: Vec3A) -> Vec3A {
        Vec3A::from(self.translation()) + self.scale() * (self.rotation() * value)
    }

    /// Transform a `Vec3` using only rotation and scale.
    #[inline]
    pub fn transform_vector3(&self, value: Vec3) -> Vec3 {
        self.scale() * (self.rotation() * value)
    }

    /// Transform a `Vec3A` using only rotation and scale.
    #[inline]
    pub fn transform_vector3a(&self, value: Vec3A) -> Vec3A {
        self.scale() * (self.rotation() * value)
    }

    /// Returns the rotation
    #[inline]
    pub fn rotation(&self) -> Quat {
        self.rotation
    }

    /// Sets the rotation
    #[inline]
    pub fn set_rotation(&mut self, rotation: Quat) {
        self.rotation = rotation;
    }

    /// Returns the translation
    #[inline]
    pub fn translation(&self) -> Vec3 {
        self.translation_and_scale.xyz()
    }

    /// Returns the translation and scale as a `Vec4`
    #[inline]
    pub fn translation_and_scale(&self) -> Vec4 {
        self.translation_and_scale
    }

    /// Sets the translation
    #[inline]
    pub fn set_translation(&mut self, translation: Vec3) {
        let scale = self.scale();
        self.translation_and_scale = translation.extend(scale);
    }

    /// Returns the scale
    #[inline]
    pub fn scale(&self) -> f32 {
        self.translation_and_scale.w
    }

    /// Returns the scale inverse
    #[inline]
    pub fn inv_scale(&self) -> f32 {
        if self.scale() == 0.0 {
            f32::INFINITY
        } else {
            1.0 / self.scale()
        }
    }

    /// Sets the scale
    #[inline]
    pub fn set_scale(&mut self, scale: f32) {
        self.translation_and_scale.w = scale;
    }

    /// Builds a `Conformal3` from an `IsoTransform` (rotation, translation).
    #[inline]
    pub fn from_iso_transform(t: IsoTransform) -> Self {
        Self::from_rotation_translation(t.rotation(), t.translation())
    }

    /// Creates a right-handed view transform using a camera position,
    /// a point to look at, and an up direction.
    ///
    /// The result transforms from world coordinates to view coordinates.
    ///
    /// For a view coordinate system with `+X=right`, `+Y=up` and `+Z=back`.
    ///
    /// Will return [`None`] if any argument is zero, non-finite, or if forward and up are colinear.
    #[cfg(not(target_arch = "spirv"))] // TODO: large Options in rust-gpu
    #[inline]
    pub fn look_at_rh(eye: Vec3, target: Vec3, up: Vec3) -> Option<Self> {
        IsoTransform::look_at_rh(eye, target, up).map(Self::from_iso_transform)
    }

    /// Returns `true` if, and only if, all components are finite.
    ///
    /// If any component is either `NaN`, positive or negative infinity, this will return `false`.
    pub fn is_finite(&self) -> bool {
        self.translation_and_scale.is_finite() && self.rotation.is_finite()
    }
}

impl core::ops::Mul for &Conformal3 {
    type Output = Conformal3;

    #[inline]
    fn mul(self, rhs: &Conformal3) -> Conformal3 {
        let translation = self.transform_point3(rhs.translation());
        let rotation = self.rotation() * rhs.rotation();
        let scale = self.scale() * rhs.scale();
        Conformal3::from_scale_rotation_translation(scale, rotation, translation)
    }
}

impl core::ops::Mul<Conformal3> for &Conformal3 {
    type Output = Conformal3;

    #[inline]
    fn mul(self, rhs: Conformal3) -> Conformal3 {
        self.mul(&rhs)
    }
}

impl core::ops::Mul for Conformal3 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        (&self).mul(&rhs)
    }
}

impl core::ops::Mul<Conformal3> for IsoTransform {
    type Output = Conformal3;

    #[inline]
    fn mul(self, rhs: Conformal3) -> Conformal3 {
        Conformal3::from_iso_transform(self).mul(rhs)
    }
}

impl core::ops::Mul<IsoTransform> for Conformal3 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: IsoTransform) -> Self {
        self.mul(Self::from_iso_transform(rhs))
    }
}

/// Identity transform
impl Default for Conformal3 {
    /// Identity transform
    #[inline]
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl From<Conformal3> for Mat4 {
    #[inline]
    fn from(c: Conformal3) -> Self {
        c.to_mat4()
    }
}

impl From<Conformal3> for crate::Affine3A {
    #[inline]
    fn from(c: Conformal3) -> Self {
        c.to_affine3a()
    }
}

impl From<IsoTransform> for Conformal3 {
    #[inline]
    fn from(c: IsoTransform) -> Self {
        Self::from_iso_transform(c)
    }
}

#[cfg(feature = "std")]
impl core::fmt::Debug for Conformal3 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let (axis, angle) = self.rotation().to_axis_angle();
        let translation = self.translation();
        let scale = self.scale();
        f.debug_struct("Conformal3")
            .field(
                "translation",
                &format!("[{} {} {}]", translation[0], translation[1], translation[2]),
            )
            .field(
                "rotation",
                &format!(
                    "{:.1}° around [{} {} {}]",
                    angle.to_degrees(),
                    axis[0],
                    axis[1],
                    axis[2],
                ),
            )
            .field("scale", &format!("{}", scale))
            .finish()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn approx_eq_transform(a: Conformal3, b: Conformal3) -> bool {
        let max_abs_diff = 1e-6;
        a.translation().abs_diff_eq(b.translation(), max_abs_diff)
            && a.rotation().abs_diff_eq(b.rotation(), max_abs_diff)
            && ((a.scale() - b.scale()).abs() < max_abs_diff)
    }

    macro_rules! assert_approx_eq_transform {
        ($a: expr, $b: expr) => {
            assert!(approx_eq_transform($a, $b), "{:#?} != {:#?}", $a, $b,);
        };
    }

    #[test]
    fn test_inverse() {
        use crate::Conformal3;

        let transform = Conformal3::from_scale_rotation_translation(
            2.0,
            Quat::from_rotation_y(std::f32::consts::PI),
            Vec3::ONE,
        );
        let identity = transform * transform.inverse();
        assert_approx_eq_transform!(identity, Conformal3::IDENTITY);

        let transform = Conformal3::from_scale_rotation_translation(
            10.0,
            Quat::from_axis_angle(Vec3::ONE.normalize(), 1.234),
            Vec3::new(1.0, 2.0, 3.0),
        );
        let identity = transform * transform.inverse();
        assert_approx_eq_transform!(identity, Conformal3::IDENTITY);
    }
}
