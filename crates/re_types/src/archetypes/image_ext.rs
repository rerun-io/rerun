use crate::{
    datatypes::TensorData,
    image::{find_non_empty_dim_indices, ImageConstructionError},
};

use super::Image;

impl Image {
    /// Try to construct an [`Image`] from anything that can be converted into [`TensorData`]
    ///
    /// Will return an [`ImageConstructionError`] if the shape of the tensor data is invalid
    /// for treating as an image.
    ///
    /// This is useful for constructing an [`Image`] from an ndarray.
    pub fn try_from<T: TryInto<TensorData>>(data: T) -> Result<Self, ImageConstructionError<T>>
    where
        <T as TryInto<TensorData>>::Error: std::error::Error,
    {
        let mut data: TensorData = data
            .try_into()
            .map_err(ImageConstructionError::TensorDataConversion)?;

        let non_empty_dim_inds = find_non_empty_dim_indices(&data.shape);

        match non_empty_dim_inds.len() {
            2 => {
                assign_if_none(&mut data.shape[non_empty_dim_inds[0]].name, "height");
                assign_if_none(&mut data.shape[non_empty_dim_inds[1]].name, "width");
            }
            3 => match data.shape[non_empty_dim_inds[2]].size {
                1 | 3 | 4 => {
                    assign_if_none(&mut data.shape[non_empty_dim_inds[0]].name, "height");
                    assign_if_none(&mut data.shape[non_empty_dim_inds[1]].name, "width");
                    assign_if_none(&mut data.shape[non_empty_dim_inds[2]].name, "depth");
                }
                _ => return Err(ImageConstructionError::BadImageShape(data.shape)),
            },
            _ => return Err(ImageConstructionError::BadImageShape(data.shape)),
        };

        Ok(Self {
            data: data.into(),
            draw_order: None,
        })
    }

    /// Creates a new [`Image`] from a file.
    ///
    /// The image format will be inferred from the path (extension), or the contents if that fails.
    #[cfg(feature = "image")]
    #[cfg(not(target_arch = "wasm32"))]
    #[inline]
    pub fn from_file_path(filepath: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        let filepath = filepath.as_ref();
        Ok(Self::new(crate::datatypes::TensorData::from_image_file(
            filepath,
        )?))
    }

    /// Creates a new [`Image`] from the contents of a file.
    ///
    /// If unspecified, the image format will be inferred from the contents.
    #[cfg(feature = "image")]
    #[inline]
    pub fn from_file_contents(
        contents: Vec<u8>,
        format: Option<image::ImageFormat>,
    ) -> anyhow::Result<Self> {
        let format = if let Some(format) = format {
            format
        } else {
            image::guess_format(&contents)?
        };

        let tensor = crate::components::TensorData(crate::datatypes::TensorData::from_image_bytes(
            contents, format,
        )?);

        Ok(Self::new(tensor))
    }
}

fn assign_if_none(name: &mut Option<::re_types_core::ArrowString>, new_name: &str) {
    if name.is_none() {
        *name = Some(new_name.into());
    }
}

// ----------------------------------------------------------------------------
// Make it possible to create an ArrayView directly from an Image.

macro_rules! forward_array_views {
    ($type:ty, $alias:ty) => {
        impl<'a> TryFrom<&'a $alias> for ::ndarray::ArrayViewD<'a, $type> {
            type Error = crate::tensor_data::TensorCastError;

            #[inline]
            fn try_from(value: &'a $alias) -> Result<Self, Self::Error> {
                (&value.data.0).try_into()
            }
        }
    };
}

forward_array_views!(u8, Image);
forward_array_views!(u16, Image);
forward_array_views!(u32, Image);
forward_array_views!(u64, Image);

forward_array_views!(i8, Image);
forward_array_views!(i16, Image);
forward_array_views!(i32, Image);
forward_array_views!(i64, Image);

forward_array_views!(half::f16, Image);
forward_array_views!(f32, Image);
forward_array_views!(f64, Image);
