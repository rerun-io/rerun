use super::Plane3D;

impl Plane3D {
    /// The Y^Z plane with normal = +X.
    pub const YZ: Self = Self([1.0, 0.0, 0.0, 0.0]);

    /// The Z^X plane with normal = +Y.
    pub const ZX: Self = Self([0.0, 1.0, 0.0, 0.0]);

    /// The X^Y plane with normal = +Z.
    pub const XY: Self = Self([0.0, 0.0, 1.0, 0.0]);

    /// The normal of the plane (unnormalized if the plane is unnormalized).
    #[inline]
    pub const fn normal(&self) -> super::Vec3D {
        super::Vec3D([self.0[0], self.0[1], self.0[2]])
    }

    /// The distance of the plane from the origin (in multiples of the normal if the normal is unnormalized).
    #[inline]
    pub const fn distance(&self) -> f32 {
        self.0[3]
    }

    /// Create a new plane from a normal and distance.
    ///
    /// The plane will not be normalized upon creation.
    #[inline]
    pub fn new(normal: impl Into<crate::datatypes::Vec3D>, distance: f32) -> Self {
        let normal = normal.into();
        Self([normal.0[0], normal.0[1], normal.0[2], distance])
    }
}

#[cfg(feature = "glam")]
impl From<macaw::Plane3> for Plane3D {
    #[inline]
    fn from(plane: macaw::Plane3) -> Self {
        Self([plane.normal.x, plane.normal.y, plane.normal.z, plane.d])
    }
}

#[cfg(feature = "glam")]
impl From<Plane3D> for macaw::Plane3 {
    #[inline]
    fn from(plane: Plane3D) -> Self {
        Self {
            normal: glam::vec3(plane.0[0], plane.0[1], plane.0[2]),
            d: plane.0[3],
        }
        .normalized()
    }
}
