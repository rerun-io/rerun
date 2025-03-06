// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/datatypes/tensor_dimension_index_slider.fbs".

#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]
#![allow(clippy::map_flatten)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]

use ::re_types_core::try_serialize_field;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch as _, SerializedComponentBatch};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Datatype**: Defines a slider for the index of some dimension.
#[derive(Clone, Debug, Default, Copy, Hash, PartialEq, Eq)]
pub struct TensorDimensionIndexSlider {
    /// The dimension number.
    pub dimension: u32,
}

::re_types_core::macros::impl_into_cow!(TensorDimensionIndexSlider);

impl ::re_types_core::Loggable for TensorDimensionIndexSlider {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        #![allow(clippy::wildcard_imports)]
        use arrow::datatypes::*;
        DataType::Struct(Fields::from(vec![Field::new(
            "dimension",
            DataType::UInt32,
            false,
        )]))
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<arrow::array::ArrayRef>
    where
        Self: Clone + 'a,
    {
        #![allow(clippy::wildcard_imports)]
        #![allow(clippy::manual_is_variant_and)]
        use ::re_types_core::{arrow_helpers::as_array_ref, Loggable as _, ResultExt as _};
        use arrow::{array::*, buffer::*, datatypes::*};
        Ok({
            let fields = Fields::from(vec![Field::new("dimension", DataType::UInt32, false)]);
            let (somes, data): (Vec<_>, Vec<_>) = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    (datum.is_some(), datum)
                })
                .unzip();
            let validity: Option<arrow::buffer::NullBuffer> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            as_array_ref(StructArray::new(
                fields,
                vec![{
                    let (somes, dimension): (Vec<_>, Vec<_>) = data
                        .iter()
                        .map(|datum| {
                            let datum = datum.as_ref().map(|datum| datum.dimension.clone());
                            (datum.is_some(), datum)
                        })
                        .unzip();
                    let dimension_validity: Option<arrow::buffer::NullBuffer> = {
                        let any_nones = somes.iter().any(|some| !*some);
                        any_nones.then(|| somes.into())
                    };
                    as_array_ref(PrimitiveArray::<UInt32Type>::new(
                        ScalarBuffer::from(
                            dimension
                                .into_iter()
                                .map(|v| v.unwrap_or_default())
                                .collect::<Vec<_>>(),
                        ),
                        dimension_validity,
                    ))
                }],
                validity,
            ))
        })
    }

    fn from_arrow_opt(
        arrow_data: &dyn arrow::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        #![allow(clippy::wildcard_imports)]
        use ::re_types_core::{arrow_zip_validity::ZipValidity, Loggable as _, ResultExt as _};
        use arrow::{array::*, buffer::*, datatypes::*};
        Ok({
            let arrow_data = arrow_data
                .as_any()
                .downcast_ref::<arrow::array::StructArray>()
                .ok_or_else(|| {
                    let expected = Self::arrow_datatype();
                    let actual = arrow_data.data_type().clone();
                    DeserializationError::datatype_mismatch(expected, actual)
                })
                .with_context("rerun.blueprint.datatypes.TensorDimensionIndexSlider")?;
            if arrow_data.is_empty() {
                Vec::new()
            } else {
                let (arrow_data_fields, arrow_data_arrays) =
                    (arrow_data.fields(), arrow_data.columns());
                let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data_fields
                    .iter()
                    .map(|field| field.name().as_str())
                    .zip(arrow_data_arrays)
                    .collect();
                let dimension = {
                    if !arrays_by_name.contains_key("dimension") {
                        return Err(DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "dimension",
                        ))
                        .with_context("rerun.blueprint.datatypes.TensorDimensionIndexSlider");
                    }
                    let arrow_data = &**arrays_by_name["dimension"];
                    arrow_data
                        .as_any()
                        .downcast_ref::<UInt32Array>()
                        .ok_or_else(|| {
                            let expected = DataType::UInt32;
                            let actual = arrow_data.data_type().clone();
                            DeserializationError::datatype_mismatch(expected, actual)
                        })
                        .with_context(
                            "rerun.blueprint.datatypes.TensorDimensionIndexSlider#dimension",
                        )?
                        .into_iter()
                };
                ZipValidity::new_with_validity(
                        ::itertools::izip!(dimension),
                        arrow_data.nulls(),
                    )
                    .map(|opt| {
                        opt
                            .map(|(dimension)| Ok(Self {
                                dimension: dimension
                                    .ok_or_else(DeserializationError::missing_data)
                                    .with_context(
                                        "rerun.blueprint.datatypes.TensorDimensionIndexSlider#dimension",
                                    )?,
                            }))
                            .transpose()
                    })
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context(
                        "rerun.blueprint.datatypes.TensorDimensionIndexSlider",
                    )?
            }
        })
    }
}

impl From<u32> for TensorDimensionIndexSlider {
    #[inline]
    fn from(dimension: u32) -> Self {
        Self { dimension }
    }
}

impl From<TensorDimensionIndexSlider> for u32 {
    #[inline]
    fn from(value: TensorDimensionIndexSlider) -> Self {
        value.dimension
    }
}

impl ::re_byte_size::SizeBytes for TensorDimensionIndexSlider {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.dimension.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <u32>::is_pod()
    }
}
