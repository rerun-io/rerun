use glam::Affine3A;
use glam::Mat4;
use glam::Quat;
use glam::Vec3;
use glam::Vec3A;

/// An isometric transform represented by translation * rotation.
///
/// An isometric transform conserves distances and angles.
///
/// The operations are applied right-to-left, so when transforming a point
/// it will first be rotated and finally translated.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IsoTransform {
    /// Normalized
    rotation: Quat,

    /// Final translation. This is where the input origin will end up,
    /// so for many circumstances this can be thought of as the position.
    translation: Vec3A,
}

/// Identity transform
impl Default for IsoTransform {
    /// Identity transform
    #[inline]
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl IsoTransform {
    // ------------------------------------------------------------------------
    // Constructors:

    /// The identity transform: doesn't transform at all. Like multiplying with `1`.
    pub const IDENTITY: Self = Self {
        rotation: Quat::IDENTITY,
        translation: Vec3A::ZERO,
    };

    /// A transform that first rotates around the origin and then moves all points by a set amount.
    ///
    /// Equivalent to `IsoTransform::from_translation(translation) * IsoTransform::from_quat(rotation)`.
    ///
    /// The given rotation should be normalized.
    #[inline]
    pub fn from_rotation_translation(rotation: Quat, translation: Vec3) -> Self {
        Self {
            rotation,
            translation: translation.into(),
        }
    }

    /// A rotation around a given point
    #[inline]
    pub fn from_rotation_around_point(rotation: Quat, point: Vec3) -> Self {
        Self::from_rotation_translation(rotation, point) * Self::from_translation(-point)
    }

    /// A pure rotation without any translation.
    ///
    /// The given rotation should be normalized.
    #[inline]
    pub fn from_quat(rotation: Quat) -> Self {
        Self {
            rotation,
            translation: Vec3A::ZERO,
        }
    }

    /// A pure translation without any rotation.
    #[inline]
    pub fn from_translation(translation: Vec3) -> Self {
        Self {
            rotation: Quat::IDENTITY,
            translation: translation.into(),
        }
    }

    /// Tries to convert a 4x4 matrix to a [`IsoTransform`].
    ///
    /// This may return [`None`] as a [`Mat4`] can represent things that a [`IsoTransform`] cannot,
    /// such as scale, shearing and projection.
    ///
    /// # Panics
    ///
    /// Will panic if the determinant of `t` is zero and the `assert` feature is enabled.
    #[cfg(not(target_arch = "spirv"))] // TODO: large Options in rust-gpu
    #[inline]
    pub fn from_mat4(t: &Mat4) -> Option<Self> {
        let (scale3, rotation, translation) = t.to_scale_rotation_translation();
        scale3
            .abs_diff_eq(Vec3::splat(1.0), 1e-4)
            .then(|| Self::from_rotation_translation(rotation, translation))
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
        use crate::QuatExt;
        let rotation = Quat::rotate_negative_z_towards(target - eye, up)?;
        Some(Self::from_quat(rotation.inverse()) * Self::from_translation(-eye))
    }

    // ------------------------------------------------------------------------
    // Accessors:

    #[inline]
    pub fn rotation(&self) -> Quat {
        self.rotation
    }

    #[inline]
    pub fn set_rotation(&mut self, rotation: Quat) {
        self.rotation = rotation;
    }

    #[inline]
    pub fn translation(&self) -> Vec3 {
        self.translation.into()
    }

    #[inline]
    pub fn set_translation(&mut self, translation: Vec3) {
        self.translation = translation.into();
    }

    /// True if every value is finite
    #[inline]
    pub fn is_finite(&self) -> bool {
        self.translation.is_finite() && self.rotation.is_finite()
    }

    /// Returns `true` if any elements are `NaN`.
    #[inline]
    pub fn is_nan(&self) -> bool {
        self.translation.is_nan() || self.rotation.is_nan()
    }

    // ------------------------------------------------------------------------
    // Conversions:

    /// Convert to an equivalent `Mat4` transformation matrix.
    #[inline]
    pub fn to_mat4(self) -> Mat4 {
        Mat4::from_rotation_translation(self.rotation, self.translation())
    }

    // ------------------------------------------------------------------------
    // Operations:

    /// Get the transform that undoes this transform so that `t.inverse() * t == IDENTITY`.
    #[inline]
    #[must_use]
    pub fn inverse(&self) -> Self {
        let inv_rotation = self.rotation.inverse();
        Self {
            rotation: inv_rotation,
            translation: inv_rotation * -self.translation,
        }
    }

    /// Returns self normalized.
    /// You generally don't need to call this unless you've multiplied A LOT of `IsoTransforms`.
    #[inline]
    #[must_use]
    pub fn normalize(&self) -> Self {
        Self {
            rotation: self.rotation.normalize(),
            translation: self.translation,
        }
    }

    /// Rotate and translate a point.
    #[inline]
    pub fn transform_point3(&self, p: Vec3) -> Vec3 {
        (self.translation + self.rotation.mul_vec3a(p.into())).into()
    }

    /// Rotate a vector.
    #[inline]
    pub fn transform_vector3(&self, v: Vec3) -> Vec3 {
        self.rotation.mul_vec3a(v.into()).into()
    }
}

/// iso * iso -> iso
impl core::ops::Mul for &IsoTransform {
    type Output = IsoTransform;

