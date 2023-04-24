use std::borrow::Cow;

/// Utility for dealing with buffers containing raw 2D texture data.
#[derive(Clone)]
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
    pub fn new(format: wgpu::TextureFormat, extent: glam::UVec2) -> Self {
        let block_dimensions = format.block_dimensions();
        let width_blocks = extent.x / block_dimensions.0;
        let height_blocks = extent.y / block_dimensions.1;

        let block_size = format
            .block_size(Some(wgpu::TextureAspect::All))
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
        crate::profile_function!();

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
        crate::profile_function!();

        assert_eq!(buffer.len() as wgpu::BufferAddress, self.buffer_size_padded);
        assert!(self.bytes_per_row_unpadded % std::mem::size_of::<T>() as u32 == 0);

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

pub fn is_float_filterable(format: wgpu::TextureFormat, device_features: wgpu::Features) -> bool {
    format
        .guaranteed_format_features(device_features)
        .flags
        .contains(wgpu::TextureFormatFeatureFlags::FILTERABLE)
}

pub fn num_texture_components(format: wgpu::TextureFormat) -> u8 {
    #[allow(clippy::match_same_arms)]
    match format {
        wgpu::TextureFormat::R8Unorm
        | wgpu::TextureFormat::R8Snorm
        | wgpu::TextureFormat::R8Uint
        | wgpu::TextureFormat::R8Sint
        | wgpu::TextureFormat::R16Uint
        | wgpu::TextureFormat::R16Sint
        | wgpu::TextureFormat::R16Unorm
        | wgpu::TextureFormat::R16Snorm
        | wgpu::TextureFormat::R16Float
        | wgpu::TextureFormat::R32Uint
        | wgpu::TextureFormat::R32Sint
        | wgpu::TextureFormat::R32Float => 1,

        wgpu::TextureFormat::Rg8Unorm
        | wgpu::TextureFormat::Rg8Snorm
        | wgpu::TextureFormat::Rg8Uint
        | wgpu::TextureFormat::Rg8Sint
        | wgpu::TextureFormat::Rg16Uint
        | wgpu::TextureFormat::Rg16Sint
        | wgpu::TextureFormat::Rg16Unorm
        | wgpu::TextureFormat::Rg16Snorm
        | wgpu::TextureFormat::Rg16Float
        | wgpu::TextureFormat::Rg32Uint
        | wgpu::TextureFormat::Rg32Sint
        | wgpu::TextureFormat::Rg32Float => 2,

        wgpu::TextureFormat::Rgba8Unorm
        | wgpu::TextureFormat::Rgba8UnormSrgb
        | wgpu::TextureFormat::Rgba8Snorm
        | wgpu::TextureFormat::Rgba8Uint
        | wgpu::TextureFormat::Rgba8Sint
        | wgpu::TextureFormat::Bgra8Unorm
        | wgpu::TextureFormat::Bgra8UnormSrgb
        | wgpu::TextureFormat::Rgba16Uint
        | wgpu::TextureFormat::Rgba16Sint
        | wgpu::TextureFormat::Rgba16Unorm
        | wgpu::TextureFormat::Rgba16Snorm
        | wgpu::TextureFormat::Rgba16Float
        | wgpu::TextureFormat::Rgba32Uint
        | wgpu::TextureFormat::Rgba32Sint
        | wgpu::TextureFormat::Rgba32Float => 4,

        wgpu::TextureFormat::Rgb9e5Ufloat | wgpu::TextureFormat::Rg11b10Float => 3,
        wgpu::TextureFormat::Rgb10a2Unorm => 4,

        wgpu::TextureFormat::Stencil8
        | wgpu::TextureFormat::Depth16Unorm
        | wgpu::TextureFormat::Depth24Plus
        | wgpu::TextureFormat::Depth32Float => 1,

        // It's complicated. Each aspect has actually only a single channel.
        wgpu::TextureFormat::Depth24PlusStencil8 | wgpu::TextureFormat::Depth32FloatStencil8 => 2,

        wgpu::TextureFormat::Bc1RgbaUnorm
        | wgpu::TextureFormat::Bc1RgbaUnormSrgb
        | wgpu::TextureFormat::Bc2RgbaUnorm
        | wgpu::TextureFormat::Bc2RgbaUnormSrgb
        | wgpu::TextureFormat::Bc3RgbaUnorm
        | wgpu::TextureFormat::Bc3RgbaUnormSrgb
        | wgpu::TextureFormat::Bc4RUnorm
        | wgpu::TextureFormat::Bc4RSnorm
        | wgpu::TextureFormat::Bc5RgUnorm
        | wgpu::TextureFormat::Bc5RgSnorm
        | wgpu::TextureFormat::Bc6hRgbUfloat
        | wgpu::TextureFormat::Bc6hRgbFloat
        | wgpu::TextureFormat::Bc7RgbaUnorm
        | wgpu::TextureFormat::Bc7RgbaUnormSrgb
        | wgpu::TextureFormat::Etc2Rgb8Unorm
        | wgpu::TextureFormat::Etc2Rgb8UnormSrgb
        | wgpu::TextureFormat::Etc2Rgb8A1Unorm
        | wgpu::TextureFormat::Etc2Rgb8A1UnormSrgb
        | wgpu::TextureFormat::Etc2Rgba8Unorm
        | wgpu::TextureFormat::Etc2Rgba8UnormSrgb
        | wgpu::TextureFormat::EacR11Unorm
        | wgpu::TextureFormat::EacR11Snorm
        | wgpu::TextureFormat::EacRg11Unorm
        | wgpu::TextureFormat::EacRg11Snorm
        | wgpu::TextureFormat::Astc { .. } => 4,
    }
}
