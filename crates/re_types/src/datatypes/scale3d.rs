// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/datatypes/scale3d.fbs".

#![allow(trivial_numeric_casts)]
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

/// **Datatype**:  3D scaling factor, part of a transform representation.
#[derive(Clone, Debug, Copy, PartialEq)]
pub enum Scale3D {
    /// Individual scaling factors for each axis, distorting the original object.
    ThreeD(crate::datatypes::Vec3D),

    /// Uniform scaling factor along all axis.
    Uniform(f32),
}

impl<'a> From<Scale3D> for ::std::borrow::Cow<'a, Scale3D> {
    #[inline]
    fn from(value: Scale3D) -> Self {
        std::borrow::Cow::Owned(value)
    }
}

impl<'a> From<&'a Scale3D> for ::std::borrow::Cow<'a, Scale3D> {
    #[inline]
    fn from(value: &'a Scale3D) -> Self {
        std::borrow::Cow::Borrowed(value)
    }
}

impl crate::Loggable for Scale3D {
    type Name = crate::DatatypeName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.datatypes.Scale3D".into()
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use ::arrow2::datatypes::*;
        DataType::Union(
            vec![
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
            ],
            Some(vec![0i32, 1i32, 2i32]),
            UnionMode::Dense,
        )
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> crate::SerializationResult<Box<dyn ::arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        re_tracing::profile_function!();
        use crate::{Loggable as _, ResultExt as _};
        use ::arrow2::{array::*, datatypes::*};
        Ok({
            let data: Vec<_> = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    datum
                })
                .collect();
            UnionArray::new(
                <crate::datatypes::Scale3D>::arrow_datatype(),
                data.iter()
                    .map(|a| match a.as_deref() {
                        None => 0,
                        Some(Scale3D::ThreeD(_)) => 1i8,
                        Some(Scale3D::Uniform(_)) => 2i8,
                    })
                    .collect(),
                vec![
                    NullArray::new(DataType::Null, data.iter().filter(|v| v.is_none()).count())
                        .boxed(),
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
                        let three_d_bitmap: Option<::arrow2::bitmap::Bitmap> = {
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
                            let three_d_inner_bitmap: Option<::arrow2::bitmap::Bitmap> =
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
                                    Box::new(Field {
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
                        let uniform_bitmap: Option<::arrow2::bitmap::Bitmap> = {
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
                ],
                Some({
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
                }),
            )
            .boxed()
        })
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    fn from_arrow_opt(
        arrow_data: &dyn ::arrow2::array::Array,
    ) -> crate::DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        re_tracing::profile_function!();
        use crate::{Loggable as _, ResultExt as _};
        use ::arrow2::{array::*, buffer::*, datatypes::*};
        Ok({
            let arrow_data = arrow_data
                .as_any()
                .downcast_ref::<::arrow2::array::UnionArray>()
                .ok_or_else(|| {
                    crate::DeserializationError::datatype_mismatch(
                        DataType::Union(
                            vec![
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
                            ],
                            Some(vec![0i32, 1i32, 2i32]),
                            UnionMode::Dense,
                        ),
                        arrow_data.data_type().clone(),
                    )
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
                        crate::DeserializationError::datatype_mismatch(
                            Self::arrow_datatype(),
                            arrow_data.data_type().clone(),
                        )
                    })
                    .with_context("rerun.datatypes.Scale3D")?;
                if arrow_data_types.len() != arrow_data_offsets.len() {
                    return Err(crate::DeserializationError::offset_slice_oob(
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
                            .downcast_ref::<::arrow2::array::FixedSizeListArray>()
                            .ok_or_else(|| {
                                crate::DeserializationError::datatype_mismatch(
                                    DataType::FixedSizeList(
                                        Box::new(Field {
                                            name: "item".to_owned(),
                                            data_type: DataType::Float32,
                                            is_nullable: false,
                                            metadata: [].into(),
                                        }),
                                        3usize,
                                    ),
                                    arrow_data.data_type().clone(),
                                )
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
                                        crate::DeserializationError::datatype_mismatch(
                                            DataType::Float32,
                                            arrow_data_inner.data_type().clone(),
                                        )
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
                                        return Err(crate::DeserializationError::offset_slice_oob(
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
                            .collect::<crate::DeserializationResult<Vec<Option<_>>>>()?
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
                            crate::DeserializationError::datatype_mismatch(
                                DataType::Float32,
                                arrow_data.data_type().clone(),
                            )
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
                                        return Err(crate::DeserializationError::offset_oob(
                                            offset as _,
                                            three_d.len(),
                                        ))
                                        .with_context("rerun.datatypes.Scale3D#ThreeD");
                                    }

                                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                    unsafe { three_d.get_unchecked(offset as usize) }
                                        .clone()
                                        .ok_or_else(crate::DeserializationError::missing_data)
                                        .with_context("rerun.datatypes.Scale3D#ThreeD")?
                                }),
                                2i8 => Scale3D::Uniform({
                                    if offset as usize >= uniform.len() {
                                        return Err(crate::DeserializationError::offset_oob(
                                            offset as _,
                                            uniform.len(),
                                        ))
                                        .with_context("rerun.datatypes.Scale3D#Uniform");
                                    }

                                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                    unsafe { uniform.get_unchecked(offset as usize) }
                                        .clone()
                                        .ok_or_else(crate::DeserializationError::missing_data)
                                        .with_context("rerun.datatypes.Scale3D#Uniform")?
                                }),
                                _ => {
                                    return Err(crate::DeserializationError::missing_union_arm(
                                        Self::arrow_datatype(),
                                        "<invalid>",
                                        *typ as _,
                                    ))
                                    .with_context("rerun.datatypes.Scale3D");
                                }
                            }))
                        }
                    })
                    .collect::<crate::DeserializationResult<Vec<_>>>()
                    .with_context("rerun.datatypes.Scale3D")?
            }
        })
    }
}
