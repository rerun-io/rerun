// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/testing/archetypes/fuzzy.fbs".

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

#[derive(Clone, Debug, PartialEq)]
pub struct AffixFuzzer1 {
    pub fuzz1001: crate::testing::components::AffixFuzzer1,
    pub fuzz1002: crate::testing::components::AffixFuzzer2,
    pub fuzz1003: crate::testing::components::AffixFuzzer3,
    pub fuzz1004: crate::testing::components::AffixFuzzer4,
    pub fuzz1005: crate::testing::components::AffixFuzzer5,
    pub fuzz1006: crate::testing::components::AffixFuzzer6,
    pub fuzz1007: crate::testing::components::AffixFuzzer7,
    pub fuzz1008: crate::testing::components::AffixFuzzer8,
    pub fuzz1009: crate::testing::components::AffixFuzzer9,
    pub fuzz1010: crate::testing::components::AffixFuzzer10,
    pub fuzz1011: crate::testing::components::AffixFuzzer11,
    pub fuzz1012: crate::testing::components::AffixFuzzer12,
    pub fuzz1013: crate::testing::components::AffixFuzzer13,
    pub fuzz1014: crate::testing::components::AffixFuzzer14,
    pub fuzz1015: crate::testing::components::AffixFuzzer15,
    pub fuzz1016: crate::testing::components::AffixFuzzer16,
    pub fuzz1017: crate::testing::components::AffixFuzzer17,
    pub fuzz1018: crate::testing::components::AffixFuzzer18,
    pub fuzz1019: crate::testing::components::AffixFuzzer19,
    pub fuzz1020: crate::testing::components::AffixFuzzer20,
    pub fuzz1021: crate::testing::components::AffixFuzzer21,
    pub fuzz1022: crate::testing::components::AffixFuzzer22,
}

