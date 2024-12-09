// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs".

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

use ::re_types_core::external::arrow2;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, ComponentBatchCowWithDescriptor};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FlattenedScalar {
    pub value: f32,
}

::re_types_core::macros::impl_into_cow!(FlattenedScalar);

impl ::re_types_core::Loggable for FlattenedScalar {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        #![allow(clippy::wildcard_imports)]
        use arrow::datatypes::*;
        DataType::Struct(Fields::from(vec![Field::new(
            "value",
            DataType::Float32,
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
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow::{array::*, buffer::*, datatypes::*};

        #[allow(unused)]
        fn as_array_ref<T: Array + 'static>(t: T) -> ArrayRef {
            std::sync::Arc::new(t) as ArrayRef
        }
        Ok({
            let fields = Fields::from(vec![Field::new("value", DataType::Float32, false)]);
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
                    let (somes, value): (Vec<_>, Vec<_>) = data
                        .iter()
                        .map(|datum| {
                            let datum = datum.as_ref().map(|datum| datum.value.clone());
                            (datum.is_some(), datum)
                        })
                        .unzip();
                    let value_validity: Option<arrow::buffer::NullBuffer> = {
                        let any_nones = somes.iter().any(|some| !*some);
                        any_nones.then(|| somes.into())
                    };
                    as_array_ref(PrimitiveArray::<Float32Type>::new(
                        ScalarBuffer::from(
                            value
                                .into_iter()
                                .map(|v| v.unwrap_or_default())
                                .collect::<Vec<_>>(),
                        ),
                        value_validity,
                    ))
                }],
                validity,
            ))
        })
    }

    fn from_arrow2_opt(
        arrow_data: &dyn arrow2::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        #![allow(clippy::wildcard_imports)]
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow::datatypes::*;
        use arrow2::{array::*, buffer::*};
        Ok({
            let arrow_data = arrow_data
                .as_any()
                .downcast_ref::<arrow2::array::StructArray>()
                .ok_or_else(|| {
                    let expected = Self::arrow_datatype();
                    let actual = arrow_data.data_type().clone();
                    DeserializationError::datatype_mismatch(expected, actual)
                })
                .with_context("rerun.testing.datatypes.FlattenedScalar")?;
            if arrow_data.is_empty() {
                Vec::new()
            } else {
                let (arrow_data_fields, arrow_data_arrays) =
                    (arrow_data.fields(), arrow_data.values());
                let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data_fields
                    .iter()
                    .map(|field| field.name.as_str())
                    .zip(arrow_data_arrays)
                    .collect();
                let value = {
                    if !arrays_by_name.contains_key("value") {
                        return Err(DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "value",
                        ))
                        .with_context("rerun.testing.datatypes.FlattenedScalar");
                    }
                    let arrow_data = &**arrays_by_name["value"];
                    arrow_data
                        .as_any()
                        .downcast_ref::<Float32Array>()
                        .ok_or_else(|| {
                            let expected = DataType::Float32;
                            let actual = arrow_data.data_type().clone();
                            DeserializationError::datatype_mismatch(expected, actual)
                        })
                        .with_context("rerun.testing.datatypes.FlattenedScalar#value")?
                        .into_iter()
                        .map(|opt| opt.copied())
                };
                arrow2::bitmap::utils::ZipValidity::new_with_validity(
                    ::itertools::izip!(value),
                    arrow_data.validity(),
                )
                .map(|opt| {
                    opt.map(|(value)| {
                        Ok(Self {
                            value: value
                                .ok_or_else(DeserializationError::missing_data)
                                .with_context("rerun.testing.datatypes.FlattenedScalar#value")?,
                        })
                    })
                    .transpose()
                })
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.testing.datatypes.FlattenedScalar")?
            }
        })
    }
}

impl From<f32> for FlattenedScalar {
    #[inline]
    fn from(value: f32) -> Self {
        Self { value }
    }
}

impl From<FlattenedScalar> for f32 {
    #[inline]
    fn from(value: FlattenedScalar) -> Self {
        value.value
    }
}

impl ::re_types_core::SizeBytes for FlattenedScalar {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.value.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <f32>::is_pod()
    }
}
