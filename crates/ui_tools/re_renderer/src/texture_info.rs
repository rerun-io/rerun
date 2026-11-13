use std::borrow::Cow;

/// Utility for dealing with buffers containing raw 2D texture data.
#[derive(Clone, Debug)]
pub struct Texture2DBufferInfo {
    /// How many bytes per row contain actual data.
    pub bytes_per_row_unpadded: u32,

    /// How many bytes per row are required to be allocated in total.
    ///
    /// Padding bytes are always at the end of a row.
    pub bytes_per_row_padded: u32,

    /// Size required for an unpadded buffer.
    pub buffer_size_unpadded: wgpu::BufferAddress,

    /// Size required for a padded buffer as it is read/written from/to the GPU.
    pub buffer_size_padded: wgpu::BufferAddress,
}

impl Texture2DBufferInfo {
    /// Retrieves 2D texture buffer info for a given format & texture size.
    ///
    /// If a single buffer is not possible for all aspects of the texture format, all sizes will be zero.
    #[inline]
    pub fn new(format: wgpu::TextureFormat, extent: wgpu::Extent3d) -> Self {
        let block_dimensions = format.block_dimensions();
        let width_blocks = extent.width / block_dimensions.0;
        let height_blocks = extent.height / block_dimensions.1;

        let block_size = format
            .block_copy_size(Some(wgpu::TextureAspect::All))
            .unwrap_or(0); // This happens if we can't have a single buffer.
        let bytes_per_row_unpadded = width_blocks * block_size;
        let bytes_per_row_padded =
            wgpu::util::align_to(bytes_per_row_unpadded, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);

        Self {
            bytes_per_row_unpadded,
            bytes_per_row_padded,
            buffer_size_unpadded: (bytes_per_row_unpadded * height_blocks) as wgpu::BufferAddress,
            buffer_size_padded: (bytes_per_row_padded * height_blocks) as wgpu::BufferAddress,
        }
    }

    #[inline]
    pub fn num_rows(&self) -> u32 {
        self.buffer_size_padded as u32 / self.bytes_per_row_padded
    }

    /// Removes the padding from a buffer containing gpu texture data.
    ///
    /// The passed in buffer is to be expected to be exactly of size [`Texture2DBufferInfo::buffer_size_padded`].
    ///
    /// Note that if you're passing in gpu data, there no alignment guarantees on the returned slice,
    /// do NOT convert it using [`bytemuck`]. Use [`Texture2DBufferInfo::remove_padding_and_convert`] instead.
    pub fn remove_padding<'a>(&self, buffer: &'a [u8]) -> Cow<'a, [u8]> {
        re_tracing::profile_function!();

        assert_eq!(buffer.len() as wgpu::BufferAddress, self.buffer_size_padded);

        if self.bytes_per_row_padded == self.bytes_per_row_unpadded {
            return Cow::Borrowed(buffer);
        }

        let mut unpadded_buffer = Vec::with_capacity(self.buffer_size_unpadded as _);

        for row in 0..self.num_rows() {
            let offset = (self.bytes_per_row_padded * row) as usize;
            unpadded_buffer.extend_from_slice(
                &buffer[offset..(offset + self.bytes_per_row_unpadded as usize)],
            );
        }

        unpadded_buffer.into()
    }

    /// Removes the padding from a buffer containing gpu texture data and remove convert to a given type.
    ///
    /// The passed in buffer is to be expected to be exactly of size [`Texture2DBufferInfo::buffer_size_padded`].
    ///
    /// The unpadded row size is expected to be a multiple of the size of the target type.
    /// (Which means that, while uncommon, it technically doesn't need to be as big as a block in the pixel - this can be useful for e.g. packing wide bitfields)
    pub fn remove_padding_and_convert<T: bytemuck::Pod>(&self, buffer: &[u8]) -> Vec<T> {
        re_tracing::profile_function!();

        assert_eq!(buffer.len() as wgpu::BufferAddress, self.buffer_size_padded);
        assert!(
            self.bytes_per_row_unpadded
                .is_multiple_of(std::mem::size_of::<T>() as u32)
        );

        // Due to https://github.com/gfx-rs/wgpu/issues/3508 the data might be completely unaligned,
        // so much, that we can't even interpret it as e.g. a u32 slice.
        // Therefore, we have to do a copy of the data regardless of whether it's padded or not.

        let mut unpadded_buffer: Vec<T> = vec![
            T::zeroed();
            (self.num_rows() * self.bytes_per_row_unpadded / std::mem::size_of::<T>() as u32)
                as usize
        ]; // TODO(andreas): Consider using unsafe set_len() instead of vec![] to avoid zeroing the memory.

        // The copy has to happen on a u8 slice, because any other type would assume some alignment that we can't guarantee because of the above.
        let unpadded_buffer_u8_view = bytemuck::cast_slice_mut(&mut unpadded_buffer);

        for row in 0..self.num_rows() {
            let offset_padded = (self.bytes_per_row_padded * row) as usize;
            let offset_unpadded = (self.bytes_per_row_unpadded * row) as usize;
            unpadded_buffer_u8_view
                [offset_unpadded..(offset_unpadded + self.bytes_per_row_unpadded as usize)]
                .copy_from_slice(
                    &buffer[offset_padded..(offset_padded + self.bytes_per_row_unpadded as usize)],
                );
        }

        unpadded_buffer
    }
}