impl ::re_types_core::SizeBytes for AffixFuzzer1 {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.fuzz1001.heap_size_bytes()
            + self.fuzz1002.heap_size_bytes()
            + self.fuzz1003.heap_size_bytes()
            + self.fuzz1004.heap_size_bytes()
            + self.fuzz1005.heap_size_bytes()
            + self.fuzz1006.heap_size_bytes()
            + self.fuzz1007.heap_size_bytes()
            + self.fuzz1008.heap_size_bytes()
            + self.fuzz1009.heap_size_bytes()
            + self.fuzz1010.heap_size_bytes()
            + self.fuzz1011.heap_size_bytes()
            + self.fuzz1012.heap_size_bytes()
            + self.fuzz1013.heap_size_bytes()
            + self.fuzz1014.heap_size_bytes()
            + self.fuzz1015.heap_size_bytes()
            + self.fuzz1016.heap_size_bytes()
            + self.fuzz1017.heap_size_bytes()
            + self.fuzz1018.heap_size_bytes()
            + self.fuzz1019.heap_size_bytes()
            + self.fuzz1020.heap_size_bytes()
            + self.fuzz1021.heap_size_bytes()
            + self.fuzz1022.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::testing::components::AffixFuzzer1>::is_pod()
            && <crate::testing::components::AffixFuzzer2>::is_pod()
            && <crate::testing::components::AffixFuzzer3>::is_pod()
            && <crate::testing::components::AffixFuzzer4>::is_pod()
            && <crate::testing::components::AffixFuzzer5>::is_pod()
            && <crate::testing::components::AffixFuzzer6>::is_pod()
            && <crate::testing::components::AffixFuzzer7>::is_pod()
            && <crate::testing::components::AffixFuzzer8>::is_pod()
            && <crate::testing::components::AffixFuzzer9>::is_pod()
            && <crate::testing::components::AffixFuzzer10>::is_pod()
            && <crate::testing::components::AffixFuzzer11>::is_pod()
            && <crate::testing::components::AffixFuzzer12>::is_pod()
            && <crate::testing::components::AffixFuzzer13>::is_pod()
            && <crate::testing::components::AffixFuzzer14>::is_pod()
            && <crate::testing::components::AffixFuzzer15>::is_pod()
            && <crate::testing::components::AffixFuzzer16>::is_pod()
            && <crate::testing::components::AffixFuzzer17>::is_pod()
            && <crate::testing::components::AffixFuzzer18>::is_pod()
            && <crate::testing::components::AffixFuzzer19>::is_pod()
            && <crate::testing::components::AffixFuzzer20>::is_pod()
            && <crate::testing::components::AffixFuzzer21>::is_pod()
            && <crate::testing::components::AffixFuzzer22>::is_pod()
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 22usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.testing.components.AffixFuzzer1".into(),
            "rerun.testing.components.AffixFuzzer2".into(),
            "rerun.testing.components.AffixFuzzer3".into(),
            "rerun.testing.components.AffixFuzzer4".into(),
            "rerun.testing.components.AffixFuzzer5".into(),
            "rerun.testing.components.AffixFuzzer6".into(),
            "rerun.testing.components.AffixFuzzer7".into(),
            "rerun.testing.components.AffixFuzzer8".into(),
            "rerun.testing.components.AffixFuzzer9".into(),
            "rerun.testing.components.AffixFuzzer10".into(),
            "rerun.testing.components.AffixFuzzer11".into(),
            "rerun.testing.components.AffixFuzzer12".into(),
            "rerun.testing.components.AffixFuzzer13".into(),
            "rerun.testing.components.AffixFuzzer14".into(),
            "rerun.testing.components.AffixFuzzer15".into(),
            "rerun.testing.components.AffixFuzzer16".into(),
            "rerun.testing.components.AffixFuzzer17".into(),
            "rerun.testing.components.AffixFuzzer18".into(),
            "rerun.testing.components.AffixFuzzer19".into(),
            "rerun.testing.components.AffixFuzzer20".into(),
            "rerun.testing.components.AffixFuzzer21".into(),
            "rerun.testing.components.AffixFuzzer22".into(),
        ]
    });

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.testing.components.AffixFuzzer1Indicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 23usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.testing.components.AffixFuzzer1".into(),
            "rerun.testing.components.AffixFuzzer2".into(),
            "rerun.testing.components.AffixFuzzer3".into(),
            "rerun.testing.components.AffixFuzzer4".into(),
            "rerun.testing.components.AffixFuzzer5".into(),
            "rerun.testing.components.AffixFuzzer6".into(),
            "rerun.testing.components.AffixFuzzer7".into(),
            "rerun.testing.components.AffixFuzzer8".into(),
            "rerun.testing.components.AffixFuzzer9".into(),
            "rerun.testing.components.AffixFuzzer10".into(),
            "rerun.testing.components.AffixFuzzer11".into(),
            "rerun.testing.components.AffixFuzzer12".into(),
            "rerun.testing.components.AffixFuzzer13".into(),
            "rerun.testing.components.AffixFuzzer14".into(),
            "rerun.testing.components.AffixFuzzer15".into(),
            "rerun.testing.components.AffixFuzzer16".into(),
            "rerun.testing.components.AffixFuzzer17".into(),
            "rerun.testing.components.AffixFuzzer18".into(),
            "rerun.testing.components.AffixFuzzer19".into(),
            "rerun.testing.components.AffixFuzzer20".into(),
            "rerun.testing.components.AffixFuzzer21".into(),
            "rerun.testing.components.AffixFuzzer22".into(),
            "rerun.testing.components.AffixFuzzer1Indicator".into(),
        ]
    });

impl AffixFuzzer1 {
    /// The total number of components in the archetype: 22 required, 1 recommended, 0 optional
    pub const NUM_COMPONENTS: usize = 23usize;
}

/// Indicator component for the [`AffixFuzzer1`] [`::re_types_core::Archetype`]
pub type AffixFuzzer1Indicator = ::re_types_core::GenericIndicatorComponent<AffixFuzzer1>;

