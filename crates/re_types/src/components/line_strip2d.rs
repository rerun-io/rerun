// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/components/line_strip2d.fbs".

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
#[derive(Clone, Debug, Default, PartialEq)]
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

    #[inline]
    fn name() -> Self::Name {
        "rerun.components.LineStrip2D".into()
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use ::arrow2::datatypes::*;
        DataType::List(Box::new(Field {
            name: "item".to_owned(),
            data_type: <crate::datatypes::Vec2D>::arrow_datatype(),
            is_nullable: false,
            metadata: [].into(),
        }))
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
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
                    Self::arrow_datatype(),
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
                        let data0_inner_data_inner_bitmap: Option<::arrow2::bitmap::Bitmap> =
                            data0_inner_bitmap.as_ref().map(|bitmap| {
                                bitmap
                                    .iter()
                                    .map(|i| std::iter::repeat(i).take(2usize))
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
                                2usize,
                            ),
                            PrimitiveArray::new(
                                DataType::Float32,
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
                .downcast_ref::<::arrow2::array::ListArray<i32>>()
                .ok_or_else(|| {
                    crate::DeserializationError::datatype_mismatch(
                        DataType::List(Box::new(Field {
                            name: "item".to_owned(),
                            data_type: <crate::datatypes::Vec2D>::arrow_datatype(),
                            is_nullable: false,
                            metadata: [].into(),
                        })),
                        arrow_data.data_type().clone(),
                    )
                })
                .with_context("rerun.components.LineStrip2D#points")?;
            if arrow_data.is_empty() {
                Vec::new()
            } else {
                let arrow_data_inner = {
                    let arrow_data_inner = &**arrow_data.values();
                    {
                        let arrow_data_inner = arrow_data_inner
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
                                    arrow_data_inner.data_type().clone(),
                                )
                            })
                            .with_context("rerun.components.LineStrip2D#points")?;
                        if arrow_data_inner.is_empty() {
                            Vec::new()
                        } else {
                            let offsets = (0..)
                                .step_by(2usize)
                                .zip((2usize..).step_by(2usize).take(arrow_data_inner.len()));
                            let arrow_data_inner_inner = {
                                let arrow_data_inner_inner = &**arrow_data_inner.values();
                                arrow_data_inner_inner
                                    .as_any()
                                    .downcast_ref::<Float32Array>()
                                    .ok_or_else(|| {
                                        crate::DeserializationError::datatype_mismatch(
                                            DataType::Float32,
                                            arrow_data_inner_inner.data_type().clone(),
                                        )
                                    })
                                    .with_context("rerun.components.LineStrip2D#points")?
                                    .into_iter()
                                    .map(|opt| opt.copied())
                                    .collect::<Vec<_>>()
                            };
                            arrow2::bitmap::utils::ZipValidity::new_with_validity(
                                offsets,
                                arrow_data_inner.validity(),
                            )
                            .map(|elem| {
                                elem.map(|(start, end)| {
                                    debug_assert!(end - start == 2usize);
                                    if end as usize > arrow_data_inner_inner.len() {
                                        return Err(crate::DeserializationError::offset_slice_oob(
                                            (start, end),
                                            arrow_data_inner_inner.len(),
                                        ));
                                    }

                                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                    let data = unsafe {
                                        arrow_data_inner_inner
                                            .get_unchecked(start as usize..end as usize)
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
                let offsets = arrow_data.offsets();
                arrow2::bitmap::utils::ZipValidity::new_with_validity(
                    offsets.iter().zip(offsets.lengths()),
                    arrow_data.validity(),
                )
                .map(|elem| {
                    elem.map(|(start, len)| {
                        let start = *start as usize;
                        let end = start + len;
                        if end as usize > arrow_data_inner.len() {
                            return Err(crate::DeserializationError::offset_slice_oob(
                                (start, end),
                                arrow_data_inner.len(),
                            ));
                        }

                        #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                        let data =
                            unsafe { arrow_data_inner.get_unchecked(start as usize..end as usize) };
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
}
