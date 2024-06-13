// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/datatypes/material.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]
#![allow(clippy::iter_on_single_items)]
#![allow(clippy::map_flatten)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_cast)]

use ::re_types_core::external::arrow2;
use ::re_types_core::ComponentName;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Datatype**: Material properties of a mesh.
#[derive(Clone, Debug, Copy, PartialEq, Eq, Hash)]
pub struct Material {
    /// Optional color multiplier.
    pub albedo_factor: Option<crate::datatypes::Rgba32>,
}

impl ::re_types_core::SizeBytes for Material {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.albedo_factor.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Option<crate::datatypes::Rgba32>>::is_pod()
    }
}

impl<T: Into<Option<crate::datatypes::Rgba32>>> From<T> for Material {
    fn from(v: T) -> Self {
        Self {
            albedo_factor: v.into(),
        }
    }
}

impl std::borrow::Borrow<Option<crate::datatypes::Rgba32>> for Material {
    #[inline]
    fn borrow(&self) -> &Option<crate::datatypes::Rgba32> {
        &self.albedo_factor
    }
}

impl std::ops::Deref for Material {
    type Target = Option<crate::datatypes::Rgba32>;

    #[inline]
    fn deref(&self) -> &Option<crate::datatypes::Rgba32> {
        &self.albedo_factor
    }
}

impl std::ops::DerefMut for Material {
    #[inline]
    fn deref_mut(&mut self) -> &mut Option<crate::datatypes::Rgba32> {
        &mut self.albedo_factor
    }
}

::re_types_core::macros::impl_into_cow!(Material);

impl ::re_types_core::Loggable for Material {
    type Name = ::re_types_core::DatatypeName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.datatypes.Material".into()
    }

    #[allow(clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use arrow2::datatypes::*;
        DataType::Struct(std::sync::Arc::new(vec![Field::new(
            "albedo_factor",
            <crate::datatypes::Rgba32>::arrow_datatype(),
            true,
        )]))
    }

    #[allow(clippy::wildcard_imports)]
    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, datatypes::*};
        Ok({
            let (somes, data): (Vec<_>, Vec<_>) = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    (datum.is_some(), datum)
                })
                .unzip();
            let bitmap: Option<arrow2::bitmap::Bitmap> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            StructArray::new(
                Self::arrow_datatype(),
                vec![{
                    let (somes, albedo_factor): (Vec<_>, Vec<_>) = data
                        .iter()
                        .map(|datum| {
                            let datum = datum
                                .as_ref()
                                .map(|datum| datum.albedo_factor.clone())
                                .flatten();
                            (datum.is_some(), datum)
                        })
                        .unzip();
                    let albedo_factor_bitmap: Option<arrow2::bitmap::Bitmap> = {
                        let any_nones = somes.iter().any(|some| !*some);
                        any_nones.then(|| somes.into())
                    };
                    PrimitiveArray::new(
                        DataType::UInt32,
                        albedo_factor
                            .into_iter()
                            .map(|datum| datum.map(|datum| datum.0).unwrap_or_default())
                            .collect(),
                        albedo_factor_bitmap,
                    )
                    .boxed()
                }],
                bitmap,
            )
            .boxed()
        })
    }

    #[allow(clippy::wildcard_imports)]
    fn from_arrow_opt(
        arrow_data: &dyn arrow2::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, buffer::*, datatypes::*};
        Ok({
            let arrow_data = arrow_data
                .as_any()
                .downcast_ref::<arrow2::array::StructArray>()
                .ok_or_else(|| {
                    let expected = Self::arrow_datatype();
                    let actual = arrow_data.data_type().clone();
                    DeserializationError::datatype_mismatch(expected, actual)
                })
                .with_context("rerun.datatypes.Material")?;
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
                let albedo_factor = {
                    if !arrays_by_name.contains_key("albedo_factor") {
                        return Err(DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "albedo_factor",
                        ))
                        .with_context("rerun.datatypes.Material");
                    }
                    let arrow_data = &**arrays_by_name["albedo_factor"];
                    arrow_data
                        .as_any()
                        .downcast_ref::<UInt32Array>()
                        .ok_or_else(|| {
                            let expected = DataType::UInt32;
                            let actual = arrow_data.data_type().clone();
                            DeserializationError::datatype_mismatch(expected, actual)
                        })
                        .with_context("rerun.datatypes.Material#albedo_factor")?
                        .into_iter()
                        .map(|opt| opt.copied())
                        .map(|res_or_opt| res_or_opt.map(|v| crate::datatypes::Rgba32(v)))
                };
                arrow2::bitmap::utils::ZipValidity::new_with_validity(
                    ::itertools::izip!(albedo_factor),
                    arrow_data.validity(),
                )
                .map(|opt| {
                    opt.map(|(albedo_factor)| Ok(Self { albedo_factor }))
                        .transpose()
                })
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.datatypes.Material")?
            }
        })
    }
}
