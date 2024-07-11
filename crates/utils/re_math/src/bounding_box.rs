use glam::{Affine3A, Quat, Vec3};

use crate::{Conformal3, IsoTransform};

/// A 3-dimensional axis-aligned bounding box
#[derive(Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BoundingBox {
    /// Bounding box minimum (inclusive).
    pub min: Vec3,

    /// Bounding box maximum (inclusive).
    pub max: Vec3,
}

impl core::fmt::Debug for BoundingBox {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?} - {:?}", self.min, self.max)
    }
}

#[allow(unused)]
impl BoundingBox {
    /// A [`BoundingBox`] that only contains [`Vec3::ZERO`].
    pub const ZERO: Self = Self {
        min: Vec3::ZERO,
        max: Vec3::ZERO,
    };

    /// A [`BoundingBox`] that contains no points.
    ///
    /// This is useful as the seed for bounding bounding boxes.
    #[inline]
    pub fn nothing() -> Self {
        Self {
            min: Vec3::splat(core::f32::INFINITY),
            max: Vec3::splat(core::f32::NEG_INFINITY),
        }
    }

    /// A [`BoundingBox`] that contains every point.
    #[inline]
    pub fn everything() -> Self {
        Self {
            min: Vec3::splat(core::f32::NEG_INFINITY),
            max: Vec3::splat(core::f32::INFINITY),
        }
    }

