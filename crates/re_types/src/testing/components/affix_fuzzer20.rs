// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AffixFuzzer20(pub crate::testing::datatypes::AffixFuzzer20);

impl ::re_types_core::SizeBytes for AffixFuzzer20 {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        [self.0.heap_size_bytes()].into_iter().sum::<u64>()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::testing::datatypes::AffixFuzzer20>::is_pod()
    }
}

impl<T: Into<crate::testing::datatypes::AffixFuzzer20>> From<T> for AffixFuzzer20 {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::testing::datatypes::AffixFuzzer20> for AffixFuzzer20 {
    #[inline]
    fn borrow(&self) -> &crate::testing::datatypes::AffixFuzzer20 {
        &self.0
    }
}

impl std::ops::Deref for AffixFuzzer20 {
    type Target = crate::testing::datatypes::AffixFuzzer20;

    #[inline]
    fn deref(&self) -> &crate::testing::datatypes::AffixFuzzer20 {
        &self.0
    }
}

::re_types_core::macros::impl_into_cow!(AffixFuzzer20);

impl ::re_types_core::Loggable for AffixFuzzer20 {
    type Name = ::re_types_core::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.testing.components.AffixFuzzer20".into()
    }

    #[allow(clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use arrow2::datatypes::*;
        DataType::Struct(vec![
            Field {
                name: "p".to_owned(),
                data_type: <crate::testing::datatypes::PrimitiveComponent>::arrow_datatype(),
                is_nullable: false,
                metadata: [].into(),
            },
            Field {
                name: "s".to_owned(),
                data_type: <crate::testing::datatypes::StringComponent>::arrow_datatype(),
                is_nullable: false,
                metadata: [].into(),
            },
        ])
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
                _ = data0_bitmap;
                crate::testing::datatypes::AffixFuzzer20::to_arrow_opt(data0)?
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
        Ok(
            crate::testing::datatypes::AffixFuzzer20::from_arrow_opt(arrow_data)
                .with_context("rerun.testing.components.AffixFuzzer20#nested_transparent")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .map(|res| res.map(|v| Some(Self(v))))
                .collect::<DeserializationResult<Vec<Option<_>>>>()
                .with_context("rerun.testing.components.AffixFuzzer20#nested_transparent")
                .with_context("rerun.testing.components.AffixFuzzer20")?,
        )
    }
}
