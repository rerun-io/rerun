use super::Tensor;

// ----------------------------------------------------------------------------

// ----------------------------------------------------------------------------

/// A Generic Image
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub data: crate::components::TensorData,
}

impl Image {
    pub const NUM_COMPONENTS: usize = Tensor::NUM_COMPONENTS;
}

impl crate::Archetype for Image {
    #[inline]
    fn name() -> crate::ArchetypeName {
        crate::ArchetypeName::Borrowed("rerun.archetypes.Tensor")
    }

    #[inline]
    fn required_components() -> &'static [crate::ComponentName] {
        Tensor::recommended_components()
    }

    #[inline]
    fn recommended_components() -> &'static [crate::ComponentName] {
        Tensor::recommended_components()
    }

    #[inline]
    fn optional_components() -> &'static [crate::ComponentName] {
        Tensor::optional_components()
    }

    #[inline]
    fn all_components() -> &'static [crate::ComponentName] {
        Tensor::all_components()
    }

    #[inline]
    fn try_to_arrow(
        &self,
    ) -> crate::SerializationResult<
        Vec<(::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)>,
    > {
        let tensor = Tensor {
            meaning: Some(crate::datatypes::TensorMeaning::Rgba(true).into()),
            data: self.data.clone(),
        };

        tensor.try_to_arrow()
    }

    #[inline]
    fn try_from_arrow(
        arrow_data: impl IntoIterator<
            Item = (::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>),
        >,
    ) -> crate::DeserializationResult<Self> {
        let tensor = Tensor::try_from_arrow(arrow_data)?;
        // TODO(jleibs): Maybe error if tensor-meaning is wrong
        Ok(Image { data: tensor.data })
    }
}

impl Image {
    pub fn try_from<T: TryInto<crate::datatypes::TensorData>>(data: T) -> Result<Self, T::Error> {
        Ok(Self {
            data: crate::components::TensorData(data.try_into()?),
        })
    }
}

// ----------------------------------------------------------------------------
