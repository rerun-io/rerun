// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/blueprint/components/included_space_views.fbs".

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

/// **Component**: The id of a `SpaceView`.
///
/// Unstable. Used for the ongoing blueprint experimentations.
#[derive(Clone, Debug, Default)]
#[repr(transparent)]
pub struct IncludedSpaceViews(pub Vec<crate::datatypes::Uuid>);

impl ::re_types_core::SizeBytes for IncludedSpaceViews {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Vec<crate::datatypes::Uuid>>::is_pod()
    }
}

impl<I: Into<crate::datatypes::Uuid>, T: IntoIterator<Item = I>> From<T> for IncludedSpaceViews {
    fn from(v: T) -> Self {
        Self(v.into_iter().map(|v| v.into()).collect())
    }
}

::re_types_core::macros::impl_into_cow!(IncludedSpaceViews);

impl ::re_types_core::Loggable for IncludedSpaceViews {
    type Name = ::re_types_core::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.blueprint.components.IncludedSpaceViews".into()
    }

    #[allow(clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use arrow2::datatypes::*;
        DataType::List(std::sync::Arc::new(Field {
            name: "item".to_owned(),
            data_type: <crate::datatypes::Uuid>::arrow_datatype(),
            is_nullable: false,
            metadata: [].into(),
        }))
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
                let data0_inner_data: Vec<_> = data0
                    .iter()
                    .flatten()
                    .flatten()
                    .cloned()
                    .map(Some)
                    .collect();
                let data0_inner_bitmap: Option<arrow2::bitmap::Bitmap> = None;
                let offsets = arrow2::offset::Offsets::<i32>::try_from_lengths(
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
                        _ = data0_inner_bitmap;
                        crate::datatypes::Uuid::to_arrow_opt(data0_inner_data)?
                    },
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
                    let expected = DataType::List(std::sync::Arc::new(Field {
                        name: "item".to_owned(),
                        data_type: <crate::datatypes::Uuid>::arrow_datatype(),
                        is_nullable: false,
                        metadata: [].into(),
                    }));
                    let actual = arrow_data.data_type().clone();
                    DeserializationError::datatype_mismatch(expected, actual)
                })
                .with_context("rerun.blueprint.components.IncludedSpaceViews#space_view_ids")?;
            if arrow_data.is_empty() {
                Vec::new()
            } else {
                let arrow_data_inner = {
                    let arrow_data_inner = &**arrow_data.values();
                    crate::datatypes::Uuid::from_arrow_opt(arrow_data_inner)
                        .with_context(
                            "rerun.blueprint.components.IncludedSpaceViews#space_view_ids",
                        )?
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
                        if end as usize > arrow_data_inner.len() {
                            return Err(DeserializationError::offset_slice_oob(
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
                .collect::<DeserializationResult<Vec<Option<_>>>>()?
            }
            .into_iter()
        }
        .map(|v| v.ok_or_else(DeserializationError::missing_data))
        .map(|res| res.map(|v| Some(Self(v))))
        .collect::<DeserializationResult<Vec<Option<_>>>>()
        .with_context("rerun.blueprint.components.IncludedSpaceViews#space_view_ids")
        .with_context("rerun.blueprint.components.IncludedSpaceViews")?)
    }
}
