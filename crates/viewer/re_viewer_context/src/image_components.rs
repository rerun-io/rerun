use re_chunk::RowId;
use re_types::{
    components::{ColorModel, Colormap, ElementType},
    datatypes::Blob,
    tensor_data::TensorElement,
};

/// Represents an `Image`, `SegmentationImage` or `DepthImage`.
#[derive(Clone)]
pub struct ImageComponents {
    pub row_id: RowId,

    /// The image data, row-wise, with stride=width.
    pub blob: Blob,

    /// Width and height
    pub resolution: [u32; 2],

    /// The element type.
    pub element_type: ElementType,

    /// `None` for depth images and segmentation images,
    /// `Some` for color images.
    pub color_model: Option<ColorModel>,

    /// Primarily for depth images atm
    pub colormap: Option<Colormap>,
    // TODO(#6386): `PixelFormat` and `ColorModel`
}

impl ImageComponents {
    #[inline]
    pub fn width(&self) -> u32 {
        self.resolution[0]
    }

    #[inline]
    pub fn height(&self) -> u32 {
        self.resolution[1]
    }

    /// 1 for grayscale and depth images, 3 for RGB, etc
    #[inline]
    pub fn components_per_pixel(&self) -> usize {
        self.color_model.map_or(1, ColorModel::num_components)
    }

    #[inline]
    pub fn bits_per_texel(&self) -> usize {
        // TODO(#6386): use `PixelFormat`
        self.element_type.bits() * self.components_per_pixel()
    }

    /// Get the value of the element at the given index.
    ///
    /// Return `None` if out-of-bounds.
    #[inline]
    pub fn get_xy(&self, x: u32, y: u32) -> Option<TensorElement> {
        if x >= self.width() {
            return None;
        }

        fn get<T: bytemuck::Pod>(blob: &[u8], element_offset: usize) -> Option<T> {
            // NOTE: `blob` is not necessary aligned to `T`,
            // hence the complexity of this function.

            let size = std::mem::size_of::<T>();
            let byte_offset = element_offset * size;
            if blob.len() <= byte_offset + size {
                return None;
            }

            let slice = &blob[byte_offset..byte_offset + size];

            let mut dest = T::zeroed();
            bytemuck::bytes_of_mut(&mut dest).copy_from_slice(slice);
            Some(dest)
        }

        let offset = y as usize * self.width() as usize + x as usize;

        match self.element_type {
            ElementType::U8 => self.blob.get(offset).copied().map(TensorElement::U8),
            ElementType::U16 => get(&self.blob, offset).map(TensorElement::U16),
            ElementType::U32 => get(&self.blob, offset).map(TensorElement::U32),
            ElementType::U64 => get(&self.blob, offset).map(TensorElement::U64),

            ElementType::I8 => get(&self.blob, offset).map(TensorElement::I8),
            ElementType::I16 => get(&self.blob, offset).map(TensorElement::I16),
            ElementType::I32 => get(&self.blob, offset).map(TensorElement::I32),
            ElementType::I64 => get(&self.blob, offset).map(TensorElement::I64),

            ElementType::F16 => get(&self.blob, offset).map(TensorElement::F16),
            ElementType::F32 => get(&self.blob, offset).map(TensorElement::F32),
            ElementType::F64 => get(&self.blob, offset).map(TensorElement::F64),
        }
    }

    /// Total number of elements in the image, e.g. `W x H x 3` for an RGB image.
    #[inline]
    pub fn num_elements(&self) -> usize {
        self.blob.len() * 8 / self.bits_per_texel()
    }
}
