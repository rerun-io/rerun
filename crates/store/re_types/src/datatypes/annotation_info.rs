// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/datatypes/annotation_info.fbs".

#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]
#![allow(clippy::map_flatten)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]

use ::re_types_core::try_serialize_field;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, ComponentBatchCowWithDescriptor, SerializedComponentBatch};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Datatype**: Annotation info annotating a class id or key-point id.
///
/// Color and label will be used to annotate entities/keypoints which reference the id.
/// The id refers either to a class or key-point id
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AnnotationInfo {
    /// [`datatypes::ClassId`][crate::datatypes::ClassId] or [`datatypes::KeypointId`][crate::datatypes::KeypointId] to which this annotation info belongs.
    pub id: u16,

    /// The label that will be shown in the UI.
    pub label: Option<crate::datatypes::Utf8>,

    /// The color that will be applied to the annotated entity.
    pub color: Option<crate::datatypes::Rgba32>,
}

::re_types_core::macros::impl_into_cow!(AnnotationInfo);

impl ::re_types_core::Loggable for AnnotationInfo {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        #![allow(clippy::wildcard_imports)]
        use arrow::datatypes::*;
        DataType::Struct(Fields::from(vec![
            Field::new("id", DataType::UInt16, false),
            Field::new("label", <crate::datatypes::Utf8>::arrow_datatype(), true),
            Field::new("color", <crate::datatypes::Rgba32>::arrow_datatype(), true),
        ]))
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<arrow::array::ArrayRef>
    where
        Self: Clone + 'a,
    {
        #![allow(clippy::wildcard_imports)]
        #![allow(clippy::manual_is_variant_and)]
        use ::re_types_core::{arrow_helpers::as_array_ref, Loggable as _, ResultExt as _};
        use arrow::{array::*, buffer::*, datatypes::*};
        Ok({
            let fields = Fields::from(vec![
                Field::new("id", DataType::UInt16, false),
                Field::new("label", <crate::datatypes::Utf8>::arrow_datatype(), true),
                Field::new("color", <crate::datatypes::Rgba32>::arrow_datatype(), true),
            ]);
            let (somes, data): (Vec<_>, Vec<_>) = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    (datum.is_some(), datum)
                })
                .unzip();
            let validity: Option<arrow::buffer::NullBuffer> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            as_array_ref(StructArray::new(
                fields,
                vec![
                    {
                        let (somes, id): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum.as_ref().map(|datum| datum.id.clone());
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let id_validity: Option<arrow::buffer::NullBuffer> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        as_array_ref(PrimitiveArray::<UInt16Type>::new(
                            ScalarBuffer::from(
                                id.into_iter()
                                    .map(|v| v.unwrap_or_default())
                                    .collect::<Vec<_>>(),
                            ),
                            id_validity,
                        ))
                    },
                    {
                        let (somes, label): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum =
                                    datum.as_ref().map(|datum| datum.label.clone()).flatten();
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let label_validity: Option<arrow::buffer::NullBuffer> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        {
                            let offsets = arrow::buffer::OffsetBuffer::<i32>::from_lengths(
                                label.iter().map(|opt| {
                                    opt.as_ref().map(|datum| datum.0.len()).unwrap_or_default()
                                }),
                            );
                            #[allow(clippy::unwrap_used)]
                            let capacity = offsets.last().copied().unwrap() as usize;
                            let mut buffer_builder =
                                arrow::array::builder::BufferBuilder::<u8>::new(capacity);
                            for data in label.iter().flatten() {
                                buffer_builder.append_slice(data.0.as_bytes());
                            }
                            let inner_data: arrow::buffer::Buffer = buffer_builder.finish();

                            #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                            as_array_ref(unsafe {
                                StringArray::new_unchecked(offsets, inner_data, label_validity)
                            })
                        }
                    },
                    {
                        let (somes, color): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum =
                                    datum.as_ref().map(|datum| datum.color.clone()).flatten();
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let color_validity: Option<arrow::buffer::NullBuffer> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        as_array_ref(PrimitiveArray::<UInt32Type>::new(
                            ScalarBuffer::from(
                                color
                                    .into_iter()
                                    .map(|datum| datum.map(|datum| datum.0).unwrap_or_default())
                                    .collect::<Vec<_>>(),
                            ),
                            color_validity,
                        ))
                    },
                ],
                validity,
            ))
        })
    }

    fn from_arrow_opt(
        arrow_data: &dyn arrow::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        #![allow(clippy::wildcard_imports)]
        use ::re_types_core::{arrow_zip_validity::ZipValidity, Loggable as _, ResultExt as _};
        use arrow::{array::*, buffer::*, datatypes::*};
        Ok({
            let arrow_data = arrow_data
                .as_any()
                .downcast_ref::<arrow::array::StructArray>()
                .ok_or_else(|| {
                    let expected = Self::arrow_datatype();
                    let actual = arrow_data.data_type().clone();
                    DeserializationError::datatype_mismatch(expected, actual)
                })
                .with_context("rerun.datatypes.AnnotationInfo")?;
            if arrow_data.is_empty() {
                Vec::new()
            } else {
                let (arrow_data_fields, arrow_data_arrays) =
                    (arrow_data.fields(), arrow_data.columns());
                let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data_fields
                    .iter()
                    .map(|field| field.name().as_str())
                    .zip(arrow_data_arrays)
                    .collect();
                let id = {
                    if !arrays_by_name.contains_key("id") {
                        return Err(DeserializationError::missing_struct_field(
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
                            let expected = DataType::UInt16;
                            let actual = arrow_data.data_type().clone();
                            DeserializationError::datatype_mismatch(expected, actual)
                        })
                        .with_context("rerun.datatypes.AnnotationInfo#id")?
                        .into_iter()
                };
                let label = {
                    if !arrays_by_name.contains_key("label") {
                        return Err(DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "label",
                        ))
                        .with_context("rerun.datatypes.AnnotationInfo");
                    }
                    let arrow_data = &**arrays_by_name["label"];
                    {
                        let arrow_data = arrow_data
                            .as_any()
                            .downcast_ref::<StringArray>()
                            .ok_or_else(|| {
                                let expected = DataType::Utf8;
                                let actual = arrow_data.data_type().clone();
                                DeserializationError::datatype_mismatch(expected, actual)
                            })
                            .with_context("rerun.datatypes.AnnotationInfo#label")?;
                        let arrow_data_buf = arrow_data.values();
                        let offsets = arrow_data.offsets();
                        ZipValidity::new_with_validity(offsets.windows(2), arrow_data.nulls())
                            .map(|elem| {
                                elem.map(|window| {
                                    let start = window[0] as usize;
                                    let end = window[1] as usize;
                                    let len = end - start;
                                    if arrow_data_buf.len() < end {
                                        return Err(DeserializationError::offset_slice_oob(
                                            (start, end),
                                            arrow_data_buf.len(),
                                        ));
                                    }

                                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                    let data = arrow_data_buf.slice_with_length(start, len);
                                    Ok(data)
                                })
                                .transpose()
                            })
                            .map(|res_or_opt| {
                                res_or_opt.map(|res_or_opt| {
                                    res_or_opt.map(|v| {
                                        crate::datatypes::Utf8(::re_types_core::ArrowString::from(
                                            v,
                                        ))
                                    })
                                })
                            })
                            .collect::<DeserializationResult<Vec<Option<_>>>>()
                            .with_context("rerun.datatypes.AnnotationInfo#label")?
                            .into_iter()
                    }
                };
                let color = {
                    if !arrays_by_name.contains_key("color") {
                        return Err(DeserializationError::missing_struct_field(
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
                            let expected = DataType::UInt32;
                            let actual = arrow_data.data_type().clone();
                            DeserializationError::datatype_mismatch(expected, actual)
                        })
                        .with_context("rerun.datatypes.AnnotationInfo#color")?
                        .into_iter()
                        .map(|res_or_opt| res_or_opt.map(crate::datatypes::Rgba32))
                };
                ZipValidity::new_with_validity(
                    ::itertools::izip!(id, label, color),
                    arrow_data.nulls(),
                )
                .map(|opt| {
                    opt.map(|(id, label, color)| {
                        Ok(Self {
                            id: id
                                .ok_or_else(DeserializationError::missing_data)
                                .with_context("rerun.datatypes.AnnotationInfo#id")?,
                            label,
                            color,
                        })
                    })
                    .transpose()
                })
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.datatypes.AnnotationInfo")?
            }
        })
    }
}

impl ::re_byte_size::SizeBytes for AnnotationInfo {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.id.heap_size_bytes() + self.label.heap_size_bytes() + self.color.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <u16>::is_pod()
            && <Option<crate::datatypes::Utf8>>::is_pod()
            && <Option<crate::datatypes::Rgba32>>::is_pod()
    }
}
