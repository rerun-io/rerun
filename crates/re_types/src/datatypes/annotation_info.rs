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

/// Annotation info annotating a class id or key-point id.
///
/// Color and label will be used to annotate entities/keypoints which reference the id.
/// The id refers either to a class or key-point id
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AnnotationInfo {
    /// `ClassId` or `KeypointId` to which this annotation info belongs.
    pub id: u16,

    /// The label that will be shown in the UI.
    pub label: Option<crate::components::Label>,

    /// The color that will be applied to the annotated entity.
    pub color: Option<crate::components::Color>,
}

impl<'a> From<AnnotationInfo> for ::std::borrow::Cow<'a, AnnotationInfo> {
    #[inline]
    fn from(value: AnnotationInfo) -> Self {
        std::borrow::Cow::Owned(value)
    }
}

impl<'a> From<&'a AnnotationInfo> for ::std::borrow::Cow<'a, AnnotationInfo> {
    #[inline]
    fn from(value: &'a AnnotationInfo) -> Self {
        std::borrow::Cow::Borrowed(value)
    }
}

impl crate::Loggable for AnnotationInfo {
    type Name = crate::DatatypeName;
    type Item<'a> = Option<Self>;
    type Iter<'a> = <Vec<Self::Item<'a>> as IntoIterator>::IntoIter;

    #[inline]
    fn name() -> Self::Name {
        "rerun.datatypes.AnnotationInfo".into()
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    #[inline]
    fn to_arrow_datatype() -> arrow2::datatypes::DataType {
        use ::arrow2::datatypes::*;
        DataType::Struct(vec![
            Field {
                name: "id".to_owned(),
                data_type: DataType::UInt16,
                is_nullable: false,
                metadata: [].into(),
            },
            Field {
                name: "label".to_owned(),
                data_type: <crate::components::Label>::to_arrow_datatype(),
                is_nullable: true,
                metadata: [].into(),
            },
            Field {
                name: "color".to_owned(),
                data_type: <crate::components::Color>::to_arrow_datatype(),
                is_nullable: true,
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
                        Box::new(<crate::datatypes::AnnotationInfo>::to_arrow_datatype()),
                        None,
                    )
                } else {
                    <crate::datatypes::AnnotationInfo>::to_arrow_datatype()
                })
                .to_logical_type()
                .clone(),
                vec![
                    {
                        let (somes, id): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum.as_ref().map(|datum| {
                                    let Self { id, .. } = &**datum;
                                    id.clone()
                                });
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let id_bitmap: Option<::arrow2::bitmap::Bitmap> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        PrimitiveArray::new(
                            {
                                _ = extension_wrapper;
                                DataType::UInt16.to_logical_type().clone()
                            },
                            id.into_iter().map(|v| v.unwrap_or_default()).collect(),
                            id_bitmap,
                        )
                        .boxed()
                    },
                    {
                        let (somes, label): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum
                                    .as_ref()
                                    .map(|datum| {
                                        let Self { label, .. } = &**datum;
                                        label.clone()
                                    })
                                    .flatten();
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let label_bitmap: Option<::arrow2::bitmap::Bitmap> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        {
                            let inner_data: ::arrow2::buffer::Buffer<u8> = label
                                .iter()
                                .flatten()
                                .flat_map(|datum| {
                                    let crate::components::Label(data0) = datum;
                                    data0.0.clone()
                                })
                                .collect();
                            let offsets = ::arrow2::offset::Offsets::<i32>::try_from_lengths(
                                label.iter().map(|opt| {
                                    opt.as_ref()
                                        .map(|datum| {
                                            let crate::components::Label(data0) = datum;
                                            data0.0.len()
                                        })
                                        .unwrap_or_default()
                                }),
                            )
                            .unwrap()
                            .into();

                            #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                            unsafe {
                                Utf8Array::<i32>::new_unchecked(
                                    {
                                        _ = extension_wrapper;
                                        DataType::Utf8.to_logical_type().clone()
                                    },
                                    offsets,
                                    inner_data,
                                    label_bitmap,
                                )
                            }
                            .boxed()
                        }
                    },
                    {
                        let (somes, color): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum
                                    .as_ref()
                                    .map(|datum| {
                                        let Self { color, .. } = &**datum;
                                        color.clone()
                                    })
                                    .flatten();
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let color_bitmap: Option<::arrow2::bitmap::Bitmap> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        PrimitiveArray::new(
                            {
                                _ = extension_wrapper;
                                DataType::UInt32.to_logical_type().clone()
                            },
                            color
                                .into_iter()
                                .map(|datum| {
                                    datum
                                        .map(|datum| {
                                            let crate::components::Color(data0) = datum;
                                            data0
                                        })
                                        .unwrap_or_default()
                                })
                                .collect(),
                            color_bitmap,
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
                    location: "rerun.datatypes.AnnotationInfo".into(),
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
                let id = {
                    let data = &**arrays_by_name["id"];
                    data.as_any()
                        .downcast_ref::<UInt16Array>()
                        .unwrap()
                        .into_iter()
                        .map(|v| v.copied())
                };
                let label = {
                    let data = &**arrays_by_name["label"];
                    {
                        let downcast = data.as_any().downcast_ref::<Utf8Array<i32>>().unwrap();
                        let offsets = downcast.offsets();
                        arrow2::bitmap::utils::ZipValidity::new_with_validity(
                            offsets.iter().zip(offsets.lengths()),
                            downcast.validity(),
                        )
                        .map(|elem| elem.map(|(o, l)| downcast.values().clone().sliced(*o as _, l)))
                        .map(|opt| opt.map(|v| crate::components::Label(crate::ArrowString(v))))
                    }
                };
                let color = {
                    let data = &**arrays_by_name["color"];
                    data.as_any()
                        .downcast_ref::<UInt32Array>()
                        .unwrap()
                        .into_iter()
                        .map(|opt| opt.map(|v| crate::components::Color(*v)))
                };
                ::itertools::izip!(id, label, color)
                    .enumerate()
                    .map(|(i, (id, label, color))| {
                        is_valid(i)
                            .then(|| {
                                Ok(Self {
                                    id: id
                                        .ok_or_else(|| crate::DeserializationError::MissingData {
                                            backtrace: ::backtrace::Backtrace::new_unresolved(),
                                        })
                                        .map_err(|err| crate::DeserializationError::Context {
                                            location: "rerun.datatypes.AnnotationInfo#id".into(),
                                            source: Box::new(err),
                                        })?,
                                    label,
                                    color,
                                })
                            })
                            .transpose()
                    })
                    .collect::<crate::DeserializationResult<Vec<_>>>()
                    .map_err(|err| crate::DeserializationError::Context {
                        location: "rerun.datatypes.AnnotationInfo".into(),
                        source: Box::new(err),
                    })?
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

impl crate::Datatype for AnnotationInfo {}
