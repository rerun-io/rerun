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

/// A line strip in 2D space.
///
/// A line strip is a list of points connected by line segments. It can be used to draw
/// approximations of smooth curves.
///
/// The points will be connected in order, like so:
/// ```text
///        2------3     5
///       /        \   /
/// 0----1          \ /
///                  4
/// ```
#[derive(Clone, Debug, Default, PartialEq, PartialOrd)]
pub struct LineStrip2D(pub Vec<crate::datatypes::Vec2D>);

impl<I: Into<crate::datatypes::Vec2D>, T: IntoIterator<Item = I>> From<T> for LineStrip2D {
    fn from(v: T) -> Self {
        Self(v.into_iter().map(|v| v.into()).collect())
    }
}

impl<'a> From<LineStrip2D> for ::std::borrow::Cow<'a, LineStrip2D> {
    #[inline]
    fn from(value: LineStrip2D) -> Self {
        std::borrow::Cow::Owned(value)
    }
}

impl<'a> From<&'a LineStrip2D> for ::std::borrow::Cow<'a, LineStrip2D> {
    #[inline]
    fn from(value: &'a LineStrip2D) -> Self {
        std::borrow::Cow::Borrowed(value)
    }
}

impl crate::Loggable for LineStrip2D {
    type Name = crate::ComponentName;
    type Item<'a> = Option<Self>;
    type Iter<'a> = <Vec<Self::Item<'a>> as IntoIterator>::IntoIter;