/// Utility for dealing with buffers containing raw 3D texture data.
///
/// Unlike a 2D copy, a copy that spans several depth slices has to tell the GPU how many rows
/// make up a single slice, since each row is padded individually.
#[derive(Clone, Debug)]
pub struct Texture3DBufferInfo {
    /// Layout of a single depth slice.
    pub slice: Texture2DBufferInfo,

    /// How many rows of blocks a single depth slice consists of.
    pub rows_per_image: u32,

    /// Number of depth slices.
    pub depth: u32,

    /// Size required for an unpadded buffer holding all slices.
    pub buffer_size_unpadded: wgpu::BufferAddress,

    /// Size required for a padded buffer holding all slices, as it is read/written from/to the GPU.
    pub buffer_size_padded: wgpu::BufferAddress,
}

impl Texture3DBufferInfo {
    /// Retrieves 3D texture buffer info for a given format & texture size.
    ///
    /// If a single buffer is not possible for all aspects of the texture format, all sizes will be zero.
    #[inline]
    pub fn new(format: wgpu::TextureFormat, extent: wgpu::Extent3d) -> Self {
        let slice = Texture2DBufferInfo::new(format, extent);
        let rows_per_image = extent.height / format.block_dimensions().1;
        let depth = extent.depth_or_array_layers;

        let buffer_size_unpadded = slice.buffer_size_unpadded * depth as wgpu::BufferAddress;
        let buffer_size_padded = slice.buffer_size_padded * depth as wgpu::BufferAddress;

        Self {
            slice,
            rows_per_image,
            depth,
            buffer_size_unpadded,
            buffer_size_padded,
        }
    }

    /// Layout of the data in a buffer holding all slices, as wgpu expects it for a copy.
    #[inline]
    pub fn buffer_layout(&self, offset: wgpu::BufferAddress) -> wgpu::TexelCopyBufferLayout {
        wgpu::TexelCopyBufferLayout {
            offset,
            bytes_per_row: Some(self.slice.bytes_per_row_padded),
            // Only required when copying more than one slice, but always correct.
            rows_per_image: Some(self.rows_per_image),
        }
    }
}

