use glam::{Vec3, Vec4};

/// A 3-dimensional plane primitive.
///
/// Represented by a normal and the signed distance from the origin to the plane.
///
/// This representation can specify any possible plane in 3D space using 4 parameters
/// (disregarding floating point precision). There are duplicate representations caused
/// by negating both parameters, but that's fine - you could view it as an oriented plane.
///
/// A point `point` is on the plane when `plane.normal.dot(point) + plane.d = 0`.
#[derive(Copy, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Plane3 {
    /// Normal vector
    pub normal: Vec3,

    /// Distance
    pub d: f32,
}

impl Plane3 {
    /// The X^Y plane with normal = +Z
    pub const XY: Self = Self {
        normal: Vec3::Z,
        d: 0.0,
    };

    /// The Y^Z plane with normal = +X
    pub const YZ: Self = Self {
        normal: Vec3::X,
        d: 0.0,
    };

    /// The Z^X plane with normal = +Y
    pub const ZX: Self = Self {
        normal: Vec3::Y,
        d: 0.0,
    };

    /// From the plane normal and a distance `d` so that for all points on the plane:
    /// `normal.dot(point) + d = 0`.
    #[inline]
    pub const fn from_normal_dist(normal: Vec3, d: f32) -> Self {
        Self { normal, d }
    }

    /// From the plane normal and a point on the plane.
    #[inline]
    pub fn from_normal_point(normal: Vec3, point: Vec3) -> Self {
        Self {
            normal,
            d: -normal.dot(point),
        }
    }

    /// Get normalized plane
    #[inline]
    #[must_use]
    pub fn normalized(&self) -> Self {
        let inv_len = self.normal.length_recip();
        Self {
            normal: self.normal * inv_len,
            d: self.d * inv_len,
        }
    }

    /// Computes the distance between the plane and the point p.
    /// The returned distance is only correct if the plane is normalized or the distance is zero.
    #[inline]
    pub fn distance(&self, p: Vec3) -> f32 {
        self.normal.dot(p) + self.d
    }

    /// The bool is whether the plane was hit or not.
    ///
    /// If false, the ray was either perpendicular to the plane, or the ray shot away from the plane.
    ///
    /// The returned f32 is the t value so you can easily compute the
    /// intersection point through "origin + dir * t".
    pub fn intersect_ray(&self, origin: Vec3, dir: Vec3) -> (bool, f32) {
        let denom = dir.dot(self.normal);
        if denom == 0.0 {
            (false, 0.0)
        } else {
            let t = -(origin.dot(self.normal) + self.d) / denom;
            if t < 0.0 {
                (false, t)
            } else {
                (true, t)
            }
        }
    }

    /// True if every value is finite
    #[inline]
    pub fn is_finite(&self) -> bool {
        self.normal.is_finite() && self.d.is_finite()
    }

    /// The distance to a point `[x, y, z, 1]` is the dot product of the point and this.
    #[inline]
    pub fn as_vec4(&self) -> Vec4 {
        self.normal.extend(self.d)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_plane3() {
        #![allow(clippy::float_cmp)]
        let point = Vec3::new(2.0, 3.0, 4.0);
        let p = Plane3::from_normal_point(Vec3::new(2.0, 0.0, 0.0), point);
        assert_eq!(p.distance(point), 0.0);
        let p = p.normalized();
        assert_eq!(p.distance(point), 0.0);
    }
}