    #[inline]
    fn mul(self, rhs: &IsoTransform) -> IsoTransform {
        IsoTransform {
            rotation: self.rotation * rhs.rotation,
            translation: self.translation + self.rotation.mul_vec3a(rhs.translation),
        }
    }
}

/// iso * iso -> iso
impl core::ops::Mul<IsoTransform> for &IsoTransform {
    type Output = IsoTransform;

    #[inline]
    fn mul(self, rhs: IsoTransform) -> IsoTransform {
        self.mul(&rhs)
    }
}

/// iso * iso -> iso
impl core::ops::Mul for IsoTransform {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        (&self).mul(&rhs)
    }
}

/// iso * affine3a -> affine3a
impl core::ops::Mul<Affine3A> for IsoTransform {
    type Output = Affine3A;

    #[inline]
    fn mul(self, rhs: Affine3A) -> Affine3A {
        Affine3A::from(self).mul(rhs)
    }
}

/// affine3a * iso -> affine3a
impl core::ops::Mul<IsoTransform> for Affine3A {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: IsoTransform) -> Self {
        self.mul(Self::from(rhs))
    }
}

/// mat4 * iso -> mat4
impl core::ops::Mul<Mat4> for IsoTransform {
    type Output = Mat4;

    #[inline]
    fn mul(self, rhs: Mat4) -> Mat4 {
        self.to_mat4().mul(rhs)
    }
}

/// iso * mat4 -> mat4
impl core::ops::Mul<IsoTransform> for Mat4 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: IsoTransform) -> Self {
        self.mul(rhs.to_mat4())
    }
}

impl From<IsoTransform> for crate::Affine3A {
    #[inline]
    fn from(iso: IsoTransform) -> Self {
        Self::from_rotation_translation(iso.rotation(), iso.translation())
    }
}

impl From<IsoTransform> for Mat4 {
    #[inline]
    fn from(t: IsoTransform) -> Self {
        t.to_mat4()
    }
}

