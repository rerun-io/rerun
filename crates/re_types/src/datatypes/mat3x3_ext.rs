use super::Mat3x3;

use crate::datatypes::Vec3D;

impl Mat3x3 {
    #[rustfmt::skip]
    pub const IDENTITY: Mat3x3 = Mat3x3([
        1.0, 0.0, 0.0,
        0.0, 1.0, 0.0,
        0.0, 0.0, 1.0,
    ]);

    /// Returns the matrix column for the given `index`.
    ///
    /// # Panics
    ///
    /// Panics if `index` is greater than 2.
    #[inline]
    pub fn col(&self, index: usize) -> Vec3D {
        match index {
            0 => [self.0[0], self.0[1], self.0[2]].into(),
            1 => [self.0[3], self.0[4], self.0[5]].into(),
            2 => [self.0[6], self.0[7], self.0[8]].into(),
            _ => panic!("index out of bounds"),
        }
    }
}

impl<Idx> std::ops::Index<Idx> for Mat3x3
where
    Idx: std::slice::SliceIndex<[f32]>,
{
    type Output = Idx::Output;

    #[inline]
    fn index(&self, index: Idx) -> &Self::Output {
        &self.0[index]
    }
}

impl From<[Vec3D; 3]> for Mat3x3 {
    #[inline]
    #[rustfmt::skip]
    fn from(v: [Vec3D; 3]) -> Self {
        Self([
            v[0].x(), v[0].y(), v[0].z(),
            v[1].x(), v[1].y(), v[1].z(),
            v[2].x(), v[2].y(), v[2].z(),
        ])
    }
}

impl From<Mat3x3> for [Vec3D; 3] {
    fn from(val: Mat3x3) -> Self {
        [
            [val.0[0], val.0[1], val.0[2]].into(), //
            [val.0[3], val.0[4], val.0[5]].into(), //
            [val.0[6], val.0[7], val.0[8]].into(), //
        ]
    }
}

impl From<[[f32; 3]; 3]> for Mat3x3 {
    #[inline]
    fn from(v: [[f32; 3]; 3]) -> Self {
        Self::from([Vec3D(v[0]), Vec3D(v[1]), Vec3D(v[2])])
    }
}

#[cfg(feature = "glam")]
impl From<Mat3x3> for glam::Mat3 {
    #[inline]
    fn from(v: Mat3x3) -> Self {
        let [x, y, z]: [Vec3D; 3] = v.into();
        Self::from_cols(x.into(), y.into(), z.into())
    }
}

#[cfg(feature = "glam")]
impl From<glam::Mat3> for Mat3x3 {
    #[inline]
    fn from(v: glam::Mat3) -> Self {
        Self::from(v.to_cols_array_2d())
    }
}