impl ::re_types_core::Archetype for AffixFuzzer1 {
    type Indicator = AffixFuzzer1Indicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.testing.archetypes.AffixFuzzer1".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Affix fuzzer 1"
    }

    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        static INDICATOR: AffixFuzzer1Indicator = AffixFuzzer1Indicator::DEFAULT;
        MaybeOwnedComponentBatch::Ref(&INDICATOR)
    }

    #[inline]
    fn required_components() -> ::std::borrow::Cow<'static, [ComponentName]> {
        REQUIRED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn recommended_components() -> ::std::borrow::Cow<'static, [ComponentName]> {
        RECOMMENDED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn optional_components() -> ::std::borrow::Cow<'static, [ComponentName]> {
        OPTIONAL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn all_components() -> ::std::borrow::Cow<'static, [ComponentName]> {
        ALL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn from_arrow_components(
        arrow_data: impl IntoIterator<Item = (ComponentName, Box<dyn arrow2::array::Array>)>,
    ) -> DeserializationResult<Self> {
        re_tracing::profile_function!();
        use ::re_types_core::{Loggable as _, ResultExt as _};
        let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
            .into_iter()
            .map(|(name, array)| (name.full_name(), array))
            .collect();
        let fuzz1001 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer1")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1001")?;
            <crate::testing::components::AffixFuzzer1>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1001")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1001")?
        };
        let fuzz1002 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer2")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1002")?;
            <crate::testing::components::AffixFuzzer2>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1002")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1002")?
        };
        let fuzz1003 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer3")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1003")?;
            <crate::testing::components::AffixFuzzer3>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1003")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1003")?
        };
        let fuzz1004 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer4")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1004")?;
            <crate::testing::components::AffixFuzzer4>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1004")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1004")?
        };
        let fuzz1005 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer5")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1005")?;
            <crate::testing::components::AffixFuzzer5>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1005")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1005")?
        };
        let fuzz1006 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer6")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1006")?;
            <crate::testing::components::AffixFuzzer6>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1006")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1006")?
        };
        let fuzz1007 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer7")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1007")?;
            <crate::testing::components::AffixFuzzer7>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1007")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1007")?
        };
        let fuzz1008 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer8")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1008")?;
            <crate::testing::components::AffixFuzzer8>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1008")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1008")?
        };
        let fuzz1009 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer9")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1009")?;
            <crate::testing::components::AffixFuzzer9>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1009")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1009")?
        };
        let fuzz1010 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer10")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1010")?;
            <crate::testing::components::AffixFuzzer10>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1010")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1010")?
        };
        let fuzz1011 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer11")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1011")?;
            <crate::testing::components::AffixFuzzer11>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1011")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1011")?
        };
        let fuzz1012 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer12")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1012")?;
            <crate::testing::components::AffixFuzzer12>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1012")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1012")?
        };
        let fuzz1013 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer13")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1013")?;
            <crate::testing::components::AffixFuzzer13>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1013")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1013")?
        };
        let fuzz1014 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer14")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1014")?;
            <crate::testing::components::AffixFuzzer14>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1014")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1014")?
        };
        let fuzz1015 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer15")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1015")?;
            <crate::testing::components::AffixFuzzer15>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1015")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1015")?
        };
        let fuzz1016 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer16")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1016")?;
            <crate::testing::components::AffixFuzzer16>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1016")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1016")?
        };
        let fuzz1017 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer17")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1017")?;
            <crate::testing::components::AffixFuzzer17>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1017")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1017")?
        };
        let fuzz1018 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer18")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1018")?;
            <crate::testing::components::AffixFuzzer18>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1018")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1018")?
        };
        let fuzz1019 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer19")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1019")?;
            <crate::testing::components::AffixFuzzer19>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1019")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1019")?
        };
        let fuzz1020 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer20")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1020")?;
            <crate::testing::components::AffixFuzzer20>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1020")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1020")?
        };
        let fuzz1021 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer21")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1021")?;
            <crate::testing::components::AffixFuzzer21>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1021")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1021")?
        };
        let fuzz1022 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer22")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1022")?;
            <crate::testing::components::AffixFuzzer22>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1022")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer1#fuzz1022")?
        };
        Ok(Self {
            fuzz1001,
            fuzz1002,
            fuzz1003,
            fuzz1004,
            fuzz1005,
            fuzz1006,
            fuzz1007,
            fuzz1008,
            fuzz1009,
            fuzz1010,
            fuzz1011,
            fuzz1012,
            fuzz1013,
            fuzz1014,
            fuzz1015,
            fuzz1016,
            fuzz1017,
            fuzz1018,
            fuzz1019,
            fuzz1020,
            fuzz1021,
            fuzz1022,
        })
    }
}

