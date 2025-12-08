use super::ImagePlaneDistance;

impl Default for ImagePlaneDistance {
    #[inline]
    fn default() -> Self {
        1.0.into()
    }
}

impl From<ImagePlaneDistance> for f32 {
    #[inline]
    fn from(val: ImagePlaneDistance) -> Self {
        val.0.into()
    }
}
