use super::Vec3D;

impl Vec3D {
    pub const ZERO: Vec3D = Vec3D([0.0; 3]);
    pub const ONE: Vec3D = Vec3D([1.0; 3]);

    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self([x, y, z])
    }

    #[inline]
    pub fn x(&self) -> f32 {
        self.0[0]
    }

    #[inline]
    pub fn y(&self) -> f32 {
        self.0[1]
    }

    #[inline]
    pub fn z(&self) -> f32 {
        self.0[2]
    }
}

impl From<(f32, f32, f32)> for Vec3D {
    #[inline]
    fn from((x, y, z): (f32, f32, f32)) -> Self {
        Self::new(x, y, z)
    }
}

// NOTE: All these by-ref impls make the lives of end-users much easier when juggling around with
// slices, because Rust cannot keep track of the inherent `Copy` capability of it all across all the
// layers of `Into`/`IntoIterator`.

impl<'a> From<&'a Vec3D> for Vec3D {
    fn from(v: &'a Vec3D) -> Self {
        Self(v.0)
    }
}

impl<'a> From<&'a (f32, f32, f32)> for Vec3D {
    #[inline]
    fn from((x, y, z): &'a (f32, f32, f32)) -> Self {
        Self::new(*x, *y, *z)
    }
}

impl<'a> From<&'a [f32; 3]> for Vec3D {
    #[inline]
    fn from(v: &'a [f32; 3]) -> Self {
        Self(*v)
    }
}

impl<Idx> std::ops::Index<Idx> for Vec3D
where
    Idx: std::slice::SliceIndex<[f32]>,
{
    type Output = Idx::Output;

    #[inline]
    fn index(&self, index: Idx) -> &Self::Output {
        &self.0[index]
    }
}

#[cfg(feature = "glam")]
impl From<Vec3D> for glam::Vec3 {
    #[inline]
    fn from(v: Vec3D) -> Self {
        Self::from_slice(&v.0)
    }
}

#[cfg(feature = "glam")]
impl From<glam::Vec3> for Vec3D {
    #[inline]
    fn from(v: glam::Vec3) -> Self {
        Self(v.to_array())
    }
}

#[cfg(feature = "mint")]
impl From<Vec3D> for mint::Vector3<f32> {
    #[inline]
    fn from(v: Vec3D) -> Self {
        Self {
            x: v[0],
            y: v[1],
            z: v[2],
        }
    }
}

#[cfg(feature = "mint")]
impl From<mint::Vector3<f32>> for Vec3D {
    #[inline]
    fn from(v: mint::Vector3<f32>) -> Self {
        Self([v.x, v.y, v.z])
    }
}

impl std::fmt::Display for Vec3D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:.prec$}, {:.prec$}, {:.prec$}]",
            self.x(),
            self.y(),
            self.z(),
            prec = crate::DISPLAY_PRECISION,
        )
    }
}
