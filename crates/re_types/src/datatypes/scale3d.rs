// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/datatypes/scale3d.fbs".

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

/// **Datatype**: 3D scaling factor, part of a transform representation.
#[derive(Clone, Debug, Copy, PartialEq)]
pub enum Scale3D {
    /// Individual scaling factors for each axis, distorting the original object.
    ThreeD(crate::datatypes::Vec3D),

    /// Uniform scaling factor along all axis.
    Uniform(f32),
}

impl ::re_types_core::SizeBytes for Scale3D {
    #[allow(clippy::match_same_arms)]
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Self::ThreeD(v) => v.heap_size_bytes(),
            Self::Uniform(v) => v.heap_size_bytes(),
        }
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::datatypes::Vec3D>::is_pod() && <f32>::is_pod()
    }
}

::re_types_core::macros::impl_into_cow!(Scale3D);

impl ::re_types_core::Loggable for Scale3D {
    type Name = ::re_types_core::DatatypeName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.datatypes.Scale3D".into()
    }

    #[allow(clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use arrow2::datatypes::*;
        DataType::Union(
            std::sync::Arc::new(vec![
                Field {
                    name: "_null_markers".to_owned(),
                    data_type: DataType::Null,
                    is_nullable: true,
                    metadata: [].into(),
                },
                Field {
                    name: "ThreeD".to_owned(),
                    data_type: <crate::datatypes::Vec3D>::arrow_datatype(),
                    is_nullable: false,
                    metadata: [].into(),
                },
                Field {
                    name: "Uniform".to_owned(),
                    data_type: DataType::Float32,
                    is_nullable: false,
                    metadata: [].into(),
                },
            ]),
            Some(std::sync::Arc::new(vec![0i32, 1i32, 2i32])),
            UnionMode::Dense,
        )
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
            let data: Vec<_> = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    datum
                })
                .collect();
            let types = data
                .iter()
                .map(|a| match a.as_deref() {
                    None => 0,
                    Some(Scale3D::ThreeD(_)) => 1i8,
                    Some(Scale3D::Uniform(_)) => 2i8,
                })
                .collect();
            let fields = vec![
                NullArray::new(DataType::Null, data.iter().filter(|v| v.is_none()).count()).boxed(),
                {
                    let (somes, three_d): (Vec<_>, Vec<_>) = data
                        .iter()
                        .filter(|datum| matches!(datum.as_deref(), Some(Scale3D::ThreeD(_))))
                        .map(|datum| {
                            let datum = match datum.as_deref() {
                                Some(Scale3D::ThreeD(v)) => Some(v.clone()),
                                _ => None,
                            };
                            (datum.is_some(), datum)
                        })
                        .unzip();
                    let three_d_bitmap: Option<arrow2::bitmap::Bitmap> = {
                        let any_nones = somes.iter().any(|some| !*some);
                        any_nones.then(|| somes.into())
                    };
                    {
                        use arrow2::{buffer::Buffer, offset::OffsetsBuffer};
                        let three_d_inner_data: Vec<_> = three_d
                            .iter()
                            .map(|datum| {
                                datum
                                    .map(|datum| {
                                        let crate::datatypes::Vec3D(data0) = datum;
                                        data0
                                    })
                                    .unwrap_or_default()
                            })
                            .flatten()
                            .map(Some)
                            .collect();
                        let three_d_inner_bitmap: Option<arrow2::bitmap::Bitmap> =
                            three_d_bitmap.as_ref().map(|bitmap| {
                                bitmap
                                    .iter()
                                    .map(|i| std::iter::repeat(i).take(3usize))
                                    .flatten()
                                    .collect::<Vec<_>>()
                                    .into()
                            });
                        FixedSizeListArray::new(
                            DataType::FixedSizeList(
                                std::sync::Arc::new(Field {
                                    name: "item".to_owned(),
                                    data_type: DataType::Float32,
                                    is_nullable: false,
                                    metadata: [].into(),
                                }),
                                3usize,
                            ),
                            PrimitiveArray::new(
                                DataType::Float32,
                                three_d_inner_data
                                    .into_iter()
                                    .map(|v| v.unwrap_or_default())
                                    .collect(),
                                three_d_inner_bitmap,
                            )
                            .boxed(),
                            three_d_bitmap,
                        )
                        .boxed()
                    }
                },
                {
                    let (somes, uniform): (Vec<_>, Vec<_>) = data
                        .iter()
                        .filter(|datum| matches!(datum.as_deref(), Some(Scale3D::Uniform(_))))
                        .map(|datum| {
                            let datum = match datum.as_deref() {
                                Some(Scale3D::Uniform(v)) => Some(v.clone()),
                                _ => None,
                            };
                            (datum.is_some(), datum)
                        })
                        .unzip();
                    let uniform_bitmap: Option<arrow2::bitmap::Bitmap> = {
                        let any_nones = somes.iter().any(|some| !*some);
                        any_nones.then(|| somes.into())
                    };
                    PrimitiveArray::new(
                        DataType::Float32,
                        uniform.into_iter().map(|v| v.unwrap_or_default()).collect(),
                        uniform_bitmap,
                    )
                    .boxed()
                },
            ];
            let offsets = Some({
                let mut three_d_offset = 0;
                let mut uniform_offset = 0;
                let mut nulls_offset = 0;
                data.iter()
                    .map(|v| match v.as_deref() {
                        None => {
                            let offset = nulls_offset;
                            nulls_offset += 1;
                            offset
                        }
                        Some(Scale3D::ThreeD(_)) => {
                            let offset = three_d_offset;
                            three_d_offset += 1;
                            offset
                        }
                        Some(Scale3D::Uniform(_)) => {
                            let offset = uniform_offset;
                            uniform_offset += 1;
                            offset
                        }
                    })
                    .collect()
            });
            UnionArray::new(
                <crate::datatypes::Scale3D>::arrow_datatype(),
                types,
                fields,
                offsets,
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
                .downcast_ref::<arrow2::array::UnionArray>()
                .ok_or_else(|| {
                    let expected = Self::arrow_datatype();
                    let actual = arrow_data.data_type().clone();
                    DeserializationError::datatype_mismatch(expected, actual)
                })
                .with_context("rerun.datatypes.Scale3D")?;
            if arrow_data.is_empty() {
                Vec::new()
            } else {
                let (arrow_data_types, arrow_data_arrays) =
                    (arrow_data.types(), arrow_data.fields());
                let arrow_data_offsets = arrow_data
                    .offsets()
                    .ok_or_else(|| {
                        let expected = Self::arrow_datatype();
                        let actual = arrow_data.data_type().clone();
                        DeserializationError::datatype_mismatch(expected, actual)
                    })
                    .with_context("rerun.datatypes.Scale3D")?;
                if arrow_data_types.len() != arrow_data_offsets.len() {
                    return Err(DeserializationError::offset_slice_oob(
                        (0, arrow_data_types.len()),
                        arrow_data_offsets.len(),
                    ))
                    .with_context("rerun.datatypes.Scale3D");
                }
                let three_d = {
                    if 1usize >= arrow_data_arrays.len() {
                        return Ok(Vec::new());
                    }
                    let arrow_data = &*arrow_data_arrays[1usize];
                    {
                        let arrow_data = arrow_data
                            .as_any()
                            .downcast_ref::<arrow2::array::FixedSizeListArray>()
                            .ok_or_else(|| {
                                let expected = DataType::FixedSizeList(
                                    std::sync::Arc::new(Field {
                                        name: "item".to_owned(),
                                        data_type: DataType::Float32,
                                        is_nullable: false,
                                        metadata: [].into(),
                                    }),
                                    3usize,
                                );
                                let actual = arrow_data.data_type().clone();
                                DeserializationError::datatype_mismatch(expected, actual)
                            })
                            .with_context("rerun.datatypes.Scale3D#ThreeD")?;
                        if arrow_data.is_empty() {
                            Vec::new()
                        } else {
                            let offsets = (0..)
                                .step_by(3usize)
                                .zip((3usize..).step_by(3usize).take(arrow_data.len()));
                            let arrow_data_inner = {
                                let arrow_data_inner = &**arrow_data.values();
                                arrow_data_inner
                                    .as_any()
                                    .downcast_ref::<Float32Array>()
                                    .ok_or_else(|| {
                                        let expected = DataType::Float32;
                                        let actual = arrow_data_inner.data_type().clone();
                                        DeserializationError::datatype_mismatch(expected, actual)
                                    })
                                    .with_context("rerun.datatypes.Scale3D#ThreeD")?
                                    .into_iter()
                                    .map(|opt| opt.copied())
                                    .collect::<Vec<_>>()
                            };
                            arrow2::bitmap::utils::ZipValidity::new_with_validity(
                                offsets,
                                arrow_data.validity(),
                            )
                            .map(|elem| {
                                elem.map(|(start, end)| {
                                    debug_assert!(end - start == 3usize);
                                    if end as usize > arrow_data_inner.len() {
                                        return Err(DeserializationError::offset_slice_oob(
                                            (start, end),
                                            arrow_data_inner.len(),
                                        ));
                                    }

                                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                    let data = unsafe {
                                        arrow_data_inner.get_unchecked(start as usize..end as usize)
                                    };
                                    let data = data.iter().cloned().map(Option::unwrap_or_default);
                                    let arr = array_init::from_iter(data).unwrap();
                                    Ok(arr)
                                })
                                .transpose()
                            })
                            .map(|res_or_opt| {
                                res_or_opt.map(|res_or_opt| {
                                    res_or_opt.map(|v| crate::datatypes::Vec3D(v))
                                })
                            })
                            .collect::<DeserializationResult<Vec<Option<_>>>>()?
                        }
                        .into_iter()
                    }
                    .collect::<Vec<_>>()
                };
                let uniform = {
                    if 2usize >= arrow_data_arrays.len() {
                        return Ok(Vec::new());
                    }
                    let arrow_data = &*arrow_data_arrays[2usize];
                    arrow_data
                        .as_any()
                        .downcast_ref::<Float32Array>()
                        .ok_or_else(|| {
                            let expected = DataType::Float32;
                            let actual = arrow_data.data_type().clone();
                            DeserializationError::datatype_mismatch(expected, actual)
                        })
                        .with_context("rerun.datatypes.Scale3D#Uniform")?
                        .into_iter()
                        .map(|opt| opt.copied())
                        .collect::<Vec<_>>()
                };
                arrow_data_types
                    .iter()
                    .enumerate()
                    .map(|(i, typ)| {
                        let offset = arrow_data_offsets[i];
                        if *typ == 0 {
                            Ok(None)
                        } else {
                            Ok(Some(match typ {
                                1i8 => Scale3D::ThreeD({
                                    if offset as usize >= three_d.len() {
                                        return Err(DeserializationError::offset_oob(
                                            offset as _,
                                            three_d.len(),
                                        ))
                                        .with_context("rerun.datatypes.Scale3D#ThreeD");
                                    }

                                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                    unsafe { three_d.get_unchecked(offset as usize) }
                                        .clone()
                                        .ok_or_else(DeserializationError::missing_data)
                                        .with_context("rerun.datatypes.Scale3D#ThreeD")?
                                }),
                                2i8 => Scale3D::Uniform({
                                    if offset as usize >= uniform.len() {
                                        return Err(DeserializationError::offset_oob(
                                            offset as _,
                                            uniform.len(),
                                        ))
                                        .with_context("rerun.datatypes.Scale3D#Uniform");
                                    }

                                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                    unsafe { uniform.get_unchecked(offset as usize) }
                                        .clone()
                                        .ok_or_else(DeserializationError::missing_data)
                                        .with_context("rerun.datatypes.Scale3D#Uniform")?
                                }),
                                _ => {
                                    return Err(DeserializationError::missing_union_arm(
                                        Self::arrow_datatype(),
                                        "<invalid>",
                                        *typ as _,
                                    ))
                                    .with_context("rerun.datatypes.Scale3D");
                                }
                            }))
                        }
                    })
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.datatypes.Scale3D")?
            }
        })
    }
}
