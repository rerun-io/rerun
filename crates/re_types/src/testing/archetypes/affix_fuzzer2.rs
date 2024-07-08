// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/testing/archetypes/fuzzy.fbs".

#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]
#![allow(clippy::map_flatten)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_lines)]

use ::re_types_core::external::arrow2;
use ::re_types_core::ComponentName;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{DeserializationError, DeserializationResult};

#[derive(Clone, Debug, PartialEq)]
pub struct AffixFuzzer2 {
    pub fuzz1101: Vec<crate::testing::components::AffixFuzzer1>,
    pub fuzz1102: Vec<crate::testing::components::AffixFuzzer2>,
    pub fuzz1103: Vec<crate::testing::components::AffixFuzzer3>,
    pub fuzz1104: Vec<crate::testing::components::AffixFuzzer4>,
    pub fuzz1105: Vec<crate::testing::components::AffixFuzzer5>,
    pub fuzz1106: Vec<crate::testing::components::AffixFuzzer6>,
    pub fuzz1107: Vec<crate::testing::components::AffixFuzzer7>,
    pub fuzz1108: Vec<crate::testing::components::AffixFuzzer8>,
    pub fuzz1109: Vec<crate::testing::components::AffixFuzzer9>,
    pub fuzz1110: Vec<crate::testing::components::AffixFuzzer10>,
    pub fuzz1111: Vec<crate::testing::components::AffixFuzzer11>,
    pub fuzz1112: Vec<crate::testing::components::AffixFuzzer12>,
    pub fuzz1113: Vec<crate::testing::components::AffixFuzzer13>,
    pub fuzz1114: Vec<crate::testing::components::AffixFuzzer14>,
    pub fuzz1115: Vec<crate::testing::components::AffixFuzzer15>,
    pub fuzz1116: Vec<crate::testing::components::AffixFuzzer16>,
    pub fuzz1117: Vec<crate::testing::components::AffixFuzzer17>,
    pub fuzz1118: Vec<crate::testing::components::AffixFuzzer18>,
    pub fuzz1122: Vec<crate::testing::components::AffixFuzzer22>,
}

impl ::re_types_core::SizeBytes for AffixFuzzer2 {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.fuzz1101.heap_size_bytes()
            + self.fuzz1102.heap_size_bytes()
            + self.fuzz1103.heap_size_bytes()
            + self.fuzz1104.heap_size_bytes()
            + self.fuzz1105.heap_size_bytes()
            + self.fuzz1106.heap_size_bytes()
            + self.fuzz1107.heap_size_bytes()
            + self.fuzz1108.heap_size_bytes()
            + self.fuzz1109.heap_size_bytes()
            + self.fuzz1110.heap_size_bytes()
            + self.fuzz1111.heap_size_bytes()
            + self.fuzz1112.heap_size_bytes()
            + self.fuzz1113.heap_size_bytes()
            + self.fuzz1114.heap_size_bytes()
            + self.fuzz1115.heap_size_bytes()
            + self.fuzz1116.heap_size_bytes()
            + self.fuzz1117.heap_size_bytes()
            + self.fuzz1118.heap_size_bytes()
            + self.fuzz1122.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Vec<crate::testing::components::AffixFuzzer1>>::is_pod()
            && <Vec<crate::testing::components::AffixFuzzer2>>::is_pod()
            && <Vec<crate::testing::components::AffixFuzzer3>>::is_pod()
            && <Vec<crate::testing::components::AffixFuzzer4>>::is_pod()
            && <Vec<crate::testing::components::AffixFuzzer5>>::is_pod()
            && <Vec<crate::testing::components::AffixFuzzer6>>::is_pod()
            && <Vec<crate::testing::components::AffixFuzzer7>>::is_pod()
            && <Vec<crate::testing::components::AffixFuzzer8>>::is_pod()
            && <Vec<crate::testing::components::AffixFuzzer9>>::is_pod()
            && <Vec<crate::testing::components::AffixFuzzer10>>::is_pod()
            && <Vec<crate::testing::components::AffixFuzzer11>>::is_pod()
            && <Vec<crate::testing::components::AffixFuzzer12>>::is_pod()
            && <Vec<crate::testing::components::AffixFuzzer13>>::is_pod()
            && <Vec<crate::testing::components::AffixFuzzer14>>::is_pod()
            && <Vec<crate::testing::components::AffixFuzzer15>>::is_pod()
            && <Vec<crate::testing::components::AffixFuzzer16>>::is_pod()
            && <Vec<crate::testing::components::AffixFuzzer17>>::is_pod()
            && <Vec<crate::testing::components::AffixFuzzer18>>::is_pod()
            && <Vec<crate::testing::components::AffixFuzzer22>>::is_pod()
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 19usize]> =
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
            "rerun.testing.components.AffixFuzzer22".into(),
        ]
    });

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.testing.components.AffixFuzzer2Indicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 20usize]> =
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
            "rerun.testing.components.AffixFuzzer22".into(),
            "rerun.testing.components.AffixFuzzer2Indicator".into(),
        ]
    });

