// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/blueprint/components/space_view_maximized.fbs".

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

/// **Component**: Whether a space view is maximized.
///
/// Unstable. Used for the ongoing blueprint experimentations.
#[derive(Clone, Debug, Copy, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct SpaceViewMaximized(pub Option<crate::datatypes::Uuid>);

impl ::re_types_core::SizeBytes for SpaceViewMaximized {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Option<crate::datatypes::Uuid>>::is_pod()
    }
}

impl<T: Into<Option<crate::datatypes::Uuid>>> From<T> for SpaceViewMaximized {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<Option<crate::datatypes::Uuid>> for SpaceViewMaximized {
    #[inline]
    fn borrow(&self) -> &Option<crate::datatypes::Uuid> {
        &self.0
    }
}

impl std::ops::Deref for SpaceViewMaximized {
    type Target = Option<crate::datatypes::Uuid>;

    #[inline]
    fn deref(&self) -> &Option<crate::datatypes::Uuid> {
        &self.0
    }
}

::re_types_core::macros::impl_into_cow!(SpaceViewMaximized);

impl ::re_types_core::Loggable for SpaceViewMaximized {
    type Name = ::re_types_core::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.blueprint.components.SpaceViewMaximized".into()
    }

    #[allow(clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use arrow2::datatypes::*;
        DataType::Struct(std::sync::Arc::new(vec![Field {
            name: "bytes".to_owned(),
            data_type: DataType::FixedSizeList(
                std::sync::Arc::new(Field {
                    name: "item".to_owned(),
                    data_type: DataType::UInt8,
                    is_nullable: false,
                    metadata: [].into(),
                }),
                16usize,
            ),
            is_nullable: false,
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
            let (somes, data0): (Vec<_>, Vec<_>) = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    let datum = datum
                        .map(|datum| {
                            let Self(data0) = datum.into_owned();
                            data0
                        })
                        .flatten();
                    (datum.is_some(), datum)
                })
                .unzip();
            let data0_bitmap: Option<arrow2::bitmap::Bitmap> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            {
                _ = data0_bitmap;
                crate::datatypes::Uuid::to_arrow_opt(data0)?
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
        Ok(crate::datatypes::Uuid::from_arrow_opt(arrow_data)
            .with_context("rerun.blueprint.components.SpaceViewMaximized#space_view_id")?
            .into_iter()
            .map(Ok)
            .map(|res| res.map(|v| Some(Self(v))))
            .collect::<DeserializationResult<Vec<Option<_>>>>()
            .with_context("rerun.blueprint.components.SpaceViewMaximized#space_view_id")
            .with_context("rerun.blueprint.components.SpaceViewMaximized")?)
    }
}
