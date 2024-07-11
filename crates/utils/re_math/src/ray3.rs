use crate::Vec3;

/// A ray in 3-dimensional space: a line through space with a starting point and a direction.
///
/// Any point on the ray can be found through the formula `origin + t * dir`,
/// where t is a non-negative floating point value, which represents the distance
/// along the ray.
#[derive(Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ray3 {
    /// Start of the ray
    pub origin: Vec3,
    /// Direction of the ray, normalized
    pub dir: Vec3,
}

impl Ray3 {
    /// An invalid ray, starting at the origin and going nowhere.
    pub const ZERO: Self = Self {
        origin: Vec3::ZERO,
        dir: Vec3::ZERO,
    };

    /// `dir` should be normalized
    #[inline]
    pub fn from_origin_dir(origin: Vec3, dir: Vec3) -> Self {
        Self { origin, dir }
    }

    /// Get normalized ray (where `dir.len() == 1`).
    #[inline]
    #[must_use]
    pub fn normalize(&self) -> Self {
        Self {
            origin: self.origin,
            dir: self.dir.normalize(),
        }
    }

    /// Returns a new ray that has had its origin moved a given distance forwards along the ray.
    ///
    /// If the ray direction is normalized then the `t` parameter corresponds to the world space distance it moves.
    #[inline]
    #[must_use]
    pub fn offset_along_ray(&self, t: f32) -> Self {
        Self {
            origin: self.origin + self.dir * t,
            dir: self.dir,
        }
    }

    /// True if every value is finite
    #[inline]
    pub fn is_finite(&self) -> bool {
        self.origin.is_finite() && self.dir.is_finite()
    }

    #[inline]
    pub fn point_along(&self, t: f32) -> Vec3 {
        self.origin + t * self.dir
    }

    /// Returns the line segment where `self` and `other` are the closest to each other.
    /// If the rays are parallel then non-finite points are returned.
    pub fn closest_points(&self, other: &Self) -> (Vec3, Vec3) {
        // https://en.wikipedia.org/wiki/Skew_lines#Nearest_Points
        let (self_t, other_t) = self.closest_ts(other);
        (self.point_along(self_t), other.point_along(other_t))
    }

    /// Returns the distance along both rays which together form
    /// line segment where `self` and `other` are the closest to each other.
    /// If the rays are parallel then non-finite values are returned.
    pub fn closest_ts(&self, other: &Self) -> (f32, f32) {
        // https://en.wikipedia.org/wiki/Skew_lines#Nearest_Points
        let (a, b) = (self, other);
        let n = a.dir.cross(b.dir);
        let n_a = a.dir.cross(n);
        let n_b = b.dir.cross(n);
        let a_t = (b.origin - a.origin).dot(n_b) / a.dir.dot(n_b);
        let b_t = (a.origin - b.origin).dot(n_a) / b.dir.dot(n_a);
        (a_t, b_t)
    }

    /// Returns the point where the ray intersects the plane.
    /// Returns non-finite result of the ray and plane are parallel.
    pub fn intersects_plane(&self, plane: crate::Plane3) -> Vec3 {
        let (ro, rd) = (self.origin, self.dir);
        let (pn, pd) = (plane.normal, plane.d);
        // p = ro + t * rd
        // p.dot(pn) + pd = 0
        // (ro + t * rd).dot(pn) + pd = 0
        // ro.dot(pn) + t * rd.dot(pn) + pd = 0
        // t * rd.dot(pn) = -(ro.dot(pn) + pd)
        // t = -(ro.dot(pn) + pd) / rd.dot(pn)
        let t = -(ro.dot(pn) + pd) / rd.dot(pn);
        ro + t * rd

        // alternate implementation:
        //     let point = self.to_line().intersects_plane(plane);
        //     (point.truncate() / point.w).into()
    }

    // Returns the distance along the ray that is closest to the given point.
    // The returned `t` can be negative.
    #[inline]
    pub fn closest_t_to_point(&self, point: Vec3) -> f32 {
        self.dir.dot(point - self.origin)
    }

    /// Returns the point along the ray that is closest to the given point.
    /// The returned point may be "behind" the ray origin.
    #[inline]
    pub fn closest_point_to_point(&self, point: Vec3) -> Vec3 {
        self.origin + self.dir * self.dir.dot(point - self.origin)
    }
}

impl core::ops::Mul<Ray3> for crate::IsoTransform {
    type Output = Ray3;

    fn mul(self, rhs: Ray3) -> Ray3 {
        Ray3 {
            origin: self.transform_point3(rhs.origin),
            dir: self.transform_vector3(rhs.dir),
        }
    }
}

impl core::ops::Mul<Ray3> for crate::Conformal3 {
    type Output = Ray3;

    fn mul(self, rhs: Ray3) -> Ray3 {
        Ray3 {
            origin: self.transform_point3(rhs.origin),
            dir: self.transform_vector3(rhs.dir),
        }
    }
}

impl core::ops::Mul<Ray3> for glam::Affine3A {
    type Output = Ray3;

    fn mul(self, rhs: Ray3) -> Ray3 {
        Ray3 {
            origin: self.transform_point3(rhs.origin),
            dir: self.transform_vector3(rhs.dir).normalize(),
        }
    }
}

impl core::ops::Mul<Ray3> for glam::Mat4 {
    type Output = Ray3;

    fn mul(self, rhs: Ray3) -> Ray3 {
        Ray3 {
            origin: self.transform_point3(rhs.origin),
            dir: self.transform_vector3(rhs.dir).normalize(),
        }
    }
}

#[cfg(not(target_arch = "spirv"))]
impl std::fmt::Debug for Ray3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ray3")
            .field(
                "origin",
                &format!(
                    "[{:.3} {:.3} {:.3}]",
                    self.origin[0], self.origin[1], self.origin[2]
                ),
            )
            .field(
                "dir",
                &format!("[{:.2} {:.2} {:.2}]", self.dir[0], self.dir[1], self.dir[2]),
            )
            .finish()
    }
}
