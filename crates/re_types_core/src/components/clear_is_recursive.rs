// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/components/clear_is_recursive.fbs".

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

use crate::external::arrow2;
use crate::ComponentName;
use crate::SerializationResult;
use crate::{ComponentBatch, MaybeOwnedComponentBatch};
use crate::{DeserializationError, DeserializationResult};

/// **Component**: Configures how a clear operation should behave - recursive or not.
#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub struct ClearIsRecursive(
    /// If true, also clears all recursive children entities.
    pub bool,
);

impl crate::SizeBytes for ClearIsRecursive {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        [self.0.heap_size_bytes()].into_iter().sum::<u64>()
    }

    #[inline]
    fn is_pod() -> bool {
        <bool>::is_pod()
    }
}

impl From<bool> for ClearIsRecursive {
    #[inline]
    fn from(recursive: bool) -> Self {
        Self(recursive)
    }
}

impl From<ClearIsRecursive> for bool {
    #[inline]
    fn from(value: ClearIsRecursive) -> Self {
        value.0
    }
}

crate::macros::impl_into_cow!(ClearIsRecursive);

impl crate::Loggable for ClearIsRecursive {
    type Name = crate::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.components.ClearIsRecursive".into()
    }

    #[allow(clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use arrow2::datatypes::*;
        DataType::Boolean
    }

    #[allow(clippy::wildcard_imports)]
    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        use crate::{Loggable as _, ResultExt as _};
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
            BooleanArray::new(
                Self::arrow_datatype(),
                data0.into_iter().map(|v| v.unwrap_or_default()).collect(),
                data0_bitmap,
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
        use crate::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, buffer::*, datatypes::*};
        Ok(arrow_data
            .as_any()
            .downcast_ref::<BooleanArray>()
            .ok_or_else(|| {
                DeserializationError::datatype_mismatch(
                    DataType::Boolean,
                    arrow_data.data_type().clone(),
                )
            })
            .with_context("rerun.components.ClearIsRecursive#recursive")?
            .into_iter()
            .map(|v| v.ok_or_else(DeserializationError::missing_data))
            .map(|res| res.map(|v| Some(Self(v))))
            .collect::<DeserializationResult<Vec<Option<_>>>>()
            .with_context("rerun.components.ClearIsRecursive#recursive")
            .with_context("rerun.components.ClearIsRecursive")?)
    }
}