#[cfg(feature = "std")]
impl core::fmt::Debug for IsoTransform {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let (axis, angle) = self.rotation.to_axis_angle();
        f.debug_struct("IsoTransform")
            .field(
                "translation",
                &format!(
                    "[{} {} {}]",
                    self.translation[0], self.translation[1], self.translation[2]
                ),
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
            .finish()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const MAX_ERR: f32 = 1e-5;

    fn approx_eq_vec3(a: Vec3, b: Vec3) -> bool {
        a.abs_diff_eq(b, MAX_ERR)
    }

    fn approx_eq_quat(a: Quat, b: Quat) -> bool {
        a.abs_diff_eq(b, MAX_ERR) || a.abs_diff_eq(-b, MAX_ERR)
    }

    fn approx_eq_transform(a: IsoTransform, b: IsoTransform) -> bool {
        approx_eq_vec3(a.translation(), b.translation()) && approx_eq_quat(a.rotation, b.rotation)
    }

    macro_rules! assert_approx_eq_vec3 {
        ($a: expr, $b: expr) => {
            assert!(
                approx_eq_vec3($a, $b),
                "[{} {} {}] != [{} {} {}], abs-diff: {:?}",
                $a[0],
                $a[1],
                $a[2],
                $b[0],
                $b[1],
                $b[2],
                ($a - $b).abs(),
            );
        };
    }

    macro_rules! assert_approx_eq_mat4 {
        ($a: expr, $b: expr) => {
            assert!($a.abs_diff_eq($b, MAX_ERR), "Mat4 {:?} vs {:?}", $a, $b,);
        };
    }

    macro_rules! assert_approx_eq_transform {
        ($a: expr, $b: expr) => {
            assert!(approx_eq_transform($a, $b), "{:#?} != {:#?}", $a, $b,);
        };
    }

    #[test]
    fn transform() {
        let t = [
            IsoTransform {
                translation: Vec3A::new(0.0, 0.0, 0.0),
                rotation: Quat::IDENTITY,
            },
            IsoTransform {
                translation: Vec3A::new(0.0, 0.0, 0.0),
                rotation: Quat::from_axis_angle(
                    Vec3::new(0.0, 1.0, 0.0).normalize(),
                    core::f32::consts::PI / 6.0,
                ),
            },
            IsoTransform {
                translation: Vec3A::new(0.7, 1.2, 3.4),
                rotation: Quat::from_axis_angle(
                    Vec3::new(0.3, -0.5, -0.4).normalize(),
                    1.618 * core::f32::consts::PI,
                ),
            },
            IsoTransform {
                translation: Vec3A::new(-3.6, 4.2, -0.7654321),
                rotation: Quat::from_axis_angle(
                    Vec3::new(-0.8, -0.1, 0.2).normalize(),
                    0.321 * core::f32::consts::PI,
                ),
            },
        ];

        for &t in &t {
            test_single_transform(t);
        }

        for &a in &t {
            for &b in &t {
                test_transform_mul(a, b);
            }
        }
    }

    fn test_single_transform(t: IsoTransform) {
        #[cfg(feature = "std")]
        eprintln!("-------------------------------------------\nTesting {t:?}",);

        assert_approx_eq_transform!(t, IsoTransform::from_mat4(&t.to_mat4()).unwrap());
        assert_approx_eq_transform!(t, t.inverse().inverse());
        assert_approx_eq_transform!(t.inverse() * t, IsoTransform::IDENTITY);
        assert_approx_eq_transform!(t * t.inverse(), IsoTransform::IDENTITY);
        assert_approx_eq_mat4!(t.to_mat4().inverse(), t.inverse().to_mat4());

        for p in points() {
            assert_approx_eq_vec3!(t.transform_vector3(p), t.to_mat4().transform_vector3(p));
            assert_approx_eq_vec3!(t.transform_point3(p), t.to_mat4().transform_point3(p));

            assert_approx_eq_vec3!(
                t.inverse().transform_vector3(p),
                t.inverse().to_mat4().transform_vector3(p)
            );
            assert_approx_eq_vec3!(
                t.inverse().transform_point3(p),
                t.inverse().to_mat4().transform_point3(p)
            );

            assert_approx_eq_vec3!(
                t.inverse().transform_vector3(p),
                t.to_mat4().inverse().transform_vector3(p)
            );
            assert_approx_eq_vec3!(
                t.inverse().transform_point3(p),
                t.to_mat4().inverse().transform_point3(p)
            );
        }
    }

    fn test_transform_mul(a: IsoTransform, b: IsoTransform) {
        #[cfg(feature = "std")]
        eprintln!("-------------------------------------------\nTesting {a:?} x {b:?}",);

        assert_approx_eq_transform!(
            a * b,
            IsoTransform::from_mat4(&(a.to_mat4() * b.to_mat4())).unwrap()
        );
        assert_approx_eq_mat4!((a * b).to_mat4(), a.to_mat4() * b.to_mat4());
        for p in points() {
            assert_approx_eq_vec3!(
                (a * b).transform_vector3(p),
                a.transform_vector3(b.transform_vector3(p))
            );
            assert_approx_eq_vec3!(
                (a * b).transform_point3(p),
                a.transform_point3(b.transform_point3(p))
            );
        }
    }

    fn points() -> Vec<Vec3> {
        vec![
            Vec3::X,
            Vec3::Y,
            Vec3::Z,
            Vec3::new(0.1, 0.2, 0.3),
            Vec3::new(-4.5, -3.17, 0.43),
        ]
    }

    #[test]
    fn unsupported_from_mat4() {
        // converting a skewed matrix to IsoTransform should result in None
        assert_eq!(
            IsoTransform::from_mat4(
                &(Mat4::from_scale(Vec3::new(1.0, 2.0, 1.0))
                    * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_3))
            ),
            None
        );
    }

    #[test]
    fn test_rotate_around() {
        use std::f32::consts::TAU;
        let center = Vec3::new(1.0, 2.0, 3.0);
        let t = IsoTransform::from_rotation_around_point(Quat::from_rotation_z(TAU / 4.0), center);

        assert_approx_eq_vec3!(t.transform_point3(center + Vec3::X), center + Vec3::Y);
    }

    #[test]
    fn test_look_at() {
        {
            let eye = Vec3::new(0.0, 2.0, 0.0);
            let center = Vec3::new(10.0, 2.0, 0.0);
            let up = Vec3::new(0.0, 1.0, 0.0);
            let transform = IsoTransform::look_at_rh(eye, center, up).unwrap();
            assert_approx_eq_vec3!(
                transform.transform_point3(center),
                Vec3::new(0.0, 0.0, -10.0)
            );
        }

        {
            let eye = Vec3::new(0.0, 0.0, -5.0);
            let center = Vec3::new(0.0, 0.0, 0.0);
            let up = Vec3::new(1.0, 0.0, 0.0);
            let transform = IsoTransform::look_at_rh(eye, center, up).unwrap();
            let point = Vec3::new(1.0, 0.0, 0.0);
            assert_approx_eq_vec3!(transform.transform_point3(point), Vec3::new(0.0, 1.0, -5.0));
        }
    }
}
