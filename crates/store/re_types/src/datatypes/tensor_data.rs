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

use ::re_types_core::try_serialize_field;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, SerializedComponentBatch};
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
    /// The shape of the tensor, i.e. the length of each dimension.
    pub shape: ::re_types_core::ArrowBuffer<u64>,

    /// The names of the dimensions of the tensor (optional).
    ///
    /// If set, should be the same length as [`datatypes::TensorData::shape`][crate::datatypes::TensorData::shape].
    /// If it has a different length your names may show up improperly,
    /// and some constructors may produce a warning or even an error.
    ///
    /// Example: `["height", "width", "channel", "batch"]`.
    pub names: Option<Vec<::re_types_core::ArrowString>>,

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
                    DataType::UInt64,
                    false,
                ))),
                false,
            ),
            Field::new(
                "names",
                DataType::List(std::sync::Arc::new(Field::new(
                    "item",
                    DataType::Utf8,
                    false,
                ))),
                true,
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
        use ::re_types_core::{arrow_helpers::as_array_ref, Loggable as _, ResultExt as _};
        use arrow::{array::*, buffer::*, datatypes::*};
        Ok({
            let fields = Fields::from(vec![
                Field::new(
                    "shape",
                    DataType::List(std::sync::Arc::new(Field::new(
                        "item",
                        DataType::UInt64,
                        false,
                    ))),
                    false,
                ),
                Field::new(
                    "names",
                    DataType::List(std::sync::Arc::new(Field::new(
                        "item",
                        DataType::Utf8,
                        false,
                    ))),
                    true,
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
                            let offsets =
                                arrow::buffer::OffsetBuffer::<i32>::from_lengths(shape.iter().map(
                                    |opt| opt.as_ref().map_or(0, |datum| datum.num_instances()),
                                ));
                            let shape_inner_data: ScalarBuffer<_> = shape
                                .iter()
                                .flatten()
                                .map(|b| b.as_slice())
                                .collect::<Vec<_>>()
                                .concat()
                                .into();
                            let shape_inner_validity: Option<arrow::buffer::NullBuffer> = None;
                            as_array_ref(ListArray::try_new(
                                std::sync::Arc::new(Field::new("item", DataType::UInt64, false)),
                                offsets,
                                as_array_ref(PrimitiveArray::<UInt64Type>::new(
                                    shape_inner_data,
                                    shape_inner_validity,
                                )),
                                shape_validity,
                            )?)
                        }
                    },
                    {
                        let (somes, names): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum =
                                    datum.as_ref().map(|datum| datum.names.clone()).flatten();
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let names_validity: Option<arrow::buffer::NullBuffer> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        {
                            let offsets = arrow::buffer::OffsetBuffer::<i32>::from_lengths(
                                names
                                    .iter()
                                    .map(|opt| opt.as_ref().map_or(0, |datum| datum.len())),
                            );
                            let names_inner_data: Vec<_> =
                                names.into_iter().flatten().flatten().collect();
                            let names_inner_validity: Option<arrow::buffer::NullBuffer> = None;
                            as_array_ref(ListArray::try_new(
                                std::sync::Arc::new(Field::new("item", DataType::Utf8, false)),
                                offsets,
                                {
                                    let offsets = arrow::buffer::OffsetBuffer::<i32>::from_lengths(
                                        names_inner_data.iter().map(|datum| datum.len()),
                                    );
                                    #[allow(clippy::unwrap_used)]
                                    let capacity = offsets.last().copied().unwrap() as usize;
                                    let mut buffer_builder =
                                        arrow::array::builder::BufferBuilder::<u8>::new(capacity);
                                    for data in &names_inner_data {
                                        buffer_builder.append_slice(data.as_bytes());
                                    }
                                    let inner_data: arrow::buffer::Buffer = buffer_builder.finish();
                                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                    as_array_ref(unsafe {
                                        StringArray::new_unchecked(
                                            offsets,
                                            inner_data,
                                            names_inner_validity,
                                        )
                                    })
                                },
                                names_validity,
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
                .with_context("rerun.datatypes.TensorData")?;
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
                            .downcast_ref::<arrow::array::ListArray>()
                            .ok_or_else(|| {
                                let expected = DataType::List(std::sync::Arc::new(Field::new(
                                    "item",
                                    DataType::UInt64,
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
                                arrow_data_inner
                                    .as_any()
                                    .downcast_ref::<UInt64Array>()
                                    .ok_or_else(|| {
                                        let expected = DataType::UInt64;
                                        let actual = arrow_data_inner.data_type().clone();
                                        DeserializationError::datatype_mismatch(expected, actual)
                                    })
                                    .with_context("rerun.datatypes.TensorData#shape")?
                                    .values()
                            };
                            let offsets = arrow_data.offsets();
                            ZipValidity::new_with_validity(offsets.windows(2), arrow_data.nulls())
                                .map(|elem| {
                                    elem.map(|window| {
                                        let start = window[0] as usize;
                                        let end = window[1] as usize;
                                        if arrow_data_inner.len() < end {
                                            return Err(DeserializationError::offset_slice_oob(
                                                (start, end),
                                                arrow_data_inner.len(),
                                            ));
                                        }

                                        #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                        let data =
                                            arrow_data_inner.clone().slice(start, end - start);
                                        let data = ::re_types_core::ArrowBuffer::from(data);
                                        Ok(data)
                                    })
                                    .transpose()
                                })
                                .collect::<DeserializationResult<Vec<Option<_>>>>()?
                        }
                        .into_iter()
                    }
                };
                let names = {
                    if !arrays_by_name.contains_key("names") {
                        return Err(DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "names",
                        ))
                        .with_context("rerun.datatypes.TensorData");
                    }
                    let arrow_data = &**arrays_by_name["names"];
                    {
                        let arrow_data = arrow_data
                            .as_any()
                            .downcast_ref::<arrow::array::ListArray>()
                            .ok_or_else(|| {
                                let expected = DataType::List(std::sync::Arc::new(Field::new(
                                    "item",
                                    DataType::Utf8,
                                    false,
                                )));
                                let actual = arrow_data.data_type().clone();
                                DeserializationError::datatype_mismatch(expected, actual)
                            })
                            .with_context("rerun.datatypes.TensorData#names")?;
                        if arrow_data.is_empty() {
                            Vec::new()
                        } else {
                            let arrow_data_inner = {
                                let arrow_data_inner = &**arrow_data.values();
                                {
                                    let arrow_data_inner = arrow_data_inner
                                        .as_any()
                                        .downcast_ref::<StringArray>()
                                        .ok_or_else(|| {
                                            let expected = DataType::Utf8;
                                            let actual = arrow_data_inner.data_type().clone();
                                            DeserializationError::datatype_mismatch(
                                                expected, actual,
                                            )
                                        })
                                        .with_context("rerun.datatypes.TensorData#names")?;
                                    let arrow_data_inner_buf = arrow_data_inner.values();
                                    let offsets = arrow_data_inner.offsets();
                                    ZipValidity::new_with_validity(
                                        offsets.windows(2),
                                        arrow_data_inner.nulls(),
                                    )
                                    .map(|elem| {
                                        elem.map(|window| {
                                            let start = window[0] as usize;
                                            let end = window[1] as usize;
                                            let len = end - start;
                                            if arrow_data_inner_buf.len() < end {
                                                return Err(
                                                    DeserializationError::offset_slice_oob(
                                                        (start, end),
                                                        arrow_data_inner_buf.len(),
                                                    ),
                                                );
                                            }

                                            #[allow(
                                                unsafe_code,
                                                clippy::undocumented_unsafe_blocks
                                            )]
                                            let data =
                                                arrow_data_inner_buf.slice_with_length(start, len);
                                            Ok(data)
                                        })
                                        .transpose()
                                    })
                                    .map(|res_or_opt| {
                                        res_or_opt.map(|res_or_opt| {
                                            res_or_opt
                                                .map(|v| ::re_types_core::ArrowString::from(v))
                                        })
                                    })
                                    .collect::<DeserializationResult<Vec<Option<_>>>>()
                                    .with_context("rerun.datatypes.TensorData#names")?
                                    .into_iter()
                                }
                                .collect::<Vec<_>>()
                            };
                            let offsets = arrow_data.offsets();
                            ZipValidity::new_with_validity(offsets.windows(2), arrow_data.nulls())
                                .map(|elem| {
                                    elem.map(|window| {
                                        let start = window[0] as usize;
                                        let end = window[1] as usize;
                                        if arrow_data_inner.len() < end {
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
                    crate::datatypes::TensorBuffer::from_arrow_opt(arrow_data)
                        .with_context("rerun.datatypes.TensorData#buffer")?
                        .into_iter()
                };
                ZipValidity::new_with_validity(
                    ::itertools::izip!(shape, names, buffer),
                    arrow_data.nulls(),
                )
                .map(|opt| {
                    opt.map(|(shape, names, buffer)| {
                        Ok(Self {
                            shape: shape
                                .ok_or_else(DeserializationError::missing_data)
                                .with_context("rerun.datatypes.TensorData#shape")?,
                            names,
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

impl ::re_byte_size::SizeBytes for TensorData {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.shape.heap_size_bytes() + self.names.heap_size_bytes() + self.buffer.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <::re_types_core::ArrowBuffer<u64>>::is_pod()
            && <Option<Vec<::re_types_core::ArrowString>>>::is_pod()
            && <crate::datatypes::TensorBuffer>::is_pod()
    }
}