impl ::re_types_core::AsComponents for AffixFuzzer1 {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.fuzz1001 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1002 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1003 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1004 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1005 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1006 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1007 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1008 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1009 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1010 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1011 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1012 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1013 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1014 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1015 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1016 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1017 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1018 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1019 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1020 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1021 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1022 as &dyn ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for AffixFuzzer1 {}

impl AffixFuzzer1 {
    /// Create a new `AffixFuzzer1`.
    #[inline]
    pub fn new(
        fuzz1001: impl Into<crate::testing::components::AffixFuzzer1>,
        fuzz1002: impl Into<crate::testing::components::AffixFuzzer2>,
        fuzz1003: impl Into<crate::testing::components::AffixFuzzer3>,
        fuzz1004: impl Into<crate::testing::components::AffixFuzzer4>,
        fuzz1005: impl Into<crate::testing::components::AffixFuzzer5>,
        fuzz1006: impl Into<crate::testing::components::AffixFuzzer6>,
        fuzz1007: impl Into<crate::testing::components::AffixFuzzer7>,
        fuzz1008: impl Into<crate::testing::components::AffixFuzzer8>,
        fuzz1009: impl Into<crate::testing::components::AffixFuzzer9>,
        fuzz1010: impl Into<crate::testing::components::AffixFuzzer10>,
        fuzz1011: impl Into<crate::testing::components::AffixFuzzer11>,
        fuzz1012: impl Into<crate::testing::components::AffixFuzzer12>,
        fuzz1013: impl Into<crate::testing::components::AffixFuzzer13>,
        fuzz1014: impl Into<crate::testing::components::AffixFuzzer14>,
        fuzz1015: impl Into<crate::testing::components::AffixFuzzer15>,
        fuzz1016: impl Into<crate::testing::components::AffixFuzzer16>,
        fuzz1017: impl Into<crate::testing::components::AffixFuzzer17>,
        fuzz1018: impl Into<crate::testing::components::AffixFuzzer18>,
        fuzz1019: impl Into<crate::testing::components::AffixFuzzer19>,
        fuzz1020: impl Into<crate::testing::components::AffixFuzzer20>,
        fuzz1021: impl Into<crate::testing::components::AffixFuzzer21>,
        fuzz1022: impl Into<crate::testing::components::AffixFuzzer22>,
    ) -> Self {
        Self {
            fuzz1001: fuzz1001.into(),
            fuzz1002: fuzz1002.into(),
            fuzz1003: fuzz1003.into(),
            fuzz1004: fuzz1004.into(),
            fuzz1005: fuzz1005.into(),
            fuzz1006: fuzz1006.into(),
            fuzz1007: fuzz1007.into(),
            fuzz1008: fuzz1008.into(),
            fuzz1009: fuzz1009.into(),
            fuzz1010: fuzz1010.into(),
            fuzz1011: fuzz1011.into(),
            fuzz1012: fuzz1012.into(),
            fuzz1013: fuzz1013.into(),
            fuzz1014: fuzz1014.into(),
            fuzz1015: fuzz1015.into(),
            fuzz1016: fuzz1016.into(),
            fuzz1017: fuzz1017.into(),
            fuzz1018: fuzz1018.into(),
            fuzz1019: fuzz1019.into(),
            fuzz1020: fuzz1020.into(),
            fuzz1021: fuzz1021.into(),
            fuzz1022: fuzz1022.into(),
        }
    }
}
