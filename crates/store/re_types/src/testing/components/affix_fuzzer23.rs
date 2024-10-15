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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AffixFuzzer23(pub Option<crate::testing::datatypes::MultiEnum>);

impl ::re_types_core::SizeBytes for AffixFuzzer23 {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Option<crate::testing::datatypes::MultiEnum>>::is_pod()
    }
}

impl<T: Into<Option<crate::testing::datatypes::MultiEnum>>> From<T> for AffixFuzzer23 {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<Option<crate::testing::datatypes::MultiEnum>> for AffixFuzzer23 {
    #[inline]
    fn borrow(&self) -> &Option<crate::testing::datatypes::MultiEnum> {
        &self.0
    }
}

impl std::ops::Deref for AffixFuzzer23 {
    type Target = Option<crate::testing::datatypes::MultiEnum>;

    #[inline]
    fn deref(&self) -> &Option<crate::testing::datatypes::MultiEnum> {
        &self.0
    }
}

impl std::ops::DerefMut for AffixFuzzer23 {
    #[inline]
    fn deref_mut(&mut self) -> &mut Option<crate::testing::datatypes::MultiEnum> {
        &mut self.0
    }
}

::re_types_core::macros::impl_into_cow!(AffixFuzzer23);

impl ::re_types_core::Loggable for AffixFuzzer23 {
    type Name = ::re_types_core::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.testing.components.AffixFuzzer23".into()
    }

    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        #![allow(clippy::wildcard_imports)]
        use arrow2::datatypes::*;
        DataType::Struct(std::sync::Arc::new(vec![
            Field::new(
                "value1",
                <crate::testing::datatypes::EnumTest>::arrow_datatype(),
                false,
            ),
            Field::new(
                "value2",
                <crate::testing::datatypes::ValuedEnum>::arrow_datatype(),
                true,
            ),
        ]))
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        #![allow(clippy::wildcard_imports)]
        #![allow(clippy::manual_is_variant_and)]
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, datatypes::*};
        Ok({
            let (somes, data0): (Vec<_>, Vec<_>) = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    let datum = datum.map(|datum| datum.into_owned().0).flatten();
                    (datum.is_some(), datum)
                })
                .unzip();
            let data0_bitmap: Option<arrow2::bitmap::Bitmap> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            {
                _ = data0_bitmap;
                crate::testing::datatypes::MultiEnum::to_arrow_opt(data0)?
            }
        })
    }

    fn from_arrow_opt(
        arrow_data: &dyn arrow2::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        #![allow(clippy::wildcard_imports)]
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, buffer::*, datatypes::*};
        Ok(
            crate::testing::datatypes::MultiEnum::from_arrow_opt(arrow_data)
                .with_context("rerun.testing.components.AffixFuzzer23#multi_enum")?
                .into_iter()
                .map(Ok)
                .map(|res| res.map(|v| Some(Self(v))))
                .collect::<DeserializationResult<Vec<Option<_>>>>()
                .with_context("rerun.testing.components.AffixFuzzer23#multi_enum")
                .with_context("rerun.testing.components.AffixFuzzer23")?,
        )
    }
}

impl ::re_types_core::AsComponents for AffixFuzzer23 {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        vec![(self as &dyn ComponentBatch).into()]
    }
}
