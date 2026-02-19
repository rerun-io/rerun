use super::Mat4x4;
use crate::datatypes::Vec4D;

impl Mat4x4 {
    #[rustfmt::skip]
    /// The identity matrix.
    ///
    /// The multiplicative identity, representing no transform.
    pub const IDENTITY: Self = Self([
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ]);

    /// Returns the matrix column for the given `index`.
    ///
    /// # Panics
    ///
    /// Panics if `index` is greater than 3.
    #[inline]
    #[expect(clippy::panic)]
    pub fn col(&self, index: usize) -> Vec4D {
        match index {
            0 => [self.0[0], self.0[1], self.0[2], self.0[3]].into(),
            1 => [self.0[4], self.0[5], self.0[6], self.0[7]].into(),
            2 => [self.0[8], self.0[9], self.0[10], self.0[11]].into(),
            3 => [self.0[12], self.0[13], self.0[14], self.0[15]].into(),
            _ => panic!("index out of bounds"),
        }
    }
}

impl<Idx> std::ops::Index<Idx> for Mat4x4
where
    Idx: std::slice::SliceIndex<[f32]>,
{
    type Output = Idx::Output;

    #[inline]
    fn index(&self, index: Idx) -> &Self::Output {
        &self.0[index]
    }
}

impl From<[Vec4D; 4]> for Mat4x4 {
    #[inline]
    #[rustfmt::skip]
    fn from(v: [Vec4D; 4]) -> Self {
        Self([
            v[0].x(), v[0].y(), v[0].z(), v[0].w(),
            v[1].x(), v[1].y(), v[1].z(), v[1].w(),
            v[2].x(), v[2].y(), v[2].z(), v[2].w(),
            v[3].x(), v[3].y(), v[3].z(), v[3].w(),
        ])
    }
}

impl From<Mat4x4> for [Vec4D; 4] {
    fn from(val: Mat4x4) -> Self {
        [
            [val.0[0], val.0[1], val.0[2], val.0[3]].into(),
            [val.0[4], val.0[5], val.0[6], val.0[7]].into(),
            [val.0[8], val.0[9], val.0[10], val.0[11]].into(),
            [val.0[12], val.0[13], val.0[14], val.0[15]].into(),
        ]
    }
}

impl From<[[f32; 4]; 4]> for Mat4x4 {
    #[inline]
    fn from(v: [[f32; 4]; 4]) -> Self {
        Self::from([Vec4D(v[0]), Vec4D(v[1]), Vec4D(v[2]), Vec4D(v[3])])
    }
}

#[cfg(feature = "glam")]
impl From<Mat4x4> for glam::Mat4 {
    #[inline]
    fn from(v: Mat4x4) -> Self {
        let [x, y, z, w]: [Vec4D; 4] = v.into();
        Self::from_cols(x.into(), y.into(), z.into(), w.into())
    }
}

#[cfg(feature = "glam")]
impl From<glam::Mat4> for Mat4x4 {
    #[inline]
    fn from(v: glam::Mat4) -> Self {
        Self::from(v.to_cols_array_2d())
    }
}

#[cfg(feature = "mint")]
impl From<Mat4x4> for mint::ColumnMatrix4<f32> {
    #[inline]
    fn from(v: Mat4x4) -> Self {
        v.0.into()
    }
}

#[cfg(feature = "mint")]
impl From<mint::ColumnMatrix4<f32>> for Mat4x4 {
    #[inline]
    fn from(v: mint::ColumnMatrix4<f32>) -> Self {
        std::convert::From::<[[f32; 4]; 4]>::from([v.x.into(), v.y.into(), v.z.into(), v.w.into()])
    }
}