impl AffixFuzzer2 {
    /// The total number of components in the archetype: 19 required, 1 recommended, 0 optional
    pub const NUM_COMPONENTS: usize = 20usize;
}

/// Indicator component for the [`AffixFuzzer2`] [`::re_types_core::Archetype`]
pub type AffixFuzzer2Indicator = ::re_types_core::GenericIndicatorComponent<AffixFuzzer2>;

impl ::re_types_core::Archetype for AffixFuzzer2 {
    type Indicator = AffixFuzzer2Indicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.testing.archetypes.AffixFuzzer2".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Affix fuzzer 2"
    }

    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        static INDICATOR: AffixFuzzer2Indicator = AffixFuzzer2Indicator::DEFAULT;
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
        let fuzz1101 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer1")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1101")?;
            <crate::testing::components::AffixFuzzer1>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1101")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1101")?
        };
        let fuzz1102 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer2")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1102")?;
            <crate::testing::components::AffixFuzzer2>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1102")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1102")?
        };
        let fuzz1103 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer3")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1103")?;
            <crate::testing::components::AffixFuzzer3>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1103")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1103")?
        };
        let fuzz1104 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer4")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1104")?;
            <crate::testing::components::AffixFuzzer4>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1104")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1104")?
        };
        let fuzz1105 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer5")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1105")?;
            <crate::testing::components::AffixFuzzer5>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1105")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1105")?
        };
        let fuzz1106 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer6")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1106")?;
            <crate::testing::components::AffixFuzzer6>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1106")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1106")?
        };
        let fuzz1107 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer7")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1107")?;
            <crate::testing::components::AffixFuzzer7>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1107")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1107")?
        };
        let fuzz1108 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer8")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1108")?;
            <crate::testing::components::AffixFuzzer8>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1108")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1108")?
        };
        let fuzz1109 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer9")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1109")?;
            <crate::testing::components::AffixFuzzer9>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1109")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1109")?
        };
        let fuzz1110 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer10")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1110")?;
            <crate::testing::components::AffixFuzzer10>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1110")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1110")?
        };
        let fuzz1111 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer11")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1111")?;
            <crate::testing::components::AffixFuzzer11>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1111")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1111")?
        };
        let fuzz1112 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer12")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1112")?;
            <crate::testing::components::AffixFuzzer12>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1112")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1112")?
        };
        let fuzz1113 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer13")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1113")?;
            <crate::testing::components::AffixFuzzer13>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1113")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1113")?
        };
        let fuzz1114 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer14")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1114")?;
            <crate::testing::components::AffixFuzzer14>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1114")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1114")?
        };
        let fuzz1115 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer15")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1115")?;
            <crate::testing::components::AffixFuzzer15>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1115")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1115")?
        };
        let fuzz1116 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer16")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1116")?;
            <crate::testing::components::AffixFuzzer16>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1116")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1116")?
        };
        let fuzz1117 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer17")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1117")?;
            <crate::testing::components::AffixFuzzer17>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1117")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1117")?
        };
        let fuzz1118 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer18")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1118")?;
            <crate::testing::components::AffixFuzzer18>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1118")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1118")?
        };
        let fuzz1122 = {
            let array = arrays_by_name
                .get("rerun.testing.components.AffixFuzzer22")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1122")?;
            <crate::testing::components::AffixFuzzer22>::from_arrow_opt(&**array)
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1122")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.testing.archetypes.AffixFuzzer2#fuzz1122")?
        };
        Ok(Self {
            fuzz1101,
            fuzz1102,
            fuzz1103,
            fuzz1104,
            fuzz1105,
            fuzz1106,
            fuzz1107,
            fuzz1108,
            fuzz1109,
            fuzz1110,
            fuzz1111,
            fuzz1112,
            fuzz1113,
            fuzz1114,
            fuzz1115,
            fuzz1116,
            fuzz1117,
            fuzz1118,
            fuzz1122,
        })
    }
}

