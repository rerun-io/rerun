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

use ::re_types_core::try_serialize_field;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch as _, SerializedComponentBatch};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Archetype**: A video binary.
///
/// Only MP4 containers with AV1 are generally supported,
/// though the web viewer supports more video codecs, depending on browser.
///
/// See <https://rerun.io/docs/reference/video> for details of what is and isn't supported.
///
/// In order to display a video, you also need to log a [`archetypes::VideoFrameReference`][crate::archetypes::VideoFrameReference] for each frame.
///
/// ## Examples
///
/// ### Video with automatically determined frames
/// ```ignore
/// use rerun::external::anyhow;
///
/// fn main() -> anyhow::Result<()> {
///     let args = _args;
///     let Some(path) = args.get(1) else {
///         // TODO(#7354): Only mp4 is supported for now.
///         anyhow::bail!("Usage: {} <path_to_video.[mp4]>", args[0]);
///     };
///
///     let rec =
///         rerun::RecordingStreamBuilder::new("rerun_example_asset_video_auto_frames").spawn()?;
///
///     // Log video asset which is referred to by frame references.
///     let video_asset = rerun::AssetVideo::from_file_path(path)?;
///     rec.log_static("video", &video_asset)?;
///
///     // Send automatically determined video frame timestamps.
///     let frame_timestamps_nanos = video_asset.read_frame_timestamps_nanos()?;
///     let video_timestamps_nanos = frame_timestamps_nanos
///         .iter()
///         .copied()
///         .map(rerun::components::VideoTimestamp::from_nanoseconds)
///         .collect::<Vec<_>>();
///     let time_column = rerun::TimeColumn::new_duration_nanos(
///         "video_time",
///         // Note timeline values don't have to be the same as the video timestamps.
///         frame_timestamps_nanos,
///     );
///
///     rec.send_columns(
///         "video",
///         [time_column],
///         rerun::VideoFrameReference::update_fields()
///             .with_many_timestamp(video_timestamps_nanos)
///             .columns_of_unit_batches()?,
///     )?;
///
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/video_manual_frames/320a44e1e06b8b3a3161ecbbeae3e04d1ccb9589/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/video_manual_frames/320a44e1e06b8b3a3161ecbbeae3e04d1ccb9589/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/video_manual_frames/320a44e1e06b8b3a3161ecbbeae3e04d1ccb9589/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/video_manual_frames/320a44e1e06b8b3a3161ecbbeae3e04d1ccb9589/1200w.png">
///   <img src="https://static.rerun.io/video_manual_frames/320a44e1e06b8b3a3161ecbbeae3e04d1ccb9589/full.png" width="640">
/// </picture>
/// </center>
///
/// ### Demonstrates manual use of video frame references
/// ```ignore
/// use rerun::external::anyhow;
///
/// fn main() -> anyhow::Result<()> {
///     let args = _args;
///     let Some(path) = args.get(1) else {
///         // TODO(#7354): Only mp4 is supported for now.
///         anyhow::bail!("Usage: {} <path_to_video.[mp4]>", args[0]);
///     };
///
///     let rec =
///         rerun::RecordingStreamBuilder::new("rerun_example_asset_video_manual_frames").spawn()?;
///
///     // Log video asset which is referred to by frame references.
///     rec.log_static("video_asset", &rerun::AssetVideo::from_file_path(path)?)?;
///
///     // Create two entities, showing the same video frozen at different times.
///     rec.log(
///         "frame_1s",
///         &rerun::VideoFrameReference::new(rerun::components::VideoTimestamp::from_secs(1.0))
///             .with_video_reference("video_asset"),
///     )?;
///     rec.log(
///         "frame_2s",
///         &rerun::VideoFrameReference::new(rerun::components::VideoTimestamp::from_secs(2.0))
///             .with_video_reference("video_asset"),
///     )?;
///
///     // TODO(#5520): log blueprint once supported
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/video_manual_frames/9f41c00f84a98cc3f26875fba7c1d2fa2bad7151/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/video_manual_frames/9f41c00f84a98cc3f26875fba7c1d2fa2bad7151/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/video_manual_frames/9f41c00f84a98cc3f26875fba7c1d2fa2bad7151/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/video_manual_frames/9f41c00f84a98cc3f26875fba7c1d2fa2bad7151/1200w.png">
///   <img src="https://static.rerun.io/video_manual_frames/9f41c00f84a98cc3f26875fba7c1d2fa2bad7151/full.png" width="640">
/// </picture>
/// </center>
#[derive(Clone, Debug, Default)]
pub struct AssetVideo {
    /// The asset's bytes.
    pub blob: Option<SerializedComponentBatch>,

