// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/datatypes/material.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
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
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    /// Optional color multiplier.
    pub albedo_factor: Option<crate::datatypes::Rgba32>,

    /// Optional albedo texture.
    ///
    /// Used with `vertex_texcoords` on `Mesh3D`.
    /// Currently supports only RGB & RGBA sRGBA textures, ignoring alpha.
    /// (meaning that the tensor must have 3 or 4 channels and use the `u8` format)
    pub albedo_texture: Option<crate::datatypes::TensorData>,
}

impl ::re_types_core::SizeBytes for Material {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.albedo_factor.heap_size_bytes() + self.albedo_texture.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Option<crate::datatypes::Rgba32>>::is_pod()
            && <Option<crate::datatypes::TensorData>>::is_pod()
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
        DataType::Struct(vec![
            Field {
                name: "albedo_factor".to_owned(),
                data_type: <crate::datatypes::Rgba32>::arrow_datatype(),
                is_nullable: true,
                metadata: [].into(),
            },
            Field {
                name: "albedo_texture".to_owned(),
                data_type: <crate::datatypes::TensorData>::arrow_datatype(),
                is_nullable: true,
                metadata: [].into(),
            },
        ])
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
                <crate::datatypes::Material>::arrow_datatype(),
                vec![
                    {
                        let (somes, albedo_factor): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum
                                    .as_ref()
                                    .map(|datum| {
                                        let Self { albedo_factor, .. } = &**datum;
                                        albedo_factor.clone()
                                    })
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
                                .map(|datum| {
                                    datum
                                        .map(|datum| {
                                            let crate::datatypes::Rgba32(data0) = datum;
                                            data0
                                        })
                                        .unwrap_or_default()
                                })
                                .collect(),
                            albedo_factor_bitmap,
                        )
                        .boxed()
                    },
                    {
                        let (somes, albedo_texture): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum
                                    .as_ref()
                                    .map(|datum| {
                                        let Self { albedo_texture, .. } = &**datum;
                                        albedo_texture.clone()
                                    })
                                    .flatten();
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let albedo_texture_bitmap: Option<arrow2::bitmap::Bitmap> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        {
                            _ = albedo_texture_bitmap;
                            crate::datatypes::TensorData::to_arrow_opt(albedo_texture)?
                        }
                    },
                ],
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
                    DeserializationError::datatype_mismatch(
                        DataType::Struct(vec![
                            Field {
                                name: "albedo_factor".to_owned(),
                                data_type: <crate::datatypes::Rgba32>::arrow_datatype(),
                                is_nullable: true,
                                metadata: [].into(),
                            },
                            Field {
                                name: "albedo_texture".to_owned(),
                                data_type: <crate::datatypes::TensorData>::arrow_datatype(),
                                is_nullable: true,
                                metadata: [].into(),
                            },
                        ]),
                        arrow_data.data_type().clone(),
                    )
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
                            DeserializationError::datatype_mismatch(
                                DataType::UInt32,
                                arrow_data.data_type().clone(),
                            )
                        })
                        .with_context("rerun.datatypes.Material#albedo_factor")?
                        .into_iter()
                        .map(|opt| opt.copied())
                        .map(|res_or_opt| res_or_opt.map(|v| crate::datatypes::Rgba32(v)))
                };
                let albedo_texture = {
                    if !arrays_by_name.contains_key("albedo_texture") {
                        return Err(DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "albedo_texture",
                        ))
                        .with_context("rerun.datatypes.Material");
                    }
                    let arrow_data = &**arrays_by_name["albedo_texture"];
                    crate::datatypes::TensorData::from_arrow_opt(arrow_data)
                        .with_context("rerun.datatypes.Material#albedo_texture")?
                        .into_iter()
                };
                arrow2::bitmap::utils::ZipValidity::new_with_validity(
                    ::itertools::izip!(albedo_factor, albedo_texture),
                    arrow_data.validity(),
                )
                .map(|opt| {
                    opt.map(|(albedo_factor, albedo_texture)| {
                        Ok(Self {
                            albedo_factor,
                            albedo_texture,
                        })
                    })
                    .transpose()
                })
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.datatypes.Material")?
            }
        })
    }
}
