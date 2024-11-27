// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/testing/components/fuzzy.fbs".

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
use ::re_types_core::ComponentName;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{DeserializationError, DeserializationResult};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AffixFuzzer21(pub crate::testing::datatypes::AffixFuzzer21);

impl ::re_types_core::SizeBytes for AffixFuzzer21 {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::testing::datatypes::AffixFuzzer21>::is_pod()
    }
}

impl<T: Into<crate::testing::datatypes::AffixFuzzer21>> From<T> for AffixFuzzer21 {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::testing::datatypes::AffixFuzzer21> for AffixFuzzer21 {
    #[inline]
    fn borrow(&self) -> &crate::testing::datatypes::AffixFuzzer21 {
        &self.0
    }
}

impl std::ops::Deref for AffixFuzzer21 {
    type Target = crate::testing::datatypes::AffixFuzzer21;

    #[inline]
    fn deref(&self) -> &crate::testing::datatypes::AffixFuzzer21 {
        &self.0
    }
}

impl std::ops::DerefMut for AffixFuzzer21 {
    #[inline]
    fn deref_mut(&mut self) -> &mut crate::testing::datatypes::AffixFuzzer21 {
        &mut self.0
    }
}

::re_types_core::macros::impl_into_cow!(AffixFuzzer21);

impl ::re_types_core::Loggable for AffixFuzzer21 {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        crate::testing::datatypes::AffixFuzzer21::arrow_datatype()
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<arrow::array::ArrayRef>
    where
        Self: Clone + 'a,
    {
        crate::testing::datatypes::AffixFuzzer21::to_arrow_opt(data.into_iter().map(|datum| {
            datum.map(|datum| match datum.into() {
                ::std::borrow::Cow::Borrowed(datum) => ::std::borrow::Cow::Borrowed(&datum.0),
                ::std::borrow::Cow::Owned(datum) => ::std::borrow::Cow::Owned(datum.0),
            })
        }))
    }

    fn from_arrow2_opt(
        arrow_data: &dyn arrow2::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        crate::testing::datatypes::AffixFuzzer21::from_arrow2_opt(arrow_data)
            .map(|v| v.into_iter().map(|v| v.map(Self)).collect())
    }
}

impl ::re_types_core::Component for AffixFuzzer21 {
    #[inline]
    fn name() -> ComponentName {
        "rerun.testing.components.AffixFuzzer21".into()
    }
}
