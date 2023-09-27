// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::iter_on_single_items)]
#![allow(clippy::map_flatten)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_cast)]

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FlattenedScalar {
    pub value: f32,
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

impl<'a> From<FlattenedScalar> for ::std::borrow::Cow<'a, FlattenedScalar> {
    #[inline]
    fn from(value: FlattenedScalar) -> Self {
        std::borrow::Cow::Owned(value)
    }
}

impl<'a> From<&'a FlattenedScalar> for ::std::borrow::Cow<'a, FlattenedScalar> {
    #[inline]
    fn from(value: &'a FlattenedScalar) -> Self {
        std::borrow::Cow::Borrowed(value)
    }
}

impl crate::Loggable for FlattenedScalar {
    type Name = crate::DatatypeName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.testing.datatypes.FlattenedScalar".into()
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use ::arrow2::datatypes::*;
        DataType::Struct(vec![Field {
            name: "value".to_owned(),
            data_type: DataType::Float32,
            is_nullable: false,
            metadata: [].into(),
        }])
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> crate::SerializationResult<Box<dyn ::arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        use crate::{Loggable as _, ResultExt as _};
        use ::arrow2::{array::*, datatypes::*};
        Ok({
            let (somes, data): (Vec<_>, Vec<_>) = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    (datum.is_some(), datum)
                })
                .unzip();
            let bitmap: Option<::arrow2::bitmap::Bitmap> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            StructArray::new(
                <crate::testing::datatypes::FlattenedScalar>::arrow_datatype(),
                vec![{
                    let (somes, value): (Vec<_>, Vec<_>) = data
                        .iter()
                        .map(|datum| {
                            let datum = datum.as_ref().map(|datum| {
                                let Self { value, .. } = &**datum;
                                value.clone()
                            });
                            (datum.is_some(), datum)
                        })
                        .unzip();
                    let value_bitmap: Option<::arrow2::bitmap::Bitmap> = {
                        let any_nones = somes.iter().any(|some| !*some);
                        any_nones.then(|| somes.into())
                    };
                    PrimitiveArray::new(
                        DataType::Float32,
                        value.into_iter().map(|v| v.unwrap_or_default()).collect(),
                        value_bitmap,
                    )
                    .boxed()
                }],
                bitmap,
            )
            .boxed()
        })
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    fn try_from_arrow_opt(
        arrow_data: &dyn ::arrow2::array::Array,
    ) -> crate::DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        use crate::{Loggable as _, ResultExt as _};
        use ::arrow2::{array::*, buffer::*, datatypes::*};
        Ok({
            let arrow_data = arrow_data
                .as_any()
                .downcast_ref::<::arrow2::array::StructArray>()
                .ok_or_else(|| {
                    crate::DeserializationError::datatype_mismatch(
                        DataType::Struct(vec![Field {
                            name: "value".to_owned(),
                            data_type: DataType::Float32,
                            is_nullable: false,
                            metadata: [].into(),
                        }]),
                        arrow_data.data_type().clone(),
                    )
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
                        return Err(crate::DeserializationError::missing_struct_field(
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
                            crate::DeserializationError::datatype_mismatch(
                                DataType::Float32,
                                arrow_data.data_type().clone(),
                            )
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
                                .ok_or_else(crate::DeserializationError::missing_data)
                                .with_context("rerun.testing.datatypes.FlattenedScalar#value")?,
                        })
                    })
                    .transpose()
                })
                .collect::<crate::DeserializationResult<Vec<_>>>()
                .with_context("rerun.testing.datatypes.FlattenedScalar")?
            }
        })
    }
}
