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
pub struct Point2D {
    pub x: f32,
    pub y: f32,
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
    type Name = crate::DatatypeName;
    #[inline]
    fn name() -> Self::Name {
        "rerun.datatypes.Point2D".into()
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    #[inline]
    fn to_arrow_datatype() -> arrow2::datatypes::DataType {
        use ::arrow2::datatypes::*;
        DataType::Struct(vec![
            Field {
                name: "x".to_owned(),
                data_type: DataType::Float32,
                is_nullable: false,
                metadata: [].into(),
            },
            Field {
                name: "y".to_owned(),
                data_type: DataType::Float32,
                is_nullable: false,
                metadata: [].into(),
            },
        ])
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    fn try_to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
        extension_wrapper: Option<&str>,
    ) -> crate::SerializationResult<Box<dyn ::arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        use crate::Loggable as _;
        use ::arrow2::{array::*, datatypes::*};
        Ok({
            let (somes, data): (Vec<_>, Vec<_>) = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    (datum.is_some(), datum)
                })
                .unzip();
            let bitmap: Option<::arrow2::bitmap::Bitmap> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            StructArray::new(
                (if let Some(ext) = extension_wrapper {
                    DataType::Extension(
                        ext.to_owned(),
                        Box::new(<crate::datatypes::Point2D>::to_arrow_datatype()),
                        None,
                    )
                } else {
                    <crate::datatypes::Point2D>::to_arrow_datatype()
                })
                .to_logical_type()
                .clone(),
                vec![
                    {
                        let (somes, x): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum.as_ref().map(|datum| {
                                    let Self { x, .. } = &**datum;
                                    x.clone()
                                });
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let x_bitmap: Option<::arrow2::bitmap::Bitmap> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        PrimitiveArray::new(
                            {
                                _ = extension_wrapper;
                                DataType::Float32.to_logical_type().clone()
                            },
                            x.into_iter().map(|v| v.unwrap_or_default()).collect(),
                            x_bitmap,
                        )
                        .boxed()
                    },
                    {
                        let (somes, y): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum.as_ref().map(|datum| {
                                    let Self { y, .. } = &**datum;
                                    y.clone()
                                });
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let y_bitmap: Option<::arrow2::bitmap::Bitmap> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        PrimitiveArray::new(
                            {
                                _ = extension_wrapper;
                                DataType::Float32.to_logical_type().clone()
                            },
                            y.into_iter().map(|v| v.unwrap_or_default()).collect(),
                            y_bitmap,
                        )
                        .boxed()
                    },
                ],
                bitmap,
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
        use crate::Loggable as _;
        use ::arrow2::{array::*, datatypes::*};
        Ok({
            let data = data
                .as_any()
                .downcast_ref::<::arrow2::array::StructArray>()
                .ok_or_else(|| crate::DeserializationError::DatatypeMismatch {
                    expected: data.data_type().clone(),
                    got: data.data_type().clone(),
                    backtrace: ::backtrace::Backtrace::new_unresolved(),
                })
                .map_err(|err| crate::DeserializationError::Context {
                    location: "rerun.datatypes.Point2D".into(),
                    source: Box::new(err),
                })?;
            if data.is_empty() {
                Vec::new()
            } else {
                let (data_fields, data_arrays, data_bitmap) =
                    (data.fields(), data.values(), data.validity());
                let is_valid = |i| data_bitmap.map_or(true, |bitmap| bitmap.get_bit(i));
                let arrays_by_name: ::std::collections::HashMap<_, _> = data_fields
                    .iter()
                    .map(|field| field.name.as_str())
                    .zip(data_arrays)
                    .collect();
                let x = {
                    let data = &**arrays_by_name["x"];

                    data.as_any()
                        .downcast_ref::<Float32Array>()
                        .unwrap()
                        .into_iter()
                        .map(|v| v.copied())
                };
                let y = {
                    let data = &**arrays_by_name["y"];

                    data.as_any()
                        .downcast_ref::<Float32Array>()
                        .unwrap()
                        .into_iter()
                        .map(|v| v.copied())
                };
                ::itertools::izip!(x, y)
                    .enumerate()
                    .map(|(i, (x, y))| {
                        is_valid(i)
                            .then(|| {
                                Ok(Self {
                                    x: x.ok_or_else(|| crate::DeserializationError::MissingData {
                                        backtrace: ::backtrace::Backtrace::new_unresolved(),
                                    })
                                    .map_err(|err| {
                                        crate::DeserializationError::Context {
                                            location: "rerun.datatypes.Point2D#x".into(),
                                            source: Box::new(err),
                                        }
                                    })?,
                                    y: y.ok_or_else(|| crate::DeserializationError::MissingData {
                                        backtrace: ::backtrace::Backtrace::new_unresolved(),
                                    })
                                    .map_err(|err| {
                                        crate::DeserializationError::Context {
                                            location: "rerun.datatypes.Point2D#y".into(),
                                            source: Box::new(err),
                                        }
                                    })?,
                                })
                            })
                            .transpose()
                    })
                    .collect::<crate::DeserializationResult<Vec<_>>>()
                    .map_err(|err| crate::DeserializationError::Context {
                        location: "rerun.datatypes.Point2D".into(),
                        source: Box::new(err),
                    })?
            }
        })
    }
}

impl crate::Datatype for Point2D {}
