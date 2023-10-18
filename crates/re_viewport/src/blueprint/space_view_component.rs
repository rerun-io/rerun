// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/blueprint/space_view_component.fbs".

#![allow(trivial_numeric_casts)]
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

/// **Blueprint**: A view of a space.
///
/// Unstable. Used for the ongoing blueprint experimentations.
#[derive(Clone)]
pub struct SpaceViewComponent {
    pub space_view: crate::SpaceViewBlueprint,
}

impl From<crate::SpaceViewBlueprint> for SpaceViewComponent {
    #[inline]
    fn from(space_view: crate::SpaceViewBlueprint) -> Self {
        Self { space_view }
    }
}

impl From<SpaceViewComponent> for crate::SpaceViewBlueprint {
    #[inline]
    fn from(value: SpaceViewComponent) -> Self {
        value.space_view
    }
}

impl<'a> From<SpaceViewComponent> for ::std::borrow::Cow<'a, SpaceViewComponent> {
    #[inline]
    fn from(value: SpaceViewComponent) -> Self {
        std::borrow::Cow::Owned(value)
    }
}

impl<'a> From<&'a SpaceViewComponent> for ::std::borrow::Cow<'a, SpaceViewComponent> {
    #[inline]
    fn from(value: &'a SpaceViewComponent) -> Self {
        std::borrow::Cow::Borrowed(value)
    }
}

