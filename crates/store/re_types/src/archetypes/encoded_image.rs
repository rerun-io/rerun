// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/encoded_image.fbs".

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
use ::re_types_core::{ComponentBatch, ComponentBatchCowWithDescriptor, SerializedComponentBatch};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Archetype**: An image encoded as e.g. a JPEG or PNG.
///
/// Rerun also supports uncompressed images with the [`archetypes::Image`][crate::archetypes::Image].
/// For images that refer to video frames see [`archetypes::VideoFrameReference`][crate::archetypes::VideoFrameReference].
///
/// ## Example
///
/// ### `encoded_image`:
/// ```ignore
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let rec = rerun::RecordingStreamBuilder::new("rerun_example_encoded_image").spawn()?;
///
///     let image = include_bytes!("ferris.png");
///
///     rec.log(
///         "image",
///         &rerun::EncodedImage::from_file_contents(image.to_vec()),
///     )?;
///
///     Ok(())
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Default)]
pub struct EncodedImage {
    /// The encoded content of some image file, e.g. a PNG or JPEG.
    pub blob: Option<SerializedComponentBatch>,

    /// The Media Type of the asset.
    ///
    /// Supported values:
    /// * `image/jpeg`
    /// * `image/png`
    ///
    /// If omitted, the viewer will try to guess from the data blob.
    /// If it cannot guess, it won't be able to render the asset.
    pub media_type: Option<SerializedComponentBatch>,

    /// Opacity of the image, useful for layering several images.
    ///
    /// Defaults to 1.0 (fully opaque).
    pub opacity: Option<SerializedComponentBatch>,

    /// An optional floating point value that specifies the 2D drawing order.
    ///
    /// Objects with higher values are drawn on top of those with lower values.
    pub draw_order: Option<SerializedComponentBatch>,
}

