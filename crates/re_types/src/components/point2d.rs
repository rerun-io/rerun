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

/// A point in 2D space.
#[derive(Clone, Debug, Default, Copy, PartialEq, PartialOrd)]
pub struct Point2D(pub crate::datatypes::Vec2D);

impl<T: Into<crate::datatypes::Vec2D>> From<T> for Point2D {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl<'a> From<Point2D> for ::std::borrow::Cow<'a, Point2D> {
    #[inline]
    fn from(value: Point2D) -> Self {
        std::borrow::Cow::Owned(value)
    }
}

impl<'a> From<&'a Point2D> for ::std::borrow::Cow<'a, Point2D> {
    #[inline]
    fn from(value: &'a Point2D) -> Self {
        std::borrow::Cow::Borrowed(value)
    }
}

impl crate::Loggable for Point2D {
    type Name = crate::ComponentName;
    type Item<'a> = Self;
    type Iter<'a> = <Vec<Self::Item<'a>> as IntoIterator>::IntoIter;

    #[inline]
    fn name() -> Self::Name {
        "rerun.point2d".into()
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
        use crate::{Loggable as _, ResultExt as _};
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
                    .map(|datum| {
                        datum
                            .map(|datum| {
                                let crate::datatypes::Vec2D(data0) = datum.clone();
                                data0
                            })
                            .unwrap_or_default()
                    })
                    .flatten()
                    .map(Some)
                    .collect();
                let data0_inner_bitmap: Option<::arrow2::bitmap::Bitmap> =
                    data0_bitmap.as_ref().map(|bitmap| {
                        bitmap
                            .iter()
                            .map(|i| std::iter::repeat(i).take(2usize))
                            .flatten()
                            .collect::<Vec<_>>()
                            .into()
                    });
                FixedSizeListArray::new(
                    {
                        _ = extension_wrapper;
                        DataType::Extension(
                            "rerun.components.Point2D".to_owned(),
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
                                "rerun.components.Point2D".to_owned(),
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
                            2usize,
                        ),
                        arrow_data.data_type().clone(),
                    )
                })
                .with_context("rerun.components.Point2D#xy")?;
            if arrow_data.is_empty() {
                Vec::new()
            } else {
                let offsets = (0..)
                    .step_by(2usize)
                    .zip((2usize..).step_by(2usize).take(arrow_data.len()));
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
                        .with_context("rerun.components.Point2D#xy")?
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
                        debug_assert!(end - start == 2usize);
                        if end as usize > arrow_data_inner.len() {
                            return Err(crate::DeserializationError::offset_slice_oob(
                                (start, end),
                                arrow_data_inner.len(),
                            ));
                        }

                        #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                        let data =
                            unsafe { arrow_data_inner.get_unchecked(start as usize..end as usize) };
                        let data = data.iter().cloned().map(Option::unwrap_or_default);
                        let arr = array_init::from_iter(data).unwrap();
                        Ok(arr)
                    })
                    .transpose()
                })
                .map(|res_or_opt| {
                    res_or_opt.map(|res_or_opt| res_or_opt.map(|v| crate::datatypes::Vec2D(v)))
                })
                .collect::<crate::DeserializationResult<Vec<Option<_>>>>()?
            }
            .into_iter()
        }
        .map(|v| v.ok_or_else(crate::DeserializationError::missing_data))
        .map(|res| res.map(|v| Some(Self(v))))
        .collect::<crate::DeserializationResult<Vec<Option<_>>>>()
        .with_context("rerun.components.Point2D#xy")
        .with_context("rerun.components.Point2D")?)
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    #[inline]
    fn try_from_arrow(
        arrow_data: &dyn ::arrow2::array::Array,
    ) -> crate::DeserializationResult<Vec<Self>>
    where
        Self: Sized,
    {
        use crate::{Loggable as _, ResultExt as _};
        use ::arrow2::{array::*, buffer::*, datatypes::*};
        if let Some(validity) = arrow_data.validity() {
            if validity.unset_bits() != 0 {
                return Err(crate::DeserializationError::missing_data());
            }
        }
        Ok({
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
                            2usize,
                        ),
                        arrow_data.data_type().clone(),
                    )
                })
                .with_context("rerun.components.Point2D#xy")?;
            let arrow_data_inner = &**arrow_data.values();
            bytemuck::cast_slice::<_, [_; 2usize]>(
                arrow_data_inner
                    .as_any()
                    .downcast_ref::<Float32Array>()
                    .ok_or_else(|| {
                        crate::DeserializationError::datatype_mismatch(
                            DataType::Float32,
                            arrow_data_inner.data_type().clone(),
                        )
                    })
                    .with_context("rerun.components.Point2D#xy")?
                    .values()
                    .as_slice(),
            )
            .iter()
            .copied()
            .map(|v| crate::datatypes::Vec2D(v))
        }
        .map(|v| Self(v))
        .collect::<Vec<_>>())
    }

    #[inline]
    fn try_iter_from_arrow(
        data: &dyn ::arrow2::array::Array,
    ) -> crate::DeserializationResult<Self::Iter<'_>>
    where
        Self: Sized,
    {
        Ok(Self::try_from_arrow(data)?.into_iter())
    }

    #[inline]
    fn convert_item_to_self(item: Self::Item<'_>) -> Self {
        item
    }

    #[inline]
    fn convert_item_to_opt_self(item: Self::Item<'_>) -> Option<Self> {
        Some(item)
    }
}

impl crate::Component for Point2D {}