impl ::re_types_core::AsComponents for AffixFuzzer2 {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.fuzz1101 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1102 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1103 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1104 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1105 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1106 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1107 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1108 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1109 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1110 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1111 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1112 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1113 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1114 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1115 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1116 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1117 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1118 as &dyn ComponentBatch).into()),
            Some((&self.fuzz1122 as &dyn ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl AffixFuzzer2 {
    /// Create a new `AffixFuzzer2`.
    #[inline]
    pub fn new(
        fuzz1101: impl IntoIterator<Item = impl Into<crate::testing::components::AffixFuzzer1>>,
        fuzz1102: impl IntoIterator<Item = impl Into<crate::testing::components::AffixFuzzer2>>,
        fuzz1103: impl IntoIterator<Item = impl Into<crate::testing::components::AffixFuzzer3>>,
        fuzz1104: impl IntoIterator<Item = impl Into<crate::testing::components::AffixFuzzer4>>,
        fuzz1105: impl IntoIterator<Item = impl Into<crate::testing::components::AffixFuzzer5>>,
        fuzz1106: impl IntoIterator<Item = impl Into<crate::testing::components::AffixFuzzer6>>,
        fuzz1107: impl IntoIterator<Item = impl Into<crate::testing::components::AffixFuzzer7>>,
        fuzz1108: impl IntoIterator<Item = impl Into<crate::testing::components::AffixFuzzer8>>,
        fuzz1109: impl IntoIterator<Item = impl Into<crate::testing::components::AffixFuzzer9>>,
        fuzz1110: impl IntoIterator<Item = impl Into<crate::testing::components::AffixFuzzer10>>,
        fuzz1111: impl IntoIterator<Item = impl Into<crate::testing::components::AffixFuzzer11>>,
        fuzz1112: impl IntoIterator<Item = impl Into<crate::testing::components::AffixFuzzer12>>,
        fuzz1113: impl IntoIterator<Item = impl Into<crate::testing::components::AffixFuzzer13>>,
        fuzz1114: impl IntoIterator<Item = impl Into<crate::testing::components::AffixFuzzer14>>,
        fuzz1115: impl IntoIterator<Item = impl Into<crate::testing::components::AffixFuzzer15>>,
        fuzz1116: impl IntoIterator<Item = impl Into<crate::testing::components::AffixFuzzer16>>,
        fuzz1117: impl IntoIterator<Item = impl Into<crate::testing::components::AffixFuzzer17>>,
        fuzz1118: impl IntoIterator<Item = impl Into<crate::testing::components::AffixFuzzer18>>,
        fuzz1122: impl IntoIterator<Item = impl Into<crate::testing::components::AffixFuzzer22>>,
    ) -> Self {
        Self {
            fuzz1101: fuzz1101.into_iter().map(Into::into).collect(),
            fuzz1102: fuzz1102.into_iter().map(Into::into).collect(),
            fuzz1103: fuzz1103.into_iter().map(Into::into).collect(),
            fuzz1104: fuzz1104.into_iter().map(Into::into).collect(),
            fuzz1105: fuzz1105.into_iter().map(Into::into).collect(),
            fuzz1106: fuzz1106.into_iter().map(Into::into).collect(),
            fuzz1107: fuzz1107.into_iter().map(Into::into).collect(),
            fuzz1108: fuzz1108.into_iter().map(Into::into).collect(),
            fuzz1109: fuzz1109.into_iter().map(Into::into).collect(),
            fuzz1110: fuzz1110.into_iter().map(Into::into).collect(),
            fuzz1111: fuzz1111.into_iter().map(Into::into).collect(),
            fuzz1112: fuzz1112.into_iter().map(Into::into).collect(),
            fuzz1113: fuzz1113.into_iter().map(Into::into).collect(),
            fuzz1114: fuzz1114.into_iter().map(Into::into).collect(),
            fuzz1115: fuzz1115.into_iter().map(Into::into).collect(),
            fuzz1116: fuzz1116.into_iter().map(Into::into).collect(),
            fuzz1117: fuzz1117.into_iter().map(Into::into).collect(),
            fuzz1118: fuzz1118.into_iter().map(Into::into).collect(),
            fuzz1122: fuzz1122.into_iter().map(Into::into).collect(),
        }
    }
}
