// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/datatypes/tensor_data.fbs".

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

use ::re_types_core::external::arrow2;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, ComponentBatchCowWithDescriptor};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Datatype**: An N-dimensional array of numbers.
///
/// The number of dimensions and their respective lengths is specified by the `shape` field.
/// The dimensions are ordered from outermost to innermost. For example, in the common case of
/// a 2D RGB Image, the shape would be `[height, width, channel]`.
///
/// These dimensions are combined with an index to look up values from the `buffer` field,
/// which stores a contiguous array of typed values.
#[derive(Clone, Debug, PartialEq)]
pub struct TensorData {
    /// The shape of the tensor, including optional names for each dimension.
    pub shape: Vec<crate::datatypes::TensorDimension>,

    /// The content/data.
    pub buffer: crate::datatypes::TensorBuffer,
}

::re_types_core::macros::impl_into_cow!(TensorData);

impl ::re_types_core::Loggable for TensorData {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        #![allow(clippy::wildcard_imports)]
        use arrow::datatypes::*;
        DataType::Struct(Fields::from(vec![
            Field::new(
                "shape",
                DataType::List(std::sync::Arc::new(Field::new(
                    "item",
                    <crate::datatypes::TensorDimension>::arrow_datatype(),
                    false,
                ))),
                false,
            ),
            Field::new(
                "buffer",
                <crate::datatypes::TensorBuffer>::arrow_datatype(),
                true,
            ),
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
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow::{array::*, buffer::*, datatypes::*};

        #[allow(unused)]
        fn as_array_ref<T: Array + 'static>(t: T) -> ArrayRef {
            std::sync::Arc::new(t) as ArrayRef
        }
        Ok({
            let fields = Fields::from(vec![
                Field::new(
                    "shape",
                    DataType::List(std::sync::Arc::new(Field::new(
                        "item",
                        <crate::datatypes::TensorDimension>::arrow_datatype(),
                        false,
                    ))),
                    false,
                ),
                Field::new(
                    "buffer",
                    <crate::datatypes::TensorBuffer>::arrow_datatype(),
                    true,
                ),
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
                        let (somes, shape): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum.as_ref().map(|datum| datum.shape.clone());
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let shape_validity: Option<arrow::buffer::NullBuffer> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        {
                            let offsets = arrow::buffer::OffsetBuffer::<i32>::from_lengths(
                                shape
                                    .iter()
                                    .map(|opt| opt.as_ref().map_or(0, |datum| datum.len())),
                            );
                            let shape_inner_data: Vec<_> =
                                shape.into_iter().flatten().flatten().collect();
                            let shape_inner_validity: Option<arrow::buffer::NullBuffer> = None;
                            as_array_ref(ListArray::try_new(
                                std::sync::Arc::new(Field::new(
                                    "item",
                                    <crate::datatypes::TensorDimension>::arrow_datatype(),
                                    false,
                                )),
                                offsets,
                                {
                                    _ = shape_inner_validity;
                                    crate::datatypes::TensorDimension::to_arrow_opt(
                                        shape_inner_data.into_iter().map(Some),
                                    )?
                                },
                                shape_validity,
                            )?)
                        }
                    },
                    {
                        let (somes, buffer): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum.as_ref().map(|datum| datum.buffer.clone());
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let buffer_validity: Option<arrow::buffer::NullBuffer> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        {
                            _ = buffer_validity;
                            crate::datatypes::TensorBuffer::to_arrow_opt(buffer)?
                        }
                    },
                ],
                validity,
            ))
        })
    }

    fn from_arrow2_opt(
        arrow_data: &dyn arrow2::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        #![allow(clippy::wildcard_imports)]
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow::datatypes::*;
        use arrow2::{array::*, buffer::*};
        Ok({
            let arrow_data = arrow_data
                .as_any()
                .downcast_ref::<arrow2::array::StructArray>()
                .ok_or_else(|| {
                    let expected = Self::arrow_datatype();
                    let actual = arrow_data.data_type().clone();
                    DeserializationError::datatype_mismatch(expected, actual)
                })
                .with_context("rerun.datatypes.TensorData")?;
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
                let shape = {
                    if !arrays_by_name.contains_key("shape") {
                        return Err(DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "shape",
                        ))
                        .with_context("rerun.datatypes.TensorData");
                    }
                    let arrow_data = &**arrays_by_name["shape"];
                    {
                        let arrow_data = arrow_data
                            .as_any()
                            .downcast_ref::<arrow2::array::ListArray<i32>>()
                            .ok_or_else(|| {
                                let expected = DataType::List(std::sync::Arc::new(Field::new(
                                    "item",
                                    <crate::datatypes::TensorDimension>::arrow_datatype(),
                                    false,
                                )));
                                let actual = arrow_data.data_type().clone();
                                DeserializationError::datatype_mismatch(expected, actual)
                            })
                            .with_context("rerun.datatypes.TensorData#shape")?;
                        if arrow_data.is_empty() {
                            Vec::new()
                        } else {
                            let arrow_data_inner = {
                                let arrow_data_inner = &**arrow_data.values();
                                crate::datatypes::TensorDimension::from_arrow2_opt(arrow_data_inner)
                                    .with_context("rerun.datatypes.TensorData#shape")?
                                    .into_iter()
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
                                    if end > arrow_data_inner.len() {
                                        return Err(DeserializationError::offset_slice_oob(
                                            (start, end),
                                            arrow_data_inner.len(),
                                        ));
                                    }

                                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                    let data =
                                        unsafe { arrow_data_inner.get_unchecked(start..end) };
                                    let data = data
                                        .iter()
                                        .cloned()
                                        .map(Option::unwrap_or_default)
                                        .collect();
                                    Ok(data)
                                })
                                .transpose()
                            })
                            .collect::<DeserializationResult<Vec<Option<_>>>>()?
                        }
                        .into_iter()
                    }
                };
                let buffer = {
                    if !arrays_by_name.contains_key("buffer") {
                        return Err(DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "buffer",
                        ))
                        .with_context("rerun.datatypes.TensorData");
                    }
                    let arrow_data = &**arrays_by_name["buffer"];
                    crate::datatypes::TensorBuffer::from_arrow2_opt(arrow_data)
                        .with_context("rerun.datatypes.TensorData#buffer")?
                        .into_iter()
                };
                arrow2::bitmap::utils::ZipValidity::new_with_validity(
                    ::itertools::izip!(shape, buffer),
                    arrow_data.validity(),
                )
                .map(|opt| {
                    opt.map(|(shape, buffer)| {
                        Ok(Self {
                            shape: shape
                                .ok_or_else(DeserializationError::missing_data)
                                .with_context("rerun.datatypes.TensorData#shape")?,
                            buffer: buffer
                                .ok_or_else(DeserializationError::missing_data)
                                .with_context("rerun.datatypes.TensorData#buffer")?,
                        })
                    })
                    .transpose()
                })
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.datatypes.TensorData")?
            }
        })
    }
}

impl ::re_types_core::SizeBytes for TensorData {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.shape.heap_size_bytes() + self.buffer.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Vec<crate::datatypes::TensorDimension>>::is_pod()
            && <crate::datatypes::TensorBuffer>::is_pod()
    }
}
