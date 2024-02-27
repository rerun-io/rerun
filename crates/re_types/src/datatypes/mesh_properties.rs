// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/datatypes/mesh_properties.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::iter_on_single_items)]
#![allow(clippy::map_flatten)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_cast)]

use ::re_types_core::external::arrow2;
use ::re_types_core::ComponentName;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Datatype**: Optional triangle indices for a mesh.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeshProperties {
    /// A flattened array of vertex indices that describe the mesh's triangles.
    ///
    /// Its length must be divisible by 3.
    pub indices: Option<::re_types_core::ArrowBuffer<u32>>,
}

impl ::re_types_core::SizeBytes for MeshProperties {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.indices.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Option<::re_types_core::ArrowBuffer<u32>>>::is_pod()
    }
}

impl From<Option<::re_types_core::ArrowBuffer<u32>>> for MeshProperties {
    #[inline]
    fn from(indices: Option<::re_types_core::ArrowBuffer<u32>>) -> Self {
        Self { indices }
    }
}

impl From<MeshProperties> for Option<::re_types_core::ArrowBuffer<u32>> {
    #[inline]
    fn from(value: MeshProperties) -> Self {
        value.indices
    }
}

::re_types_core::macros::impl_into_cow!(MeshProperties);

impl ::re_types_core::Loggable for MeshProperties {
    type Name = ::re_types_core::DatatypeName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.datatypes.MeshProperties".into()
    }

    #[allow(clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use arrow2::datatypes::*;
        DataType::Struct(std::sync::Arc::new(vec![Field {
            name: "indices".to_owned(),
            data_type: DataType::List(std::sync::Arc::new(Field {
                name: "item".to_owned(),
                data_type: DataType::UInt32,
                is_nullable: false,
                metadata: [].into(),
            })),
            is_nullable: true,
            metadata: [].into(),
        }]))
    }

    #[allow(clippy::wildcard_imports)]
    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, datatypes::*};
        Ok({
            let (somes, data): (Vec<_>, Vec<_>) = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    (datum.is_some(), datum)
                })
                .unzip();
            let bitmap: Option<arrow2::bitmap::Bitmap> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            StructArray::new(
                <crate::datatypes::MeshProperties>::arrow_datatype(),
                vec![{
                    let (somes, indices): (Vec<_>, Vec<_>) = data
                        .iter()
                        .map(|datum| {
                            let datum = datum
                                .as_ref()
                                .map(|datum| {
                                    let Self { indices, .. } = &**datum;
                                    indices.clone()
                                })
                                .flatten();
                            (datum.is_some(), datum)
                        })
                        .unzip();
                    let indices_bitmap: Option<arrow2::bitmap::Bitmap> = {
                        let any_nones = somes.iter().any(|some| !*some);
                        any_nones.then(|| somes.into())
                    };
                    {
                        use arrow2::{buffer::Buffer, offset::OffsetsBuffer};
                        let indices_inner_data: Buffer<_> = indices
                            .iter()
                            .flatten()
                            .map(|b| b.as_slice())
                            .collect::<Vec<_>>()
                            .concat()
                            .into();
                        let indices_inner_bitmap: Option<arrow2::bitmap::Bitmap> = None;
                        let offsets = arrow2::offset::Offsets::<i32>::try_from_lengths(
                            indices.iter().map(|opt| {
                                opt.as_ref()
                                    .map(|datum| datum.num_instances())
                                    .unwrap_or_default()
                            }),
                        )
                        .unwrap()
                        .into();
                        ListArray::new(
                            DataType::List(std::sync::Arc::new(Field {
                                name: "item".to_owned(),
                                data_type: DataType::UInt32,
                                is_nullable: false,
                                metadata: [].into(),
                            })),
                            offsets,
                            PrimitiveArray::new(
                                DataType::UInt32,
                                indices_inner_data,
                                indices_inner_bitmap,
                            )
                            .boxed(),
                            indices_bitmap,
                        )
                        .boxed()
                    }
                }],
                bitmap,
            )
            .boxed()
        })
    }

    #[allow(clippy::wildcard_imports)]
    fn from_arrow_opt(
        arrow_data: &dyn arrow2::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, buffer::*, datatypes::*};
        Ok({
            let arrow_data = arrow_data
                .as_any()
                .downcast_ref::<arrow2::array::StructArray>()
                .ok_or_else(|| {
                    let expected = Self::arrow_datatype();
                    let actual = arrow_data.data_type().clone();
                    DeserializationError::datatype_mismatch(expected, actual)
                })
                .with_context("rerun.datatypes.MeshProperties")?;
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
                let indices = {
                    if !arrays_by_name.contains_key("indices") {
                        return Err(DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "indices",
                        ))
                        .with_context("rerun.datatypes.MeshProperties");
                    }
                    let arrow_data = &**arrays_by_name["indices"];
                    {
                        let arrow_data = arrow_data
                            .as_any()
                            .downcast_ref::<arrow2::array::ListArray<i32>>()
                            .ok_or_else(|| {
                                let expected = DataType::List(std::sync::Arc::new(Field {
                                    name: "item".to_owned(),
                                    data_type: DataType::UInt32,
                                    is_nullable: false,
                                    metadata: [].into(),
                                }));
                                let actual = arrow_data.data_type().clone();
                                DeserializationError::datatype_mismatch(expected, actual)
                            })
                            .with_context("rerun.datatypes.MeshProperties#indices")?;
                        if arrow_data.is_empty() {
                            Vec::new()
                        } else {
                            let arrow_data_inner = {
                                let arrow_data_inner = &**arrow_data.values();
                                arrow_data_inner
                                    .as_any()
                                    .downcast_ref::<UInt32Array>()
                                    .ok_or_else(|| {
                                        let expected = DataType::UInt32;
                                        let actual = arrow_data_inner.data_type().clone();
                                        DeserializationError::datatype_mismatch(expected, actual)
                                    })
                                    .with_context("rerun.datatypes.MeshProperties#indices")?
                                    .values()
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
                                        return Err(DeserializationError::offset_slice_oob(
                                            (start, end),
                                            arrow_data_inner.len(),
                                        ));
                                    }

                                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                    let data = unsafe {
                                        arrow_data_inner
                                            .clone()
                                            .sliced_unchecked(start as usize, end - start as usize)
                                    };
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
                arrow2::bitmap::utils::ZipValidity::new_with_validity(
                    ::itertools::izip!(indices),
                    arrow_data.validity(),
                )
                .map(|opt| opt.map(|(indices)| Ok(Self { indices })).transpose())
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.datatypes.MeshProperties")?
            }
        })
    }
}
