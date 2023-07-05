// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

#![allow(trivial_numeric_casts)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::map_flatten)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_cast)]

#[doc = "A vector in 2D space."]
#[derive(Debug, Clone, Default, Copy, PartialEq, PartialOrd)]
pub struct Vec2D(pub [f32; 2usize]);

impl<'a> From<Vec2D> for ::std::borrow::Cow<'a, Vec2D> {
    #[inline]
    fn from(value: Vec2D) -> Self {
        std::borrow::Cow::Owned(value)
    }
}

impl<'a> From<&'a Vec2D> for ::std::borrow::Cow<'a, Vec2D> {
    #[inline]
    fn from(value: &'a Vec2D) -> Self {
        std::borrow::Cow::Borrowed(value)
    }
}

impl crate::Datatype for Vec2D {
    #[inline]
    fn name() -> crate::DatatypeName {
        crate::DatatypeName::Borrowed("rerun.datatypes.Vec2D")
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    #[inline]
    fn to_arrow_datatype() -> arrow2::datatypes::DataType {
        use ::arrow2::datatypes::*;
        DataType::FixedSizeList(
            Box::new(Field {
                name: "item".to_owned(),
                data_type: DataType::Float32,
                is_nullable: false,
                metadata: [].into(),
            }),
            2usize,
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
            let (somes, data0): (Vec<_>, Vec<_>) = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    let datum = datum.map(|datum| {
                        let Self(data0) = datum.into_owned();
                        data0
                    });
                    (datum.is_some(), datum)
                })
                .unzip();
            let data0_bitmap: Option<::arrow2::bitmap::Bitmap> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            {
                use arrow2::{buffer::Buffer, offset::OffsetsBuffer};
                let data0_inner_data: Vec<_> = data0
                    .iter()
                    .flatten()
                    .flatten()
                    .map(ToOwned::to_owned)
                    .map(Some)
                    .collect();
                let data0_inner_bitmap: Option<::arrow2::bitmap::Bitmap> = {
                    let any_nones = data0_inner_data.iter().any(|v| v.is_none());
                    any_nones.then(|| data0_inner_data.iter().map(|v| v.is_some()).collect())
                };
                FixedSizeListArray::new(
                    {
                        _ = extension_wrapper;
                        DataType::Extension(
                            "rerun.datatypes.Vec2D".to_owned(),
                            Box::new(DataType::FixedSizeList(
                                Box::new(Field {
                                    name: "item".to_owned(),
                                    data_type: DataType::Float32,
                                    is_nullable: false,
                                    metadata: [].into(),
                                }),
                                2usize,
                            )),
                            None,
                        )
                        .to_logical_type()
                        .clone()
                    },
                    PrimitiveArray::new(
                        {
                            _ = extension_wrapper;
                            DataType::Extension(
                                "rerun.datatypes.Vec2D".to_owned(),
                                Box::new(DataType::Float32),
                                None,
                            )
                            .to_logical_type()
                            .clone()
                        },
                        data0_inner_data
                            .into_iter()
                            .map(|v| v.unwrap_or_default())
                            .collect(),
                        data0_inner_bitmap,
                    )
                    .boxed(),
                    data0_bitmap,
                )
                .boxed()
            }
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
            let datatype = data.data_type();
            let data = data
                .as_any()
                .downcast_ref::<::arrow2::array::ListArray<i32>>()
                .unwrap();
            let bitmap = data.validity().cloned();
            let offsets = (0..).step_by(2usize).zip((2usize..).step_by(2usize));
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
                                .ok_or_else(|| crate::DeserializationError::OffsetsMismatch {
                                    bounds: (start as usize, end as usize),
                                    len: data.len(),
                                    datatype: datatype.clone(),
                                })?
                                .to_vec()
                                .try_into()
                                .map_err(|_err| crate::DeserializationError::ArrayLengthMismatch {
                                    expected: 2usize,
                                    got: (end - start) as usize,
                                    datatype: datatype.clone(),
                                })
                        })
                        .transpose()
                })
                .collect::<crate::DeserializationResult<Vec<Option<_>>>>()?
                .into_iter()
        }
        .map(|v| {
            v.ok_or_else(|| crate::DeserializationError::MissingData {
                datatype: data.data_type().clone(),
            })
        })
        .map(|res| res.map(|v| Some(Self(v))))
        .collect::<crate::DeserializationResult<Vec<Option<_>>>>()?)
    }
}