    #[inline]
    fn name() -> Self::Name {
        "rerun.linestrip2d".into()
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    #[inline]
    fn to_arrow_datatype() -> arrow2::datatypes::DataType {
        use ::arrow2::datatypes::*;
        DataType::List(Box::new(Field {
            name: "item".to_owned(),
            data_type: <crate::datatypes::Vec2D>::to_arrow_datatype(),
            is_nullable: false,
            metadata: [].into(),
        }))
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
                    .flatten()
                    .flatten()
                    .cloned()
                    .map(Some)
                    .collect();
                let data0_inner_bitmap: Option<::arrow2::bitmap::Bitmap> = None;
                let offsets = ::arrow2::offset::Offsets::<i32>::try_from_lengths(
                    data0
                        .iter()
                        .map(|opt| opt.as_ref().map(|datum| datum.len()).unwrap_or_default()),
                )
                .unwrap()
                .into();
                ListArray::new(
                    {
                        _ = extension_wrapper;
                        DataType::Extension(
                            "rerun.components.LineStrip2D".to_owned(),
                            Box::new(DataType::List(Box::new(Field {
                                name: "item".to_owned(),
                                data_type: <crate::datatypes::Vec2D>::to_arrow_datatype(),
                                is_nullable: false,
                                metadata: [].into(),
                            }))),
                            None,
                        )
                        .to_logical_type()
                        .clone()
                    },
                    offsets,
                    {
                        use arrow2::{buffer::Buffer, offset::OffsetsBuffer};
                        let data0_inner_data_inner_data: Vec<_> = data0_inner_data
                            .iter()
                            .map(|datum| {
                                datum
                                    .map(|datum| {
                                        let crate::datatypes::Vec2D(data0) = datum;
                                        data0
                                    })
                                    .unwrap_or_default()
                            })
                            .flatten()
                            .map(Some)
                            .collect();
                        let data0_inner_data_inner_bitmap: Option<::arrow2::bitmap::Bitmap> = None;
                        FixedSizeListArray::new(
                            {
                                _ = extension_wrapper;
                                DataType::Extension(
                                    "rerun.components.LineStrip2D".to_owned(),
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
                                        "rerun.components.LineStrip2D".to_owned(),
                                        Box::new(DataType::Float32),
                                        None,
                                    )
                                    .to_logical_type()
                                    .clone()
                                },
                                data0_inner_data_inner_data
                                    .into_iter()
                                    .map(|v| v.unwrap_or_default())
                                    .collect(),
                                data0_inner_data_inner_bitmap,
                            )
                            .boxed(),
                            data0_inner_bitmap,
                        )
                        .boxed()
                    },
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
        use crate::{Loggable as _, ResultExt as _};
        use ::arrow2::{array::*, datatypes::*};
        Ok({
            let data = data
                .as_any()
                .downcast_ref::<::arrow2::array::ListArray<i32>>()
                .ok_or_else(|| {
                    crate::DeserializationError::datatype_mismatch(
                        DataType::List(Box::new(Field {
                            name: "item".to_owned(),
                            data_type: <crate::datatypes::Vec2D>::to_arrow_datatype(),
                            is_nullable: false,
                            metadata: [].into(),
                        })),
                        data.data_type().clone(),
                    )
                })
                .with_context("rerun.components.LineStrip2D#points")?;
            if data.is_empty() {
                Vec::new()
            } else {
                let data_inner = {
                    let data_inner = &**data.values();
                    {
                        let data_inner = data_inner
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
                                    data_inner.data_type().clone(),
                                )
                            })
                            .with_context("rerun.components.LineStrip2D#points")?;
                        if data_inner.is_empty() {
                            Vec::new()
                        } else {
                            let offsets = (0..)
                                .step_by(2usize)
                                .zip((2usize..).step_by(2usize).take(data_inner.len()));
                            let data_inner_inner = {
                                let data_inner_inner = &**data_inner.values();
                                data_inner_inner
                                    .as_any()
                                    .downcast_ref::<Float32Array>()
                                    .ok_or_else(|| {
                                        crate::DeserializationError::datatype_mismatch(
                                            DataType::Float32,
                                            data_inner_inner.data_type().clone(),
                                        )
                                    })
                                    .with_context("rerun.components.LineStrip2D#points")?
                                    .into_iter()
                                    .map(|opt| opt.copied())
                                    .collect::<Vec<_>>()
                            };
                            arrow2::bitmap::utils::ZipValidity::new_with_validity(
                                offsets,
                                data_inner.validity(),
                            )
                            .map(|elem| {
                                elem.map(|(start, end)| {
                                    debug_assert!(end - start == 2usize);
                                    if end as usize > data_inner_inner.len() {
                                        return Err(crate::DeserializationError::offset_slice_oob(
                                            (start, end),
                                            data_inner_inner.len(),
                                        ));
                                    }

                                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                    let data = unsafe {
                                        data_inner_inner.get_unchecked(start as usize..end as usize)
                                    };
                                    let data = data.iter().cloned().map(Option::unwrap_or_default);
                                    let arr = array_init::from_iter(data).unwrap();
                                    Ok(arr)
                                })
                                .transpose()
                            })
                            .map(|res_or_opt| {
                                res_or_opt.map(|res_or_opt| {
                                    res_or_opt.map(|v| crate::datatypes::Vec2D(v))
                                })
                            })
                            .collect::<crate::DeserializationResult<Vec<Option<_>>>>()?
                        }
                        .into_iter()
                    }
                    .collect::<Vec<_>>()
                };
                let offsets = data.offsets();
                arrow2::bitmap::utils::ZipValidity::new_with_validity(
                    offsets.iter().zip(offsets.lengths()),
                    data.validity(),
                )
                .map(|elem| {
                    elem.map(|(start, len)| {
                        let start = *start as usize;
                        let end = start + len;
                        if end as usize > data_inner.len() {
                            return Err(crate::DeserializationError::offset_slice_oob(
                                (start, end),
                                data_inner.len(),
                            ));
                        }

                        #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                        let data =
                            unsafe { data_inner.get_unchecked(start as usize..end as usize) };
                        let data = data
                            .iter()
                            .cloned()
                            .map(Option::unwrap_or_default)
                            .collect();
                        Ok(data)
                    })
                    .transpose()
                })
                .collect::<crate::DeserializationResult<Vec<Option<_>>>>()?
            }
            .into_iter()
        }
        .map(|v| v.ok_or_else(crate::DeserializationError::missing_data))
        .map(|res| res.map(|v| Some(Self(v))))
        .collect::<crate::DeserializationResult<Vec<Option<_>>>>()
        .with_context("rerun.components.LineStrip2D#points")
        .with_context("rerun.components.LineStrip2D")?)
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

impl crate::Component for LineStrip2D {}