impl EncodedImage {
    /// Returns the [`ComponentDescriptor`] for [`Self::blob`].
    #[inline]
    pub fn descriptor_blob() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.EncodedImage".into()),
            component_name: "rerun.components.Blob".into(),
            archetype_field_name: Some("blob".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::media_type`].
    #[inline]
    pub fn descriptor_media_type() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.EncodedImage".into()),
            component_name: "rerun.components.MediaType".into(),
            archetype_field_name: Some("media_type".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::opacity`].
    #[inline]
    pub fn descriptor_opacity() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.EncodedImage".into()),
            component_name: "rerun.components.Opacity".into(),
            archetype_field_name: Some("opacity".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::draw_order`].
    #[inline]
    pub fn descriptor_draw_order() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.EncodedImage".into()),
            component_name: "rerun.components.DrawOrder".into(),
            archetype_field_name: Some("draw_order".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for the associated indicator component.
    #[inline]
    pub fn descriptor_indicator() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.EncodedImage".into()),
            component_name: "rerun.components.EncodedImageIndicator".into(),
            archetype_field_name: None,
        }
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| [EncodedImage::descriptor_blob()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            EncodedImage::descriptor_media_type(),
            EncodedImage::descriptor_indicator(),
        ]
    });

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            EncodedImage::descriptor_opacity(),
            EncodedImage::descriptor_draw_order(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 5usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            EncodedImage::descriptor_blob(),
            EncodedImage::descriptor_media_type(),
            EncodedImage::descriptor_indicator(),
            EncodedImage::descriptor_opacity(),
            EncodedImage::descriptor_draw_order(),
        ]
    });

impl EncodedImage {
    /// The total number of components in the archetype: 1 required, 2 recommended, 2 optional
    pub const NUM_COMPONENTS: usize = 5usize;
}

/// Indicator component for the [`EncodedImage`] [`::re_types_core::Archetype`]
pub type EncodedImageIndicator = ::re_types_core::GenericIndicatorComponent<EncodedImage>;

impl ::re_types_core::Archetype for EncodedImage {
    type Indicator = EncodedImageIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.archetypes.EncodedImage".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Encoded image"
    }

    #[inline]
    fn indicator() -> ComponentBatchCowWithDescriptor<'static> {
        static INDICATOR: EncodedImageIndicator = EncodedImageIndicator::DEFAULT;
        ComponentBatchCowWithDescriptor::new(&INDICATOR as &dyn ::re_types_core::ComponentBatch)
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
        let opacity = arrays_by_descr
            .get(&Self::descriptor_opacity())
            .map(|array| SerializedComponentBatch::new(array.clone(), Self::descriptor_opacity()));
        let draw_order = arrays_by_descr
            .get(&Self::descriptor_draw_order())
            .map(|array| {
                SerializedComponentBatch::new(array.clone(), Self::descriptor_draw_order())
            });
        Ok(Self {
            blob,
            media_type,
            opacity,
            draw_order,
        })
    }
}

impl ::re_types_core::AsComponents for EncodedImage {
    #[inline]
    fn as_serialized_batches(&self) -> Vec<SerializedComponentBatch> {
        use ::re_types_core::Archetype as _;
        [
            Self::indicator().serialized(),
            self.blob.clone(),
            self.media_type.clone(),
            self.opacity.clone(),
            self.draw_order.clone(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for EncodedImage {}

impl EncodedImage {
    /// Create a new `EncodedImage`.
    #[inline]
    pub fn new(blob: impl Into<crate::components::Blob>) -> Self {
        Self {
            blob: try_serialize_field(Self::descriptor_blob(), [blob]),
            media_type: None,
            opacity: None,
            draw_order: None,
        }
    }

    /// Update only some specific fields of a `EncodedImage`.
    #[inline]
    pub fn update_fields() -> Self {
        Self::default()
    }

    /// Clear all the fields of a `EncodedImage`.
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
            opacity: Some(SerializedComponentBatch::new(
                crate::components::Opacity::arrow_empty(),
                Self::descriptor_opacity(),
            )),
            draw_order: Some(SerializedComponentBatch::new(
                crate::components::DrawOrder::arrow_empty(),
                Self::descriptor_draw_order(),
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
            self.opacity
                .map(|opacity| opacity.partitioned(_lengths.clone()))
                .transpose()?,
            self.draw_order
                .map(|draw_order| draw_order.partitioned(_lengths.clone()))
                .transpose()?,
        ];
        let indicator_column =
            ::re_types_core::indicator_column::<Self>(_lengths.into_iter().count())?;
        Ok(columns.into_iter().chain([indicator_column]).flatten())
    }

    /// Helper to partition the component data into unit-length sub-batches.
    ///
    /// This is semantically similar to calling [`Self::columns`] with `std::iter::take(1).repeat(n)`,
    /// where `n` is automatically guessed.
    #[inline]
    pub fn unary_columns(
        self,
    ) -> SerializationResult<impl Iterator<Item = ::re_types_core::SerializedComponentColumn>> {
        let len_blob = self.blob.as_ref().map(|b| b.array.len());
        let len_media_type = self.media_type.as_ref().map(|b| b.array.len());
        let len_opacity = self.opacity.as_ref().map(|b| b.array.len());
        let len_draw_order = self.draw_order.as_ref().map(|b| b.array.len());
        let len = None
            .or(len_blob)
            .or(len_media_type)
            .or(len_opacity)
            .or(len_draw_order)
            .unwrap_or(0);
        self.columns(std::iter::repeat(1).take(len))
    }

    /// The encoded content of some image file, e.g. a PNG or JPEG.
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
    /// * `image/jpeg`
    /// * `image/png`
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

    /// Opacity of the image, useful for layering several images.
    ///
    /// Defaults to 1.0 (fully opaque).
    #[inline]
    pub fn with_opacity(mut self, opacity: impl Into<crate::components::Opacity>) -> Self {
        self.opacity = try_serialize_field(Self::descriptor_opacity(), [opacity]);
        self
    }

    /// This method makes it possible to pack multiple [`crate::components::Opacity`] in a single component batch.
    ///
    /// This only makes sense when used in conjunction with [`Self::columns`]. [`Self::with_opacity`] should
    /// be used when logging a single row's worth of data.
    #[inline]
    pub fn with_many_opacity(
        mut self,
        opacity: impl IntoIterator<Item = impl Into<crate::components::Opacity>>,
    ) -> Self {
        self.opacity = try_serialize_field(Self::descriptor_opacity(), opacity);
        self
    }

    /// An optional floating point value that specifies the 2D drawing order.
    ///
    /// Objects with higher values are drawn on top of those with lower values.
    #[inline]
    pub fn with_draw_order(mut self, draw_order: impl Into<crate::components::DrawOrder>) -> Self {
        self.draw_order = try_serialize_field(Self::descriptor_draw_order(), [draw_order]);
        self
    }

    /// This method makes it possible to pack multiple [`crate::components::DrawOrder`] in a single component batch.
    ///
    /// This only makes sense when used in conjunction with [`Self::columns`]. [`Self::with_draw_order`] should
    /// be used when logging a single row's worth of data.
    #[inline]
    pub fn with_many_draw_order(
        mut self,
        draw_order: impl IntoIterator<Item = impl Into<crate::components::DrawOrder>>,
    ) -> Self {
        self.draw_order = try_serialize_field(Self::descriptor_draw_order(), draw_order);
        self
    }
}

impl ::re_byte_size::SizeBytes for EncodedImage {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.blob.heap_size_bytes()
            + self.media_type.heap_size_bytes()
            + self.opacity.heap_size_bytes()
            + self.draw_order.heap_size_bytes()
    }
}