    /// The Media Type of the asset.
    ///
    /// Supported values:
    /// * `video/mp4`
    ///
    /// If omitted, the viewer will try to guess from the data blob.
    /// If it cannot guess, it won't be able to render the asset.
    pub media_type: Option<SerializedComponentBatch>,
}

impl AssetVideo {
    /// Returns the [`ComponentDescriptor`] for [`Self::blob`].
    #[inline]
    pub fn descriptor_blob() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.AssetVideo".into()),
            component_name: "rerun.components.Blob".into(),
            archetype_field_name: Some("blob".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::media_type`].
    #[inline]
    pub fn descriptor_media_type() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.AssetVideo".into()),
            component_name: "rerun.components.MediaType".into(),
            archetype_field_name: Some("media_type".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for the associated indicator component.
    #[inline]
    pub fn descriptor_indicator() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.AssetVideo".into()),
            component_name: "rerun.components.AssetVideoIndicator".into(),
            archetype_field_name: None,
        }
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| [AssetVideo::descriptor_blob()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            AssetVideo::descriptor_media_type(),
            AssetVideo::descriptor_indicator(),
        ]
    });

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            AssetVideo::descriptor_blob(),
            AssetVideo::descriptor_media_type(),
            AssetVideo::descriptor_indicator(),
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
    fn indicator() -> SerializedComponentBatch {
        #[allow(clippy::unwrap_used)]
        AssetVideoIndicator::DEFAULT.serialized().unwrap()
    }

    #[inline]
    fn required_components() -> ::std::borrow::Cow<'static, [ComponentDescriptor]> {
        REQUIRED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn recommended_components() -> ::std::borrow::Cow<'static, [ComponentDescriptor]> {
        RECOMMENDED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn optional_components() -> ::std::borrow::Cow<'static, [ComponentDescriptor]> {
        OPTIONAL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn all_components() -> ::std::borrow::Cow<'static, [ComponentDescriptor]> {
        ALL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn from_arrow_components(
        arrow_data: impl IntoIterator<Item = (ComponentDescriptor, arrow::array::ArrayRef)>,
    ) -> DeserializationResult<Self> {
        re_tracing::profile_function!();
        use ::re_types_core::{Loggable as _, ResultExt as _};
        let arrays_by_descr: ::nohash_hasher::IntMap<_, _> = arrow_data.into_iter().collect();
        let blob = arrays_by_descr
            .get(&Self::descriptor_blob())
            .map(|array| SerializedComponentBatch::new(array.clone(), Self::descriptor_blob()));
        let media_type = arrays_by_descr
            .get(&Self::descriptor_media_type())
            .map(|array| {
                SerializedComponentBatch::new(array.clone(), Self::descriptor_media_type())
            });
        Ok(Self { blob, media_type })
    }
}

impl ::re_types_core::AsComponents for AssetVideo {
    #[inline]
    fn as_serialized_batches(&self) -> Vec<SerializedComponentBatch> {
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            self.blob.clone(),
            self.media_type.clone(),
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
            blob: try_serialize_field(Self::descriptor_blob(), [blob]),
            media_type: None,
        }
    }

    /// Update only some specific fields of a `AssetVideo`.
    #[inline]
    pub fn update_fields() -> Self {
        Self::default()
    }

    /// Clear all the fields of a `AssetVideo`.
    #[inline]
    pub fn clear_fields() -> Self {
        use ::re_types_core::Loggable as _;
        Self {
            blob: Some(SerializedComponentBatch::new(
                crate::components::Blob::arrow_empty(),
                Self::descriptor_blob(),
            )),
            media_type: Some(SerializedComponentBatch::new(
                crate::components::MediaType::arrow_empty(),
                Self::descriptor_media_type(),
            )),
        }
    }

    /// Partitions the component data into multiple sub-batches.
    ///
    /// Specifically, this transforms the existing [`SerializedComponentBatch`]es data into [`SerializedComponentColumn`]s
    /// instead, via [`SerializedComponentBatch::partitioned`].
    ///
    /// This makes it possible to use `RecordingStream::send_columns` to send columnar data directly into Rerun.
    ///
    /// The specified `lengths` must sum to the total length of the component batch.
    ///
    /// [`SerializedComponentColumn`]: [::re_types_core::SerializedComponentColumn]
    #[inline]
    pub fn columns<I>(
        self,
        _lengths: I,
    ) -> SerializationResult<impl Iterator<Item = ::re_types_core::SerializedComponentColumn>>
    where
        I: IntoIterator<Item = usize> + Clone,
    {
        let columns = [
            self.blob
                .map(|blob| blob.partitioned(_lengths.clone()))
                .transpose()?,
            self.media_type
                .map(|media_type| media_type.partitioned(_lengths.clone()))
                .transpose()?,
        ];
        Ok(columns
            .into_iter()
            .flatten()
            .chain([::re_types_core::indicator_column::<Self>(
                _lengths.into_iter().count(),
            )?]))
    }

    /// Helper to partition the component data into unit-length sub-batches.
    ///
    /// This is semantically similar to calling [`Self::columns`] with `std::iter::take(1).repeat(n)`,
    /// where `n` is automatically guessed.
    #[inline]
    pub fn columns_of_unit_batches(
        self,
    ) -> SerializationResult<impl Iterator<Item = ::re_types_core::SerializedComponentColumn>> {
        let len_blob = self.blob.as_ref().map(|b| b.array.len());
        let len_media_type = self.media_type.as_ref().map(|b| b.array.len());
        let len = None.or(len_blob).or(len_media_type).unwrap_or(0);
        self.columns(std::iter::repeat(1).take(len))
    }

    /// The asset's bytes.
    #[inline]
    pub fn with_blob(mut self, blob: impl Into<crate::components::Blob>) -> Self {
        self.blob = try_serialize_field(Self::descriptor_blob(), [blob]);
        self
    }

    /// This method makes it possible to pack multiple [`crate::components::Blob`] in a single component batch.
    ///
    /// This only makes sense when used in conjunction with [`Self::columns`]. [`Self::with_blob`] should
    /// be used when logging a single row's worth of data.
    #[inline]
    pub fn with_many_blob(
        mut self,
        blob: impl IntoIterator<Item = impl Into<crate::components::Blob>>,
    ) -> Self {
        self.blob = try_serialize_field(Self::descriptor_blob(), blob);
        self
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
        self.media_type = try_serialize_field(Self::descriptor_media_type(), [media_type]);
        self
    }

    /// This method makes it possible to pack multiple [`crate::components::MediaType`] in a single component batch.
    ///
    /// This only makes sense when used in conjunction with [`Self::columns`]. [`Self::with_media_type`] should
    /// be used when logging a single row's worth of data.
    #[inline]
    pub fn with_many_media_type(
        mut self,
        media_type: impl IntoIterator<Item = impl Into<crate::components::MediaType>>,
    ) -> Self {
        self.media_type = try_serialize_field(Self::descriptor_media_type(), media_type);
        self
    }
}

impl ::re_byte_size::SizeBytes for AssetVideo {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.blob.heap_size_bytes() + self.media_type.heap_size_bytes()
    }
}