    /// Create a bounding box from a minimum and maximum position.
    #[inline]
    pub fn from_min_max(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    #[inline]
    pub fn from_min_size(min: Vec3, size: Vec3) -> Self {
        Self {
            min,
            max: min + size,
        }
    }

    /// Create a bounding box from a center position and a size.
    pub fn from_center_size(center: Vec3, size: Vec3) -> Self {
        Self::from_min_max(center - 0.5 * size, center + 0.5 * size)
    }

    /// Create a bounding box from an iterator of points that the bounding box will cover.
    pub fn from_points(points: impl Iterator<Item = Vec3>) -> Self {
        let mut bb = Self::nothing();
        for p in points {
            bb.extend(p);
        }
        bb
    }

    /// Returns the center point of the bounding box.
    #[inline]
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    /// Returns the 3D axis size of the bounding box.
    #[inline]
    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    /// Returns half the size (similar to a radius).
    #[inline]
    pub fn half_size(&self) -> Vec3 {
        0.5 * (self.max - self.min)
    }

    /// Only correct for positively sized boxes.
    pub fn volume(&self) -> f32 {
        let s = self.size();
        s.x * s.y * s.z
    }

    /// True if and only if there is at least one point for which `bb.contains(point)` is true.
    ///
    /// Will return `true` if [`Self::min`] == [`Self::max`].
    /// The opposite of `is_nothing()`.
    pub fn is_something(&self) -> bool {
        self.min.x <= self.max.x && self.min.y <= self.max.y && self.min.z <= self.max.z
    }

    /// True if and only if there is no point for which `bb.contains(point)` is true.
    ///
    /// The opposite of `is_something()`.
    pub fn is_nothing(&self) -> bool {
        self.max.x < self.min.x || self.max.y < self.min.y || self.max.z < self.min.z
    }

    /// True if this box contains exactly one point.
    ///
    /// `true` if [`Self::min`] == [`Self::max`].
    #[inline]
    pub fn is_point(&self) -> bool {
        self.min == self.max
    }

    /// Returns `true` if, and only if, all elements are finite.
    ///
    /// If any element is either `NaN`, positive or negative infinity, this will return `false`.
    #[inline]
    pub fn is_finite(&self) -> bool {
        self.min.is_finite() && self.max.is_finite()
    }

    /// Returns `true` if any elements are `NaN`.
    #[inline]
    pub fn is_nan(&self) -> bool {
        self.min.is_nan() || self.max.is_nan()
    }

    /// The eight corners of this bounding box.
    pub fn corners(&self) -> [Vec3; 8] {
        [
            self.min,
            Vec3::new(self.min.x, self.min.y, self.max.z),
            Vec3::new(self.min.x, self.max.y, self.min.z),
            Vec3::new(self.min.x, self.max.y, self.max.z),
            Vec3::new(self.max.x, self.min.y, self.min.z),
            Vec3::new(self.max.x, self.min.y, self.max.z),
            Vec3::new(self.max.x, self.max.y, self.min.z),
            self.max,
        ]
    }

    /// The minimum radius of a sphere, centered at the origin, fully containing the box.
    ///
    /// Requires a well-formed box for the result to be valid.
    pub fn bounding_sphere_radius(&self) -> f32 {
        let mut max_dist_square = 0.0f32;
        for corner in self.corners() {
            max_dist_square = max_dist_square.max(corner.length_squared());
        }
        max_dist_square.sqrt()
    }

    /// The minimum radius of a sphere, centered at the bounding box, fully containing the box.
    ///
    /// Requires a well-formed box for the result to be valid.
    pub fn centered_bounding_sphere_radius(&self) -> f32 {
        let mut max_dist_square = 0.0f32;
        let center = self.center();
        for corner in self.corners() {
            max_dist_square = max_dist_square.max((corner - center).length_squared());
        }
        max_dist_square.sqrt()
    }

    /// The twelve edges of this bounding box.
    pub fn edges(&self) -> [[Vec3; 2]; 12] {
        use glam::vec3;
        let a = self.min;
        let b = self.max;
        [
            // along X:
            [vec3(a.x, a.y, a.z), vec3(b.x, a.y, a.z)],
            [vec3(a.x, b.y, a.z), vec3(b.x, b.y, a.z)],
            [vec3(a.x, a.y, b.z), vec3(b.x, a.y, b.z)],
            [vec3(a.x, b.y, b.z), vec3(b.x, b.y, b.z)],
            // along Y:
            [vec3(a.x, a.y, a.z), vec3(a.x, b.y, a.z)],
            [vec3(b.x, a.y, a.z), vec3(b.x, b.y, a.z)],
            [vec3(a.x, a.y, b.z), vec3(a.x, b.y, b.z)],
            [vec3(b.x, a.y, b.z), vec3(b.x, b.y, b.z)],
            // along Z:
            [vec3(a.x, a.y, a.z), vec3(a.x, a.y, b.z)],
            [vec3(b.x, a.y, a.z), vec3(b.x, a.y, b.z)],
            [vec3(a.x, b.y, a.z), vec3(a.x, b.y, b.z)],
            [vec3(b.x, b.y, a.z), vec3(b.x, b.y, b.z)],
        ]
    }

    /// Enlarge the box to include this point.
    #[inline]
    pub fn extend(&mut self, pos: Vec3) {
        self.min = self.min.min(pos);
        self.max = self.max.max(pos);
    }

    #[must_use]
    pub fn union(mut self, other: Self) -> Self {
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
        self
    }

    /// Returns the smallest volume that is covered by both `self` and `other`,
    /// or [`Self::nothing`] if the boxes are disjoint.
    #[must_use]
    pub fn intersection(mut self, other: Self) -> Self {
        let intersection = Self {
            min: self.min.max(other.min),
            max: self.max.min(other.max),
        };
        if intersection.is_nothing() {
            Self::nothing()
        } else {
            intersection
        }
    }

    /// Returns `true` if the point is within (or on the edge of) the box.
    #[must_use]
    pub fn contains(&self, point: Vec3) -> bool {
        (self.min.x <= point.x && point.x <= self.max.x)
            && (self.min.y <= point.y && point.y <= self.max.y)
            && (self.min.z <= point.z && point.z <= self.max.z)
    }

    /// Expand with this much padding on each side.
    #[must_use]
    pub fn expanded(&self, padding: Vec3) -> Self {
        Self {
            min: self.min - padding,
            max: self.max + padding,
        }
    }

    /// Translate (move) the box by this much.
    #[must_use]
    pub fn translated(&self, translation: Vec3) -> Self {
        Self {
            min: self.min + translation,
            max: self.max + translation,
        }
    }

    /// Return a bounding box that contains this box after it has been rotated around [`Vec3::ZERO`].
    ///
    /// Note that the rotated bounding box is very likely larger than the original,
    /// since it must be large enough to contain the now rotated box.
    #[must_use]
    pub fn rotated_around_origin(&self, q: &Quat) -> Self {
        if self.is_nothing() {
            Self::nothing()
        } else {
            // This can be optimized
            Self::from_points(self.corners().iter().map(|&c| q.mul_vec3(c)))
        }
    }

    /// Return a bounding box that contains this box after it has been transformed.
    ///
    /// Note that the rotated bounding box is very likely larger than the original,
    /// since it must be large enough to contain the now rotated box.
    #[must_use]
    pub fn transform_iso(&self, m: &IsoTransform) -> Self {
        if self.is_nothing() {
            Self::nothing()
        } else {
            Self::from_points(self.corners().iter().map(|&c| m.transform_point3(c)))
        }
    }

    /// Return a bounding box that contains this box after it has been transformed.
    ///
    /// Note that the rotated bounding box is very likely larger than the original,
    /// since it must be large enough to contain the now rotated box.
    #[must_use]
    pub fn transform_affine3(&self, m: &Affine3A) -> Self {
        if self.is_nothing() {
            Self::nothing()
        } else {
            Self::from_points(self.corners().iter().map(|&c| m.transform_point3(c)))
        }
    }

    /// Return a bounding box that contains this box after it has been transformed.
    ///
    /// Note that the rotated bounding box is very likely larger than the original,
    /// since it must be large enough to contain the now rotated box.
    #[must_use]
    pub fn transform_conformal3(&self, m: &Conformal3) -> Self {
        if self.is_nothing() {
            Self::nothing()
        } else {
            Self::from_points(self.corners().iter().map(|&c| m.transform_point3(c)))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_bounding_box() {
        use crate::Affine3A;
        use crate::Quat;
        let bb = BoundingBox::from_min_max(Vec3::ZERO, Vec3::ZERO);
        assert!(bb.contains(Vec3::ZERO));
        assert!(bb.is_something());
        assert!(!bb.is_nothing());
        let bb_rotated =
            bb.transform_affine3(&Affine3A::from_quat(Quat::from_axis_angle(Vec3::X, 0.5)));
        assert_eq!(bb, bb_rotated);
        let bb_translated =
            bb.transform_affine3(&Affine3A::from_translation(Vec3::new(2.0, 3.0, 5.0)));
        assert!(bb_translated.is_something());
        assert!(bb_translated.contains(Vec3::new(2.0, 3.0, 5.0)));
    }
}
