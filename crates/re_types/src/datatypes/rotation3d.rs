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

#[doc = "A 3D rotation."]
#[derive(Clone, Debug, Copy, PartialEq)]
pub enum Rotation3D {
    #[doc = "Rotation defined by a quaternion."]
    Quaternion(crate::datatypes::Quaternion),

    #[doc = "Rotation defined with an axis and an angle."]
    AxisAngle(crate::datatypes::RotationAxisAngle),
}

impl<'a> From<Rotation3D> for ::std::borrow::Cow<'a, Rotation3D> {
    #[inline]
    fn from(value: Rotation3D) -> Self {
        std::borrow::Cow::Owned(value)
    }
}

impl<'a> From<&'a Rotation3D> for ::std::borrow::Cow<'a, Rotation3D> {
    #[inline]
    fn from(value: &'a Rotation3D) -> Self {
        std::borrow::Cow::Borrowed(value)
    }
}

impl crate::Datatype for Rotation3D {
    #[inline]
    fn name() -> crate::DatatypeName {
        crate::DatatypeName::Borrowed("rerun.datatypes.Rotation3D")
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    #[inline]
    fn to_arrow_datatype() -> arrow2::datatypes::DataType {
        use ::arrow2::datatypes::*;
        DataType::Union(
            vec![
                Field {
                    name: "Quaternion".to_owned(),
                    data_type: DataType::FixedSizeList(
                        Box::new(Field {
                            name: "item".to_owned(),
                            data_type: DataType::Float32,
                            is_nullable: false,
                            metadata: [].into(),
                        }),
                        4usize,
                    ),
                    is_nullable: false,
                    metadata: [].into(),
                },
                Field {
                    name: "AxisAngle".to_owned(),
                    data_type: DataType::Struct(vec![
                        Field {
                            name: "axis".to_owned(),
                            data_type: DataType::FixedSizeList(
                                Box::new(Field {
                                    name: "item".to_owned(),
                                    data_type: DataType::Float32,
                                    is_nullable: false,
                                    metadata: [].into(),
                                }),
                                3usize,
                            ),
                            is_nullable: false,
                            metadata: [].into(),
                        },
                        Field {
                            name: "angle".to_owned(),
                            data_type: DataType::Union(
                                vec![
                                    Field {
                                        name: "Radians".to_owned(),
                                        data_type: DataType::Float32,
                                        is_nullable: false,
                                        metadata: [].into(),
                                    },
                                    Field {
                                        name: "Degrees".to_owned(),
                                        data_type: DataType::Float32,
                                        is_nullable: false,
                                        metadata: [].into(),
                                    },
                                ],
                                None,
                                UnionMode::Dense,
                            ),
                            is_nullable: false,
                            metadata: [].into(),
                        },
                    ]),
                    is_nullable: false,
                    metadata: [].into(),
                },
            ],
            None,
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
        use crate::{Component as _, Datatype as _};
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
                        Box::new(<crate::datatypes::Rotation3D>::to_arrow_datatype()),
                        None,
                    )
                } else {
                    <crate::datatypes::Rotation3D>::to_arrow_datatype()
                })
                .to_logical_type()
                .clone(),
                {
                    data.iter()
                        .flatten()
                        .map(|v| match **v {
                            Rotation3D::Quaternion(_) => 0i8,
                            Rotation3D::AxisAngle(_) => 1i8,
                        })
                        .collect()
                },
                vec![
                    {
                        let (somes, quaternion): (Vec<_>, Vec<_>) = data
                            .iter()
                            .flatten()
                            .filter(|datum| matches!(***datum, Rotation3D::Quaternion(_)))
                            .map(|datum| {
                                let datum = match &**datum {
                                    Rotation3D::Quaternion(v) => Some(v.clone()),
                                    _ => None,
                                };
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let quaternion_bitmap: Option<::arrow2::bitmap::Bitmap> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        {
                            use arrow2::{buffer::Buffer, offset::OffsetsBuffer};
                            let quaternion_inner_data: Vec<_> = quaternion
                                .iter()
                                .flatten()
                                .map(|datum| {
                                    let crate::datatypes::Quaternion(data0) = datum;
                                    data0
                                })
                                .flatten()
                                .map(ToOwned::to_owned)
                                .map(Some)
                                .collect();
                            let quaternion_inner_bitmap: Option<::arrow2::bitmap::Bitmap> = None;
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
                                        4usize,
                                    )
                                    .to_logical_type()
                                    .clone()
                                },
                                PrimitiveArray::new(
                                    {
                                        _ = extension_wrapper;
                                        DataType::Float32.to_logical_type().clone()
                                    },
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
                            .flatten()
                            .filter(|datum| matches!(***datum, Rotation3D::AxisAngle(_)))
                            .map(|datum| {
                                let datum = match &**datum {
                                    Rotation3D::AxisAngle(v) => Some(v.clone()),
                                    _ => None,
                                };
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let axis_angle_bitmap: Option<::arrow2::bitmap::Bitmap> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        {
                            _ = axis_angle_bitmap;
                            _ = extension_wrapper;
                            crate::datatypes::RotationAxisAngle::try_to_arrow_opt(
                                axis_angle,
                                None::<&str>,
                            )?
                        }
                    },
                ],
                Some({
                    let mut quaternion_offset = 0;
                    let mut axis_angle_offset = 0;
                    data.iter()
                        .flatten()
                        .map(|v| match **v {
                            Rotation3D::Quaternion(_) => {
                                let offset = quaternion_offset;
                                quaternion_offset += 1;
                                offset
                            }

                            Rotation3D::AxisAngle(_) => {
                                let offset = axis_angle_offset;
                                axis_angle_offset += 1;
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
        use crate::{Component as _, Datatype as _};
        use ::arrow2::{array::*, datatypes::*};
        Ok({
            let data = data
                .as_any()
                .downcast_ref::<::arrow2::array::UnionArray>()
                .ok_or_else(|| crate::DeserializationError::SchemaMismatch {
                    expected: data.data_type().clone(),
                    got: data.data_type().clone(),
                })?;
            let (data_types, data_arrays, data_offsets) =
                (data.types(), data.fields(), data.offsets().unwrap());
            let quaternion = {
                let data = &*data_arrays[0usize];

                {
                    let datatype = data.data_type();
                    let data = data
                        .as_any()
                        .downcast_ref::<::arrow2::array::FixedSizeListArray>()
                        .unwrap();
                    let bitmap = data.validity().cloned();
                    let offsets = (0..).step_by(4usize).zip((4usize..).step_by(4usize));
                    let data = &**data.values();
                    let data = data
                        .as_any()
                        .downcast_ref::<Float32Array>()
                        .unwrap()
                        .into_iter()
                        .map(|v| v.copied())
                        .map(|v| {
                            v.ok_or_else(|| crate::DeserializationError::MissingData {
                                datatype: DataType::Float32,
                            })
                        })
                        .collect::<crate::DeserializationResult<Vec<_>>>()?;
                    offsets
                        .enumerate()
                        .map(move |(i, (start, end))| {
                            bitmap
                                .as_ref()
                                .map_or(true, |bitmap| bitmap.get_bit(i))
                                .then(|| {
                                    data.get(start as usize..end as usize)
                                        .ok_or_else(|| {
                                            crate::DeserializationError::OffsetsMismatch {
                                                bounds: (start as usize, end as usize),
                                                len: data.len(),
                                                datatype: datatype.clone(),
                                            }
                                        })?
                                        .to_vec()
                                        .try_into()
                                        .map_err(|_err| {
                                            crate::DeserializationError::ArrayLengthMismatch {
                                                expected: 4usize,
                                                got: (end - start) as usize,
                                                datatype: datatype.clone(),
                                            }
                                        })
                                })
                                .transpose()
                        })
                        .map(|res| res.map(|opt| opt.map(|v| crate::datatypes::Quaternion(v))))
                        .collect::<crate::DeserializationResult<Vec<Option<_>>>>()?
                        .into_iter()
                }
                .collect::<Vec<_>>()
            };
            let axis_angle = {
                let data = &*data_arrays[1usize];

                crate::datatypes::RotationAxisAngle::try_from_arrow_opt(data)?
                    .into_iter()
                    .collect::<Vec<_>>()
            };
            data_types
                .iter()
                .enumerate()
                .map(|(i, typ)| {
                    let offset = data_offsets[i];

                    Ok(Some(match typ {
                        0i8 => Rotation3D::Quaternion(
                            quaternion
                                .get(offset as usize)
                                .ok_or_else(|| crate::DeserializationError::OffsetsMismatch {
                                    bounds: (offset as usize, offset as usize),
                                    len: quaternion.len(),
                                    datatype: data.data_type().clone(),
                                })?
                                .clone()
                                .unwrap(),
                        ),
                        1i8 => Rotation3D::AxisAngle(
                            axis_angle
                                .get(offset as usize)
                                .ok_or_else(|| crate::DeserializationError::OffsetsMismatch {
                                    bounds: (offset as usize, offset as usize),
                                    len: axis_angle.len(),
                                    datatype: data.data_type().clone(),
                                })?
                                .clone()
                                .unwrap(),
                        ),
                        _ => unreachable!(),
                    }))
                })
                .collect::<crate::DeserializationResult<Vec<_>>>()?
        })
    }
}
