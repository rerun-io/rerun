// DO NOT EDIT!: This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs:165.

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
    pub label: Option<crate::datatypes::Label>,

    /// The color that will be applied to the annotated entity.
    pub color: Option<crate::datatypes::Color>,
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

    #[inline]
    fn name() -> Self::Name {
        "rerun.datatypes.AnnotationInfo".into()
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
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
                data_type: <crate::datatypes::Label>::arrow_datatype(),
                is_nullable: true,
                metadata: [].into(),
            },
            Field {
                name: "color".to_owned(),
                data_type: <crate::datatypes::Color>::arrow_datatype(),
                is_nullable: true,
                metadata: [].into(),
            },
        ])
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    fn try_to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> crate::SerializationResult<Box<dyn ::arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        use crate::{Loggable as _, ResultExt as _};
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
                <crate::datatypes::AnnotationInfo>::arrow_datatype(),
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
                            DataType::UInt16,
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
                                    let crate::datatypes::Label(data0) = datum;
                                    data0.0.clone()
                                })
                                .collect();
                            let offsets = ::arrow2::offset::Offsets::<i32>::try_from_lengths(
                                label.iter().map(|opt| {
                                    opt.as_ref()
                                        .map(|datum| {
                                            let crate::datatypes::Label(data0) = datum;
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
                                    DataType::Utf8,
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
                            DataType::UInt32,
                            color
                                .into_iter()
                                .map(|datum| {
                                    datum
                                        .map(|datum| {
                                            let crate::datatypes::Color(data0) = datum;
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
                .downcast_ref::<::arrow2::array::StructArray>()
                .ok_or_else(|| {
                    crate::DeserializationError::datatype_mismatch(
                        DataType::Struct(vec![
                            Field {
                                name: "id".to_owned(),
                                data_type: DataType::UInt16,
                                is_nullable: false,
                                metadata: [].into(),
                            },
                            Field {
                                name: "label".to_owned(),
                                data_type: <crate::datatypes::Label>::arrow_datatype(),
                                is_nullable: true,
                                metadata: [].into(),
                            },
                            Field {
                                name: "color".to_owned(),
                                data_type: <crate::datatypes::Color>::arrow_datatype(),
                                is_nullable: true,
                                metadata: [].into(),
                            },
                        ]),
                        arrow_data.data_type().clone(),
                    )
                })
                .with_context("rerun.datatypes.AnnotationInfo")?;
            if arrow_data.is_empty() {
                Vec::new()
            } else {
                let (arrow_data_fields, arrow_data_arrays) =
                    (arrow_data.fields(), arrow_data.values());
                let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data_fields
                    .iter()
                    .map(|field| field.name.as_str())
                    .zip(arrow_data_arrays)
                    .collect();
                let id = {
                    if !arrays_by_name.contains_key("id") {
                        return Err(crate::DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "id",
                        ))
                        .with_context("rerun.datatypes.AnnotationInfo");
                    }
                    let arrow_data = &**arrays_by_name["id"];
                    arrow_data
                        .as_any()
                        .downcast_ref::<UInt16Array>()
                        .ok_or_else(|| {
                            crate::DeserializationError::datatype_mismatch(
                                DataType::UInt16,
                                arrow_data.data_type().clone(),
                            )
                        })
                        .with_context("rerun.datatypes.AnnotationInfo#id")?
                        .into_iter()
                        .map(|opt| opt.copied())
                };
                let label = {
                    if !arrays_by_name.contains_key("label") {
                        return Err(crate::DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "label",
                        ))
                        .with_context("rerun.datatypes.AnnotationInfo");
                    }
                    let arrow_data = &**arrays_by_name["label"];
                    {
                        let arrow_data = arrow_data
                            .as_any()
                            .downcast_ref::<::arrow2::array::Utf8Array<i32>>()
                            .ok_or_else(|| {
                                crate::DeserializationError::datatype_mismatch(
                                    DataType::Utf8,
                                    arrow_data.data_type().clone(),
                                )
                            })
                            .with_context("rerun.datatypes.AnnotationInfo#label")?;
                        let arrow_data_buf = arrow_data.values();
                        let offsets = arrow_data.offsets();
                        arrow2::bitmap::utils::ZipValidity::new_with_validity(
                            offsets.iter().zip(offsets.lengths()),
                            arrow_data.validity(),
                        )
                        .map(|elem| {
                            elem.map(|(start, len)| {
                                let start = *start as usize;
                                let end = start + len;
                                if end as usize > arrow_data_buf.len() {
                                    return Err(crate::DeserializationError::offset_slice_oob(
                                        (start, end),
                                        arrow_data_buf.len(),
                                    ));
                                }

                                #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                let data =
                                    unsafe { arrow_data_buf.clone().sliced_unchecked(start, len) };
                                Ok(data)
                            })
                            .transpose()
                        })
                        .map(|res_or_opt| {
                            res_or_opt.map(|res_or_opt| {
                                res_or_opt.map(|v| crate::datatypes::Label(crate::ArrowString(v)))
                            })
                        })
                        .collect::<crate::DeserializationResult<Vec<Option<_>>>>()
                        .with_context("rerun.datatypes.AnnotationInfo#label")?
                        .into_iter()
                    }
                };
                let color = {
                    if !arrays_by_name.contains_key("color") {
                        return Err(crate::DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "color",
                        ))
                        .with_context("rerun.datatypes.AnnotationInfo");
                    }
                    let arrow_data = &**arrays_by_name["color"];
                    arrow_data
                        .as_any()
                        .downcast_ref::<UInt32Array>()
                        .ok_or_else(|| {
                            crate::DeserializationError::datatype_mismatch(
                                DataType::UInt32,
                                arrow_data.data_type().clone(),
                            )
                        })
                        .with_context("rerun.datatypes.AnnotationInfo#color")?
                        .into_iter()
                        .map(|opt| opt.copied())
                        .map(|res_or_opt| res_or_opt.map(|v| crate::datatypes::Color(v)))
                };
                arrow2::bitmap::utils::ZipValidity::new_with_validity(
                    ::itertools::izip!(id, label, color),
                    arrow_data.validity(),
                )
                .map(|opt| {
                    opt.map(|(id, label, color)| {
                        Ok(Self {
                            id: id
                                .ok_or_else(crate::DeserializationError::missing_data)
                                .with_context("rerun.datatypes.AnnotationInfo#id")?,
                            label,
                            color,
                        })
                    })
                    .transpose()
                })
                .collect::<crate::DeserializationResult<Vec<_>>>()
                .with_context("rerun.datatypes.AnnotationInfo")?
            }
        })
    }
}
