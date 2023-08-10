// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

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

/// 3D scaling factor, part of a transform representation.
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
    type Item<'a> = Option<Self>;
    type Iter<'a> = <Vec<Self::Item<'a>> as IntoIterator>::IntoIter;

    #[inline]
    fn name() -> Self::Name {
        "rerun.datatypes.Scale3D".into()
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    #[inline]
    fn to_arrow_datatype() -> arrow2::datatypes::DataType {
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
                    data_type: <crate::datatypes::Vec3D>::to_arrow_datatype(),
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
    fn try_to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
        extension_wrapper: Option<&str>,
    ) -> crate::SerializationResult<Box<dyn ::arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
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
                (if let Some(ext) = extension_wrapper {
                    DataType::Extension(
                        ext.to_owned(),
                        Box::new(<crate::datatypes::Scale3D>::to_arrow_datatype()),
                        None,
                    )
                } else {
                    <crate::datatypes::Scale3D>::to_arrow_datatype()
                })
                .to_logical_type()
                .clone(),
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
                            let three_d_inner_bitmap: Option<::arrow2::bitmap::Bitmap> = None;
                            FixedSizeListArray::new(
                                {
                                    _ = extension_wrapper;
                                    DataType::FixedSizeList(
                                        Box::new(Field {
                                            name: "item".to_owned(),
                                            data_type: DataType::Float32,
                                            is_nullable: false,
                                            metadata: [].into(),
                                        }),
                                        3usize,
                                    )
                                    .to_logical_type()
                                    .clone()
                                },
                                PrimitiveArray::new(
                                    {
                                        _ = extension_wrapper;
                                        DataType::Float32.to_logical_type().clone()
                                    },
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
                            {
                                _ = extension_wrapper;
                                DataType::Float32.to_logical_type().clone()
                            },
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
    fn try_from_arrow_opt(
        data: &dyn ::arrow2::array::Array,
    ) -> crate::DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        use crate::{Loggable as _, ResultExt as _};
        use ::arrow2::{array::*, datatypes::*};
        Ok({
            let data = data
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
                                    data_type: <crate::datatypes::Vec3D>::to_arrow_datatype(),
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
                        data.data_type().clone(),
                    )
                })
                .with_context("rerun.datatypes.Scale3D")?;
            if data.is_empty() {
                Vec::new()
            } else {
                let (data_types, data_arrays) = (data.types(), data.fields());
                let data_offsets = data
                    .offsets()
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
                                        data_type: <crate::datatypes::Vec3D>::to_arrow_datatype(),
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
                            data.data_type().clone(),
                        )
                    })
                    .with_context("rerun.datatypes.Scale3D")?;
                if data_types.len() > data_offsets.len() {
                    return Err(crate::DeserializationError::offsets_mismatch(
                        (0, data_types.len()),
                        data_offsets.len(),
                    ))
                    .with_context("rerun.datatypes.Scale3D");
                }
                let three_d = {
                    if 1usize >= data_arrays.len() {
                        return Err(crate::DeserializationError::missing_union_arm(
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
                                        data_type: <crate::datatypes::Vec3D>::to_arrow_datatype(),
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
                            "rerun.datatypes.Scale3D#ThreeD",
                            1usize,
                        ))
                        .with_context("rerun.datatypes.Scale3D");
                    }
                    let data = &*data_arrays[1usize];
                    {
                        let data = data
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
                                    data.data_type().clone(),
                                )
                            })
                            .with_context("rerun.datatypes.Scale3D#ThreeD")?;
                        if data.is_empty() {
                            Vec::new()
                        } else {
                            let offsets = (0..)
                                .step_by(3usize)
                                .zip((3usize..).step_by(3usize).take(data.len()));
                            let data_inner = {
                                let data_inner = &**data.values();
                                data_inner
                                    .as_any()
                                    .downcast_ref::<Float32Array>()
                                    .ok_or_else(|| {
                                        crate::DeserializationError::datatype_mismatch(
                                            DataType::Float32,
                                            data_inner.data_type().clone(),
                                        )
                                    })
                                    .with_context("rerun.datatypes.Scale3D#ThreeD")?
                                    .into_iter()
                                    .map(|opt| opt.copied())
                                    .collect::<Vec<_>>()
                            };
                            arrow2::bitmap::utils::ZipValidity::new_with_validity(
                                offsets,
                                data.validity(),
                            )
                            .map(|elem| {
                                elem.map(|(start, end)| {
                                    debug_assert!(end - start == 3usize);
                                    if end as usize > data_inner.len() {
                                        return Err(crate::DeserializationError::offsets_mismatch(
                                            (start, end),
                                            data_inner.len(),
                                        ));
                                    }

                                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                    let data = unsafe {
                                        data_inner.get_unchecked(start as usize..end as usize)
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
                    if 2usize >= data_arrays.len() {
                        return Err(crate::DeserializationError::missing_union_arm(
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
                                        data_type: <crate::datatypes::Vec3D>::to_arrow_datatype(),
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
                            "rerun.datatypes.Scale3D#Uniform",
                            2usize,
                        ))
                        .with_context("rerun.datatypes.Scale3D");
                    }
                    let data = &*data_arrays[2usize];
                    data.as_any()
                        .downcast_ref::<Float32Array>()
                        .ok_or_else(|| {
                            crate::DeserializationError::datatype_mismatch(
                                DataType::Float32,
                                data.data_type().clone(),
                            )
                        })
                        .with_context("rerun.datatypes.Scale3D#Uniform")?
                        .into_iter()
                        .map(|opt| opt.copied())
                        .collect::<Vec<_>>()
                };
                data_types
                    .iter()
                    .enumerate()
                    .map(|(i, typ)| {
                        let offset = data_offsets[i];
                        if *typ == 0 {
                            Ok(None)
                        } else {
                            Ok(Some(match typ {
                                1i8 => Scale3D::ThreeD({
                                    if offset as usize >= three_d.len() {
                                        return Err(crate::DeserializationError::offsets_mismatch(
                                            (offset as _, offset as _),
                                            three_d.len(),
                                        ))
                                        .with_context("rerun.datatypes.Scale3D#ThreeD");
                                    }

                                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                    unsafe { three_d.get_unchecked(offset as usize) }
                                        .clone()
                                        .unwrap()
                                }),
                                2i8 => Scale3D::Uniform({
                                    if offset as usize >= uniform.len() {
                                        return Err(crate::DeserializationError::offsets_mismatch(
                                            (offset as _, offset as _),
                                            uniform.len(),
                                        ))
                                        .with_context("rerun.datatypes.Scale3D#Uniform");
                                    }

                                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                    unsafe { uniform.get_unchecked(offset as usize) }
                                        .clone()
                                        .unwrap()
                                }),
                                _ => unreachable!(),
                            }))
                        }
                    })
                    .collect::<crate::DeserializationResult<Vec<_>>>()
                    .with_context("rerun.datatypes.Scale3D")?
            }
        })
    }

    #[inline]
    fn try_iter_from_arrow(
        data: &dyn ::arrow2::array::Array,
    ) -> crate::DeserializationResult<Self::Iter<'_>>
    where
        Self: Sized,
    {
        Ok(Self::try_from_arrow_opt(data)?.into_iter())
    }

    #[inline]
    fn convert_item_to_self(item: Self::Item<'_>) -> Option<Self> {
        item
    }
}

impl crate::Datatype for Scale3D {}
