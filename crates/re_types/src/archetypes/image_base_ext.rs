use crate::datatypes::{ImageVariant, TensorDimension};

use super::ImageBase;

// ----------------------------------------------------------------------------
impl ImageBase {
    pub fn with_id(self, id: crate::datatypes::TensorId) -> Self {
        Self {
            variant: self.variant,
            data: crate::datatypes::TensorData {
                id,
                shape: self.data.0.shape,
                buffer: self.data.0.buffer,
            }
            .into(),
        }
    }
}
// ----------------------------------------------------------------------------

/// An Monochrome or RGB Image
#[derive(Clone, Debug, PartialEq)]
pub struct Image(ImageBase);

impl Image {
    pub const NUM_COMPONENTS: usize = ImageBase::NUM_COMPONENTS;

    pub fn base(&self) -> &ImageBase {
        &self.0
    }

    pub fn data(&self) -> &crate::datatypes::TensorData {
        &self.base().data.0
    }
}

impl crate::Archetype for Image {
    #[inline]
    fn name() -> crate::ArchetypeName {
        crate::ArchetypeName::Borrowed("rerun.archetypes.Tensor")
    }

    #[inline]
    fn required_components() -> &'static [crate::ComponentName] {
        ImageBase::recommended_components()
    }

    #[inline]
    fn recommended_components() -> &'static [crate::ComponentName] {
        ImageBase::recommended_components()
    }

    #[inline]
    fn optional_components() -> &'static [crate::ComponentName] {
        ImageBase::optional_components()
    }

    #[inline]
    fn all_components() -> &'static [crate::ComponentName] {
        ImageBase::all_components()
    }

    #[inline]
    fn try_to_arrow(
        &self,
    ) -> crate::SerializationResult<
        Vec<(::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)>,
    > {
        self.0.try_to_arrow()
    }

    #[inline]
    fn try_from_arrow(
        arrow_data: impl IntoIterator<
            Item = (::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>),
        >,
    ) -> crate::DeserializationResult<Self> {
        let base = ImageBase::try_from_arrow(arrow_data)?;

        Ok(Self(base))
    }
}

#[derive(thiserror::Error, Clone, Debug)]
pub enum ImageConstructionError<T: TryInto<crate::datatypes::TensorData>> {
    #[error("Could not convert source to TensorData")]
    TensorDataConversion(T::Error),

    #[error("Could not create Image from TensorData with shape {0:?}")]
    BadImageShape(Vec<TensorDimension>),
}

impl Image {
    pub fn try_from<T: TryInto<crate::datatypes::TensorData>>(
        data: T,
    ) -> Result<Self, ImageConstructionError<T>> {
        let mut data: crate::datatypes::TensorData = data
            .try_into()
            .map_err(|e| ImageConstructionError::TensorDataConversion(e))?;

        let variant = match data.shape.len() {
            2 => {
                data.shape[0].name = Some("height".into());
                data.shape[1].name = Some("width".into());
                ImageVariant::Mono(true)
            }
            3 => match data.shape[2].size {
                3 => {
                    data.shape[0].name = Some("height".into());
                    data.shape[1].name = Some("width".into());
                    data.shape[2].name = Some("color".into());
                    ImageVariant::Rgb(true)
                }
                4 => {
                    data.shape[0].name = Some("height".into());
                    data.shape[1].name = Some("width".into());
                    data.shape[2].name = Some("color".into());
                    ImageVariant::Rgba(true)
                }
                _ => return Err(ImageConstructionError::BadImageShape(data.shape)),
            },
            _ => return Err(ImageConstructionError::BadImageShape(data.shape)),
        };

        Ok(Self(ImageBase {
            variant: variant.into(),
            data: data.into(),
        }))
    }

    pub fn with_id(self, id: crate::datatypes::TensorId) -> Self {
        Self(self.0.with_id(id))
    }
}

// ----------------------------------------------------------------------------

macro_rules! forward_array_views {
    ($type:ty, $alias:ty) => {
        impl<'a> TryFrom<&'a $alias> for ::ndarray::ArrayViewD<'a, $type> {
            type Error = crate::datatypes::TensorCastError;

            #[inline]
            fn try_from(value: &'a $alias) -> Result<Self, Self::Error> {
                value.data().try_into()
            }
        }
    };
}

forward_array_views!(u8, Image);
forward_array_views!(f32, Image);
forward_array_views!(f64, Image);

// ----------------------------------------------------------------------------
