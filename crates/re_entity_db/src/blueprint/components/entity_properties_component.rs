// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/blueprint/components/entity_properties_component.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]
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

/// **Component**: The configurable set of overridable properties.
#[derive(Clone)]
pub struct EntityPropertiesComponent(pub crate::EntityProperties);

impl From<crate::EntityProperties> for EntityPropertiesComponent {
    #[inline]
    fn from(props: crate::EntityProperties) -> Self {
        Self(props)
    }
}

impl From<EntityPropertiesComponent> for crate::EntityProperties {
    #[inline]
    fn from(value: EntityPropertiesComponent) -> Self {
        value.0
    }
}

::re_types_core::macros::impl_into_cow!(EntityPropertiesComponent);

impl ::re_types_core::Loggable for EntityPropertiesComponent {
    type Name = ::re_types_core::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.blueprint.components.EntityPropertiesComponent".into()
    }

    #[allow(clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use arrow2::datatypes::*;
        DataType::List(std::sync::Arc::new(Field::new(
            "item",
            DataType::UInt8,
            false,
        )))
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
            let data0_bitmap: Option<arrow2::bitmap::Bitmap> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            {
                use arrow2::{buffer::Buffer, offset::OffsetsBuffer};
                let buffers: Vec<Option<Vec<u8>>> = data0
                    .iter()
                    .map(|opt| {
                        use ::re_types_core::SerializationError;
                        opt.as_ref()
                            .map(|b| {
                                let mut buf = Vec::new();
                                rmp_serde::encode::write_named(&mut buf, b).map_err(|err| {
                                    SerializationError::serde_failure(err.to_string())
                                })?;
                                Ok(buf)
                            })
                            .transpose()
                    })
                    .collect::<SerializationResult<Vec<_>>>()?;
                let offsets = arrow2::offset::Offsets::<i32>::try_from_lengths(
                    buffers
                        .iter()
                        .map(|opt| opt.as_ref().map(|buf| buf.len()).unwrap_or_default()),
                )
                .unwrap()
                .into();
                let data0_inner_bitmap: Option<arrow2::bitmap::Bitmap> = None;
                let data0_inner_data: Buffer<u8> = buffers
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .concat()
                    .into();
                ListArray::new(
                    Self::arrow_datatype(),
                    offsets,
                    PrimitiveArray::new(DataType::UInt8, data0_inner_data, data0_inner_bitmap)
                        .boxed(),
                    data0_bitmap,
                )
                .boxed()
            }
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
                .downcast_ref::<arrow2::array::ListArray<i32>>()
                .ok_or_else(|| {
                    let expected = Self::arrow_datatype();
                    let actual = arrow_data.data_type().clone();
                    DeserializationError::datatype_mismatch(expected, actual)
                })
                .with_context("rerun.blueprint.components.EntityPropertiesComponent#props")?;
            if arrow_data.is_empty() {
                Vec::new()
            } else {
                let arrow_data_inner = {
                    let arrow_data_inner = &**arrow_data.values();
                    arrow_data_inner
                        .as_any()
                        .downcast_ref::<UInt8Array>()
                        .ok_or_else(|| {
                            let expected = DataType::UInt8;
                            let actual = arrow_data_inner.data_type().clone();
                            DeserializationError::datatype_mismatch(expected, actual)
                        })
                        .with_context("rerun.blueprint.components.EntityPropertiesComponent#props")?
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
                        let data =
                            rmp_serde::from_slice::<crate::EntityProperties>(data.as_slice())
                                .map_err(|err| {
                                    DeserializationError::serde_failure(err.to_string())
                                })?;
                        Ok(data)
                    })
                    .transpose()
                })
                .collect::<DeserializationResult<Vec<Option<_>>>>()?
            }
            .into_iter()
        }
        .map(|v| v.ok_or_else(DeserializationError::missing_data))
        .map(|res| res.map(|v| Some(Self(v))))
        .collect::<DeserializationResult<Vec<Option<_>>>>()
        .with_context("rerun.blueprint.components.EntityPropertiesComponent#props")
        .with_context("rerun.blueprint.components.EntityPropertiesComponent")?)
    }
}
