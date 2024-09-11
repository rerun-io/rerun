// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/asset_video.fbs".

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

/// **Archetype**: A video binary.
///
/// NOTE: Videos can only be viewed in the Rerun web viewer.
/// Only MP4 containers with a limited number of codecs are currently supported, and not in all browsers.
/// Follow <https://github.com/rerun-io/rerun/issues/7298> for updates on the native support.
///
/// In order to display a video, you need to log a [`archetypes::VideoFrameReference`][crate::archetypes::VideoFrameReference] for each frame.
///
/// ⚠️ **This type is experimental and may be removed in future versions**
#[derive(Clone, Debug)]
pub struct AssetVideo {
    /// The asset's bytes.
    pub blob: crate::components::Blob,

    /// The Media Type of the asset.
    ///
    /// Supported values:
    /// * `video/mp4`
    ///
    /// If omitted, the viewer will try to guess from the data blob.
    /// If it cannot guess, it won't be able to render the asset.
    pub media_type: Option<crate::components::MediaType>,
}

impl ::re_types_core::SizeBytes for AssetVideo {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.blob.heap_size_bytes() + self.media_type.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::components::Blob>::is_pod() && <Option<crate::components::MediaType>>::is_pod()
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.Blob".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.MediaType".into(),
            "rerun.components.AssetVideoIndicator".into(),
        ]
    });

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.Blob".into(),
            "rerun.components.MediaType".into(),
            "rerun.components.AssetVideoIndicator".into(),
        ]
    });

impl AssetVideo {
    /// The total number of components in the archetype: 1 required, 2 recommended, 0 optional
    pub const NUM_COMPONENTS: usize = 3usize;
}

/// Indicator component for the [`AssetVideo`] [`::re_types_core::Archetype`]
pub type AssetVideoIndicator = ::re_types_core::GenericIndicatorComponent<AssetVideo>;

impl ::re_types_core::Archetype for AssetVideo {
    type Indicator = AssetVideoIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.archetypes.AssetVideo".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Asset video"
    }

    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        static INDICATOR: AssetVideoIndicator = AssetVideoIndicator::DEFAULT;
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
        let blob = {
            let array = arrays_by_name
                .get("rerun.components.Blob")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.AssetVideo#blob")?;
            <crate::components::Blob>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.AssetVideo#blob")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.AssetVideo#blob")?
        };
        let media_type = if let Some(array) = arrays_by_name.get("rerun.components.MediaType") {
            <crate::components::MediaType>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.AssetVideo#media_type")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        Ok(Self { blob, media_type })
    }
}

impl ::re_types_core::AsComponents for AssetVideo {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.blob as &dyn ComponentBatch).into()),
            self.media_type
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for AssetVideo {}

impl AssetVideo {
    /// Create a new `AssetVideo`.
    #[inline]
    pub fn new(blob: impl Into<crate::components::Blob>) -> Self {
        Self {
            blob: blob.into(),
            media_type: None,
        }
    }

    /// The Media Type of the asset.
    ///
    /// Supported values:
    /// * `video/mp4`
    ///
    /// If omitted, the viewer will try to guess from the data blob.
    /// If it cannot guess, it won't be able to render the asset.
    #[inline]
    pub fn with_media_type(mut self, media_type: impl Into<crate::components::MediaType>) -> Self {
        self.media_type = Some(media_type.into());
        self
    }
}
