use super::Plane3D;

impl Plane3D {
    /// The Y^Z plane with normal = +X.
    pub const YZ: Self = Self(crate::datatypes::Plane3D([1.0, 0.0, 0.0, 0.0]));

    /// The Z^X plane with normal = +Y.
    pub const ZX: Self = Self(crate::datatypes::Plane3D([0.0, 1.0, 0.0, 0.0]));

    /// The X^Y plane with normal = +Z.
    pub const XY: Self = Self(crate::datatypes::Plane3D([0.0, 0.0, 1.0, 0.0]));

    /// Create a new plane from a normal and distance.
    ///
    /// The plane will not be normalized upon creation.
    #[inline]
    pub fn new(normal: impl Into<crate::datatypes::Vec3D>, distance: f32) -> Self {
        Self(crate::datatypes::Plane3D::new(normal, distance))
    }
}

#[cfg(feature = "glam")]
impl From<Plane3D> for macaw::Plane3 {
    #[inline]
    fn from(plane: Plane3D) -> Self {
        Self {
            normal: glam::vec3(plane.0.0[0], plane.0.0[1], plane.0.0[2]),
            d: plane.0.0[3],
        }
        .normalized()
    }
}
