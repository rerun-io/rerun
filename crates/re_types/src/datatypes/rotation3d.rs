// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/datatypes/rotation3d.fbs".

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

/// **Datatype**: A 3D rotation.
#[derive(Clone, Debug, Copy, PartialEq)]
pub enum Rotation3D {
    /// Rotation defined by a quaternion.
    Quaternion(crate::datatypes::Quaternion),

    /// Rotation defined with an axis and an angle.
    AxisAngle(crate::datatypes::RotationAxisAngle),
}

impl ::re_types_core::SizeBytes for Rotation3D {
    #[allow(clippy::match_same_arms)]
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Self::Quaternion(v) => v.heap_size_bytes(),
            Self::AxisAngle(v) => v.heap_size_bytes(),
        }
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::datatypes::Quaternion>::is_pod() && <crate::datatypes::RotationAxisAngle>::is_pod()
    }
}

::re_types_core::macros::impl_into_cow!(Rotation3D);

impl ::re_types_core::Loggable for Rotation3D {
    type Name = ::re_types_core::DatatypeName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.datatypes.Rotation3D".into()
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
                    name: "Quaternion".to_owned(),
                    data_type: <crate::datatypes::Quaternion>::arrow_datatype(),
                    is_nullable: false,
                    metadata: [].into(),
                },
                Field {
                    name: "AxisAngle".to_owned(),
                    data_type: <crate::datatypes::RotationAxisAngle>::arrow_datatype(),
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
                    Some(Rotation3D::Quaternion(_)) => 1i8,
                    Some(Rotation3D::AxisAngle(_)) => 2i8,
                })
                .collect();
            let fields = vec![
                NullArray::new(DataType::Null, data.iter().filter(|v| v.is_none()).count()).boxed(),
                {
                    let (somes, quaternion): (Vec<_>, Vec<_>) = data
                        .iter()
                        .filter(|datum| matches!(datum.as_deref(), Some(Rotation3D::Quaternion(_))))
                        .map(|datum| {
                            let datum = match datum.as_deref() {
                                Some(Rotation3D::Quaternion(v)) => Some(v.clone()),
                                _ => None,
                            };
                            (datum.is_some(), datum)
                        })
                        .unzip();
                    let quaternion_bitmap: Option<arrow2::bitmap::Bitmap> = {
                        let any_nones = somes.iter().any(|some| !*some);
                        any_nones.then(|| somes.into())
                    };
                    {
                        use arrow2::{buffer::Buffer, offset::OffsetsBuffer};
                        let quaternion_inner_data: Vec<_> = quaternion
                            .iter()
                            .map(|datum| {
                                datum
                                    .map(|datum| {
                                        let crate::datatypes::Quaternion(data0) = datum;
                                        data0
                                    })
                                    .unwrap_or_default()
                            })
                            .flatten()
                            .map(Some)
                            .collect();
                        let quaternion_inner_bitmap: Option<arrow2::bitmap::Bitmap> =
                            quaternion_bitmap.as_ref().map(|bitmap| {
                                bitmap
                                    .iter()
                                    .map(|i| std::iter::repeat(i).take(4usize))
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
                                4usize,
                            ),
                            PrimitiveArray::new(
                                DataType::Float32,
                                quaternion_inner_data
                                    .into_iter()
                                    .map(|v| v.unwrap_or_default())
                                    .collect(),
                                quaternion_inner_bitmap,
                            )
                            .boxed(),
                            quaternion_bitmap,
                        )
                        .boxed()
                    }
                },
                {
                    let (somes, axis_angle): (Vec<_>, Vec<_>) = data
                        .iter()
                        .filter(|datum| matches!(datum.as_deref(), Some(Rotation3D::AxisAngle(_))))
                        .map(|datum| {
                            let datum = match datum.as_deref() {
                                Some(Rotation3D::AxisAngle(v)) => Some(v.clone()),
                                _ => None,
                            };
                            (datum.is_some(), datum)
                        })
                        .unzip();
                    let axis_angle_bitmap: Option<arrow2::bitmap::Bitmap> = {
                        let any_nones = somes.iter().any(|some| !*some);
                        any_nones.then(|| somes.into())
                    };
                    {
                        _ = axis_angle_bitmap;
                        crate::datatypes::RotationAxisAngle::to_arrow_opt(axis_angle)?
                    }
                },
            ];
            let offsets = Some({
                let mut quaternion_offset = 0;
                let mut axis_angle_offset = 0;
                let mut nulls_offset = 0;
                data.iter()
                    .map(|v| match v.as_deref() {
                        None => {
                            let offset = nulls_offset;
                            nulls_offset += 1;
                            offset
                        }
                        Some(Rotation3D::Quaternion(_)) => {
                            let offset = quaternion_offset;
                            quaternion_offset += 1;
                            offset
                        }
                        Some(Rotation3D::AxisAngle(_)) => {
                            let offset = axis_angle_offset;
                            axis_angle_offset += 1;
                            offset
                        }
                    })
                    .collect()
            });
            UnionArray::new(
                <crate::datatypes::Rotation3D>::arrow_datatype(),
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
                    DeserializationError::datatype_mismatch(
                        DataType::Union(
                            std::sync::Arc::new(vec![
                                Field {
                                    name: "_null_markers".to_owned(),
                                    data_type: DataType::Null,
                                    is_nullable: true,
                                    metadata: [].into(),
                                },
                                Field {
                                    name: "Quaternion".to_owned(),
                                    data_type: <crate::datatypes::Quaternion>::arrow_datatype(),
                                    is_nullable: false,
                                    metadata: [].into(),
                                },
                                Field {
                                    name: "AxisAngle".to_owned(),
                                    data_type:
                                        <crate::datatypes::RotationAxisAngle>::arrow_datatype(),
                                    is_nullable: false,
                                    metadata: [].into(),
                                },
                            ]),
                            Some(std::sync::Arc::new(vec![0i32, 1i32, 2i32])),
                            UnionMode::Dense,
                        ),
                        arrow_data.data_type().clone(),
                    )
                })
                .with_context("rerun.datatypes.Rotation3D")?;
            if arrow_data.is_empty() {
                Vec::new()
            } else {
                let (arrow_data_types, arrow_data_arrays) =
                    (arrow_data.types(), arrow_data.fields());
                let arrow_data_offsets = arrow_data
                    .offsets()
                    .ok_or_else(|| {
                        DeserializationError::datatype_mismatch(
                            Self::arrow_datatype(),
                            arrow_data.data_type().clone(),
                        )
                    })
                    .with_context("rerun.datatypes.Rotation3D")?;
                if arrow_data_types.len() != arrow_data_offsets.len() {
                    return Err(DeserializationError::offset_slice_oob(
                        (0, arrow_data_types.len()),
                        arrow_data_offsets.len(),
                    ))
                    .with_context("rerun.datatypes.Rotation3D");
                }
                let quaternion = {
                    if 1usize >= arrow_data_arrays.len() {
                        return Ok(Vec::new());
                    }
                    let arrow_data = &*arrow_data_arrays[1usize];
                    {
                        let arrow_data = arrow_data
                            .as_any()
                            .downcast_ref::<arrow2::array::FixedSizeListArray>()
                            .ok_or_else(|| {
                                DeserializationError::datatype_mismatch(
                                    DataType::FixedSizeList(
                                        std::sync::Arc::new(Field {
                                            name: "item".to_owned(),
                                            data_type: DataType::Float32,
                                            is_nullable: false,
                                            metadata: [].into(),
                                        }),
                                        4usize,
                                    ),
                                    arrow_data.data_type().clone(),
                                )
                            })
                            .with_context("rerun.datatypes.Rotation3D#Quaternion")?;
                        if arrow_data.is_empty() {
                            Vec::new()
                        } else {
                            let offsets = (0..)
                                .step_by(4usize)
                                .zip((4usize..).step_by(4usize).take(arrow_data.len()));
                            let arrow_data_inner = {
                                let arrow_data_inner = &**arrow_data.values();
                                arrow_data_inner
                                    .as_any()
                                    .downcast_ref::<Float32Array>()
                                    .ok_or_else(|| {
                                        DeserializationError::datatype_mismatch(
                                            DataType::Float32,
                                            arrow_data_inner.data_type().clone(),
                                        )
                                    })
                                    .with_context("rerun.datatypes.Rotation3D#Quaternion")?
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
                                    debug_assert!(end - start == 4usize);
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
                                    res_or_opt.map(|v| crate::datatypes::Quaternion(v))
                                })
                            })
                            .collect::<DeserializationResult<Vec<Option<_>>>>()?
                        }
                        .into_iter()
                    }
                    .collect::<Vec<_>>()
                };
                let axis_angle = {
                    if 2usize >= arrow_data_arrays.len() {
                        return Ok(Vec::new());
                    }
                    let arrow_data = &*arrow_data_arrays[2usize];
                    crate::datatypes::RotationAxisAngle::from_arrow_opt(arrow_data)
                        .with_context("rerun.datatypes.Rotation3D#AxisAngle")?
                        .into_iter()
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
                                1i8 => Rotation3D::Quaternion({
                                    if offset as usize >= quaternion.len() {
                                        return Err(DeserializationError::offset_oob(
                                            offset as _,
                                            quaternion.len(),
                                        ))
                                        .with_context("rerun.datatypes.Rotation3D#Quaternion");
                                    }

                                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                    unsafe { quaternion.get_unchecked(offset as usize) }
                                        .clone()
                                        .ok_or_else(DeserializationError::missing_data)
                                        .with_context("rerun.datatypes.Rotation3D#Quaternion")?
                                }),
                                2i8 => Rotation3D::AxisAngle({
                                    if offset as usize >= axis_angle.len() {
                                        return Err(DeserializationError::offset_oob(
                                            offset as _,
                                            axis_angle.len(),
                                        ))
                                        .with_context("rerun.datatypes.Rotation3D#AxisAngle");
                                    }

                                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                    unsafe { axis_angle.get_unchecked(offset as usize) }
                                        .clone()
                                        .ok_or_else(DeserializationError::missing_data)
                                        .with_context("rerun.datatypes.Rotation3D#AxisAngle")?
                                }),
                                _ => {
                                    return Err(DeserializationError::missing_union_arm(
                                        Self::arrow_datatype(),
                                        "<invalid>",
                                        *typ as _,
                                    ))
                                    .with_context("rerun.datatypes.Rotation3D");
                                }
                            }))
                        }
                    })
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.datatypes.Rotation3D")?
            }
        })
    }
}