impl ::re_types_core::Loggable for SpaceViewComponent {
    type Name = ::re_types_core::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.blueprint.SpaceViewComponent".into()
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use arrow2::datatypes::*;
        DataType::Struct(vec![Field {
            name: "space_view".to_owned(),
            data_type: DataType::List(Box::new(Field {
                name: "item".to_owned(),
                data_type: DataType::UInt8,
                is_nullable: false,
                metadata: [].into(),
            })),
            is_nullable: false,
            metadata: [].into(),
        }])
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> ::re_types_core::SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        re_tracing::profile_function!();
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
                <crate::blueprint::SpaceViewComponent>::arrow_datatype(),
                vec![{
                    let (somes, space_view): (Vec<_>, Vec<_>) = data
                        .iter()
                        .map(|datum| {
                            let datum = datum.as_ref().map(|datum| {
                                let Self { space_view, .. } = &**datum;
                                space_view.clone()
                            });
                            (datum.is_some(), datum)
                        })
                        .unzip();
                    let space_view_bitmap: Option<arrow2::bitmap::Bitmap> = {
                        let any_nones = somes.iter().any(|some| !*some);
                        any_nones.then(|| somes.into())
                    };
                    {
                        use arrow2::{buffer::Buffer, offset::OffsetsBuffer};
                        let buffers: Vec<Option<Vec<u8>>> = space_view
                            .iter()
                            .map(|opt| {
                                use ::re_types_core::SerializationError;
                                opt.as_ref()
                                    .map(|b| {
                                        let mut buf = Vec::new();
                                        rmp_serde::encode::write_named(&mut buf, b).map_err(
                                            |err| {
                                                SerializationError::serde_failure(err.to_string())
                                            },
                                        )?;
                                        Ok(buf)
                                    })
                                    .transpose()
                            })
                            .collect::<::re_types_core::SerializationResult<Vec<_>>>()?;
                        let offsets =
                        ::re_types_core::external::arrow2::offset::Offsets:: < i32 >
                        ::try_from_lengths(buffers.iter().map(| opt | opt.as_ref().map(|
                        buf | buf.len()).unwrap_or_default())).unwrap().into();
                        let space_view_inner_bitmap: Option<arrow2::bitmap::Bitmap> = None;
                        let space_view_inner_data: Buffer<u8> = buffers
                            .into_iter()
                            .flatten()
                            .collect::<Vec<_>>()
                            .concat()
                            .into();
                        ListArray::new(
                            DataType::List(Box::new(Field {
                                name: "item".to_owned(),
                                data_type: DataType::UInt8,
                                is_nullable: false,
                                metadata: [].into(),
                            })),
                            offsets,
                            PrimitiveArray::new(
                                DataType::UInt8,
                                space_view_inner_data,
                                space_view_inner_bitmap,
                            )
                            .boxed(),
                            space_view_bitmap,
                        )
                        .boxed()
                    }
                }],
                bitmap,
            )
            .boxed()
        })
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    fn from_arrow_opt(
        arrow_data: &dyn arrow2::array::Array,
    ) -> ::re_types_core::DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        re_tracing::profile_function!();
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, buffer::*, datatypes::*};
        Ok({
            let arrow_data = arrow_data
                .as_any()
                .downcast_ref::<arrow2::array::StructArray>()
                .ok_or_else(|| {
                    ::re_types_core::DeserializationError::datatype_mismatch(
                        DataType::Struct(vec![Field {
                            name: "space_view".to_owned(),
                            data_type: DataType::List(Box::new(Field {
                                name: "item".to_owned(),
                                data_type: DataType::UInt8,
                                is_nullable: false,
                                metadata: [].into(),
                            })),
                            is_nullable: false,
                            metadata: [].into(),
                        }]),
                        arrow_data.data_type().clone(),
                    )
                })
                .with_context("rerun.blueprint.SpaceViewComponent")?;
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
                let space_view = {
                    if !arrays_by_name.contains_key("space_view") {
                        return Err(::re_types_core::DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "space_view",
                        ))
                        .with_context("rerun.blueprint.SpaceViewComponent");
                    }
                    let arrow_data = &**arrays_by_name["space_view"];
                    {
                        let arrow_data = arrow_data
                            .as_any()
                            .downcast_ref::<arrow2::array::ListArray<i32>>()
                            .ok_or_else(|| {
                                ::re_types_core::DeserializationError::datatype_mismatch(
                                    DataType::List(Box::new(Field {
                                        name: "item".to_owned(),
                                        data_type: DataType::UInt8,
                                        is_nullable: false,
                                        metadata: [].into(),
                                    })),
                                    arrow_data.data_type().clone(),
                                )
                            })
                            .with_context("rerun.blueprint.SpaceViewComponent#space_view")?;
                        if arrow_data.is_empty() {
                            Vec::new()
                        } else {
                            let arrow_data_inner = {
                                let arrow_data_inner = &**arrow_data.values();
                                arrow_data_inner
                                    .as_any()
                                    .downcast_ref::<UInt8Array>()
                                    .ok_or_else(|| {
                                        ::re_types_core::DeserializationError::datatype_mismatch(
                                            DataType::UInt8,
                                            arrow_data_inner.data_type().clone(),
                                        )
                                    })
                                    .with_context("rerun.blueprint.SpaceViewComponent#space_view")?
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
                                        return Err(
                                            ::re_types_core::DeserializationError::offset_slice_oob(
                                                (start, end),
                                                arrow_data_inner.len(),
                                            ),
                                        );
                                    }

                                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                    let data = unsafe {
                                        arrow_data_inner
                                            .clone()
                                            .sliced_unchecked(start as usize, end - start as usize)
                                    };
                                    let data = rmp_serde::from_slice::<crate::SpaceViewBlueprint>(
                                        data.as_slice(),
                                    )
                                    .map_err(|err| {
                                        ::re_types_core::DeserializationError::serde_failure(
                                            err.to_string(),
                                        )
                                    })?;
                                    Ok(data)
                                })
                                .transpose()
                            })
                            .collect::<::re_types_core::DeserializationResult<Vec<Option<_>>>>()?
                        }
                        .into_iter()
                    }
                };
                arrow2::bitmap::utils::ZipValidity::new_with_validity(
                    ::itertools::izip!(space_view),
                    arrow_data.validity(),
                )
                .map(|opt| {
                    opt.map(|(space_view)| {
                        Ok(Self {
                            space_view: space_view
                                .ok_or_else(::re_types_core::DeserializationError::missing_data)
                                .with_context("rerun.blueprint.SpaceViewComponent#space_view")?,
                        })
                    })
                    .transpose()
                })
                .collect::<::re_types_core::DeserializationResult<Vec<_>>>()
                .with_context("rerun.blueprint.SpaceViewComponent")?
            }
        })
    }
}