/// The range of values a shader sees when sampling this format.
///
/// `Unorm`, `Float` and sRGB formats sample in `[0, 1]`, `Snorm` in `[-1, 1]`, and integer
/// formats sample as the raw integer cast to float.
pub fn sample_value_range(format: wgpu::TextureFormat) -> [f32; 2] {
    use wgpu::TextureFormat as F;

    #[expect(clippy::match_same_arms)]
    match format {
        // 8-bit unsigned normalized / sRGB
        F::R8Unorm
        | F::Rg8Unorm
        | F::Rgba8Unorm
        | F::Rgba8UnormSrgb
        | F::Bgra8Unorm
        | F::Bgra8UnormSrgb => [0.0, 1.0],

        // 8-bit signed normalized
        F::R8Snorm | F::Rg8Snorm | F::Rgba8Snorm => [-1.0, 1.0],

        // 8-bit integer
        F::R8Uint | F::Rg8Uint | F::Rgba8Uint => [0.0, u8::MAX as f32],
        F::R8Sint | F::Rg8Sint | F::Rgba8Sint => [i8::MIN as f32, i8::MAX as f32],

        // 16-bit normalized
        F::R16Unorm | F::Rg16Unorm | F::Rgba16Unorm => [0.0, 1.0],
        F::R16Snorm | F::Rg16Snorm | F::Rgba16Snorm => [-1.0, 1.0],

        // 16-bit integer
        F::R16Uint | F::Rg16Uint | F::Rgba16Uint => [0.0, u16::MAX as f32],
        F::R16Sint | F::Rg16Sint | F::Rgba16Sint => [i16::MIN as f32, i16::MAX as f32],

        // 16-bit float
        F::R16Float | F::Rg16Float | F::Rgba16Float => [0.0, 1.0],

        // 32-bit integer
        F::R32Uint | F::Rg32Uint | F::Rgba32Uint | F::R64Uint => [0.0, u32::MAX as f32],
        F::R32Sint | F::Rg32Sint | F::Rgba32Sint => [i32::MIN as f32, i32::MAX as f32],

        // 32-bit float
        F::R32Float | F::Rg32Float | F::Rgba32Float => [0.0, 1.0],

        // Packed formats
        F::Rgb10a2Uint => [0.0, 1023.0],
        F::Rgb10a2Unorm | F::Rg11b10Ufloat | F::Rgb9e5Ufloat => [0.0, 1.0],

        // Depth / stencil
        F::Stencil8 => [0.0, u8::MAX as f32],
        F::Depth16Unorm
        | F::Depth24Plus
        | F::Depth24PlusStencil8
        | F::Depth32Float
        | F::Depth32FloatStencil8 => [0.0, 1.0],

        // YUV video formats sample each plane as a float in [0, 1]
        F::NV12 | F::P010 => [0.0, 1.0],

        // All supported compressed formats sample as float in [0, 1]
        F::Bc1RgbaUnorm
        | F::Bc1RgbaUnormSrgb
        | F::Bc2RgbaUnorm
        | F::Bc2RgbaUnormSrgb
        | F::Bc3RgbaUnorm
        | F::Bc3RgbaUnormSrgb
        | F::Bc4RUnorm
        | F::Bc5RgUnorm
        | F::Bc6hRgbUfloat
        | F::Bc6hRgbFloat
        | F::Bc7RgbaUnorm
        | F::Bc7RgbaUnormSrgb
        | F::Etc2Rgb8Unorm
        | F::Etc2Rgb8UnormSrgb
        | F::Etc2Rgb8A1Unorm
        | F::Etc2Rgb8A1UnormSrgb
        | F::Etc2Rgba8Unorm
        | F::Etc2Rgba8UnormSrgb
        | F::EacR11Unorm
        | F::EacRg11Unorm
        | F::Astc { .. } => [0.0, 1.0],

        // Signed-normalized variants of the compressed formats
        F::Bc4RSnorm | F::Bc5RgSnorm | F::EacR11Snorm | F::EacRg11Snorm => [-1.0, 1.0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Padding is per row, so a volume of many thin slices is dominated by padding.
    #[test]
    fn texture3d_buffer_info_pads_every_row_of_every_slice() {
        let info = Texture3DBufferInfo::new(
            wgpu::TextureFormat::R16Float,
            wgpu::Extent3d {
                width: 3,
                height: 5,
                depth_or_array_layers: 2,
            },
        );

        assert_eq!(info.rows_per_image, 5);
        assert_eq!(info.depth, 2);
        assert_eq!(info.slice.bytes_per_row_unpadded, 6);
        assert_eq!(info.slice.bytes_per_row_padded, 256);
        assert_eq!(info.buffer_size_unpadded, 6 * 5 * 2);
        assert_eq!(info.buffer_size_padded, 256 * 5 * 2);
    }

    /// A row that is already aligned needs no padding at all, in which case the upload can bulk copy.
    #[test]
    fn texture3d_buffer_info_needs_no_padding_for_aligned_rows() {
        let info = Texture3DBufferInfo::new(
            wgpu::TextureFormat::R32Float,
            wgpu::Extent3d {
                width: 64,
                height: 2,
                depth_or_array_layers: 3,
            },
        );

        assert_eq!(info.slice.bytes_per_row_unpadded, 256);
        assert_eq!(info.slice.bytes_per_row_padded, 256);
        assert_eq!(info.buffer_size_padded, info.buffer_size_unpadded);
    }
}
