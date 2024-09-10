// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/video_frame_reference.fbs".

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

/// **Archetype**: References a single video frame.
///
/// Used to display video frames from a [`archetypes::AssetVideo`][crate::archetypes::AssetVideo].
///
/// ⚠️ **This type is experimental and may be removed in future versions**
#[derive(Clone, Debug)]
pub struct VideoFrameReference {
    /// References the closest video frame to this time.
    ///
    /// Note that this uses the closest video frame instead of the latest at this timestamp
    /// in order to be more forgiving of rounding errors.
    pub timestamp: crate::components::VideoTimestamp,

    /// Optional reference to an entity with a [`archetypes::AssetVideo`][crate::archetypes::AssetVideo].
    ///
    /// If none is specified, the video is assumed to be at the same entity.
    /// Note that blueprint overrides on the referenced video will be ignored regardless,
    /// as this is always interpreted as a reference to the data store.
    pub video_reference: Option<crate::components::EntityPath>,
}

impl ::re_types_core::SizeBytes for VideoFrameReference {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.timestamp.heap_size_bytes() + self.video_reference.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::components::VideoTimestamp>::is_pod()
            && <Option<crate::components::EntityPath>>::is_pod()
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.VideoTimestamp".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.VideoFrameReferenceIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.EntityPath".into()]);

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.VideoTimestamp".into(),
            "rerun.components.VideoFrameReferenceIndicator".into(),
            "rerun.components.EntityPath".into(),
        ]
    });

impl VideoFrameReference {
    /// The total number of components in the archetype: 1 required, 1 recommended, 1 optional
    pub const NUM_COMPONENTS: usize = 3usize;
}

/// Indicator component for the [`VideoFrameReference`] [`::re_types_core::Archetype`]
pub type VideoFrameReferenceIndicator =
    ::re_types_core::GenericIndicatorComponent<VideoFrameReference>;

impl ::re_types_core::Archetype for VideoFrameReference {
    type Indicator = VideoFrameReferenceIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.archetypes.VideoFrameReference".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Video frame reference"
    }

    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        static INDICATOR: VideoFrameReferenceIndicator = VideoFrameReferenceIndicator::DEFAULT;
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
        let timestamp = {
            let array = arrays_by_name
                .get("rerun.components.VideoTimestamp")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.VideoFrameReference#timestamp")?;
            <crate::components::VideoTimestamp>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.VideoFrameReference#timestamp")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.VideoFrameReference#timestamp")?
        };
        let video_reference = if let Some(array) = arrays_by_name.get("rerun.components.EntityPath")
        {
            <crate::components::EntityPath>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.VideoFrameReference#video_reference")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        Ok(Self {
            timestamp,
            video_reference,
        })
    }
}

impl ::re_types_core::AsComponents for VideoFrameReference {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.timestamp as &dyn ComponentBatch).into()),
            self.video_reference
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for VideoFrameReference {}

impl VideoFrameReference {
    /// Create a new `VideoFrameReference`.
    #[inline]
    pub fn new(timestamp: impl Into<crate::components::VideoTimestamp>) -> Self {
        Self {
            timestamp: timestamp.into(),
            video_reference: None,
        }
    }

    /// Optional reference to an entity with a [`archetypes::AssetVideo`][crate::archetypes::AssetVideo].
    ///
    /// If none is specified, the video is assumed to be at the same entity.
    /// Note that blueprint overrides on the referenced video will be ignored regardless,
    /// as this is always interpreted as a reference to the data store.
    #[inline]
    pub fn with_video_reference(
        mut self,
        video_reference: impl Into<crate::components::EntityPath>,
    ) -> Self {
        self.video_reference = Some(video_reference.into());
        self
    }
}
