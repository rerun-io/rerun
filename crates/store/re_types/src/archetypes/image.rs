// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/image.fbs".

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

/// **Archetype**: A monochrome or color image.
///
/// See also [`archetypes::DepthImage`][crate::archetypes::DepthImage] and [`archetypes::SegmentationImage`][crate::archetypes::SegmentationImage].
///
/// Rerun also supports compressed images (JPEG, PNG, …), using [`archetypes::EncodedImage`][crate::archetypes::EncodedImage].
/// For images that refer to video frames see [`archetypes::VideoFrameReference`][crate::archetypes::VideoFrameReference].
/// Compressing images or using video data instead can save a lot of bandwidth and memory.
///
/// The raw image data is stored as a single buffer of bytes in a [`components::Blob`][crate::components::Blob].
/// The meaning of these bytes is determined by the [`components::ImageFormat`][crate::components::ImageFormat] which specifies the resolution
/// and the pixel format (e.g. RGB, RGBA, …).
///
/// The order of dimensions in the underlying [`components::Blob`][crate::components::Blob] follows the typical
/// row-major, interleaved-pixel image format.
///
/// ## Examples
///
/// ### `image_simple`:
/// ```ignore
/// use ndarray::{s, Array, ShapeBuilder};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let rec = rerun::RecordingStreamBuilder::new("rerun_example_image").spawn()?;
///
///     let mut image = Array::<u8, _>::zeros((200, 300, 3).f());
///     image.slice_mut(s![.., .., 0]).fill(255);
///     image.slice_mut(s![50..150, 50..150, 0]).fill(0);
///     image.slice_mut(s![50..150, 50..150, 1]).fill(255);
///
///     rec.log(
///         "image",
///         &rerun::Image::from_color_model_and_tensor(rerun::ColorModel::RGB, image)?,
///     )?;
///
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/1200w.png">
///   <img src="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/full.png" width="640">
/// </picture>
/// </center>
///
/// ### Logging images with various formats
/// ```ignore
/// use rerun::external::ndarray;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let rec = rerun::RecordingStreamBuilder::new("rerun_example_image_formats").spawn()?;
///
///     // Simple gradient image
///     let image = ndarray::Array3::from_shape_fn((256, 256, 3), |(y, x, c)| match c {
///         0 => x as u8,
///         1 => (x + y).min(255) as u8,
///         2 => y as u8,
///         _ => unreachable!(),
///     });
///
///     // RGB image
///     rec.log(
///         "image_rgb",
///         &rerun::Image::from_color_model_and_tensor(rerun::ColorModel::RGB, image.clone())?,
///     )?;
///
///     // Green channel only (Luminance)
///     rec.log(
///         "image_green_only",
///         &rerun::Image::from_color_model_and_tensor(
///             rerun::ColorModel::L,
///             image.slice(ndarray::s![.., .., 1]).to_owned(),
///         )?,
///     )?;
///
///     // BGR image
///     rec.log(
///         "image_bgr",
///         &rerun::Image::from_color_model_and_tensor(
///             rerun::ColorModel::BGR,
///             image.slice(ndarray::s![.., .., ..;-1]).to_owned(),
///         )?,
///     )?;
///
///     // New image with Separate Y/U/V planes with 4:2:2 chroma downsampling
///     let mut yuv_bytes = Vec::with_capacity(256 * 256 + 128 * 256 * 2);
///     yuv_bytes.extend(std::iter::repeat(128).take(256 * 256)); // Fixed value for Y.
///     yuv_bytes.extend((0..256).flat_map(|_y| (0..128).map(|x| x * 2))); // Gradient for U.
///     yuv_bytes.extend((0..256).flat_map(|y| std::iter::repeat(y as u8).take(128))); // Gradient for V.
///     rec.log(
///         "image_yuv422",
///         &rerun::Image::from_pixel_format(
///             [256, 256],
///             rerun::PixelFormat::Y_U_V16_FullRange,
///             yuv_bytes,
///         ),
///     )?;
///
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <img src="https://static.rerun.io/image_formats/7b8a162fcfd266f303980439beea997dc8544c24/full.png" width="640">
/// </picture>
/// </center>
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    /// The raw image data.
    pub buffer: crate::components::ImageBuffer,

    /// The format of the image.
    pub format: crate::components::ImageFormat,

    /// Opacity of the image, useful for layering several images.
    ///
    /// Defaults to 1.0 (fully opaque).
    pub opacity: Option<crate::components::Opacity>,

    /// An optional floating point value that specifies the 2D drawing order.
    ///
    /// Objects with higher values are drawn on top of those with lower values.
    pub draw_order: Option<crate::components::DrawOrder>,
}

impl ::re_types_core::SizeBytes for Image {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.buffer.heap_size_bytes()
            + self.format.heap_size_bytes()
            + self.opacity.heap_size_bytes()
            + self.draw_order.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::components::ImageBuffer>::is_pod()
            && <crate::components::ImageFormat>::is_pod()
            && <Option<crate::components::Opacity>>::is_pod()
            && <Option<crate::components::DrawOrder>>::is_pod()
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.ImageBuffer".into(),
            "rerun.components.ImageFormat".into(),
        ]
    });

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.ImageIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.Opacity".into(),
            "rerun.components.DrawOrder".into(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 5usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.ImageBuffer".into(),
            "rerun.components.ImageFormat".into(),
            "rerun.components.ImageIndicator".into(),
            "rerun.components.Opacity".into(),
            "rerun.components.DrawOrder".into(),
        ]
    });

impl Image {
    /// The total number of components in the archetype: 2 required, 1 recommended, 2 optional
    pub const NUM_COMPONENTS: usize = 5usize;
}

/// Indicator component for the [`Image`] [`::re_types_core::Archetype`]
pub type ImageIndicator = ::re_types_core::GenericIndicatorComponent<Image>;

impl ::re_types_core::Archetype for Image {
    type Indicator = ImageIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.archetypes.Image".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Image"
    }

    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        static INDICATOR: ImageIndicator = ImageIndicator::DEFAULT;
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
    fn from_arrow2_components(
        arrow_data: impl IntoIterator<Item = (ComponentName, Box<dyn arrow2::array::Array>)>,
    ) -> DeserializationResult<Self> {
        re_tracing::profile_function!();
        use ::re_types_core::{Loggable as _, ResultExt as _};
        let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
            .into_iter()
            .map(|(name, array)| (name.full_name(), array))
            .collect();
        let buffer = {
            let array = arrays_by_name
                .get("rerun.components.ImageBuffer")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.Image#buffer")?;
            <crate::components::ImageBuffer>::from_arrow2_opt(&**array)
                .with_context("rerun.archetypes.Image#buffer")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.Image#buffer")?
        };
        let format = {
            let array = arrays_by_name
                .get("rerun.components.ImageFormat")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.Image#format")?;
            <crate::components::ImageFormat>::from_arrow2_opt(&**array)
                .with_context("rerun.archetypes.Image#format")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.Image#format")?
        };
        let opacity = if let Some(array) = arrays_by_name.get("rerun.components.Opacity") {
            <crate::components::Opacity>::from_arrow2_opt(&**array)
                .with_context("rerun.archetypes.Image#opacity")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let draw_order = if let Some(array) = arrays_by_name.get("rerun.components.DrawOrder") {
            <crate::components::DrawOrder>::from_arrow2_opt(&**array)
                .with_context("rerun.archetypes.Image#draw_order")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        Ok(Self {
            buffer,
            format,
            opacity,
            draw_order,
        })
    }
}

impl ::re_types_core::AsComponents for Image {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.buffer as &dyn ComponentBatch).into()),
            Some((&self.format as &dyn ComponentBatch).into()),
            self.opacity
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
            self.draw_order
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for Image {}

impl Image {
    /// Create a new `Image`.
    #[inline]
    pub fn new(
        buffer: impl Into<crate::components::ImageBuffer>,
        format: impl Into<crate::components::ImageFormat>,
    ) -> Self {
        Self {
            buffer: buffer.into(),
            format: format.into(),
            opacity: None,
            draw_order: None,
        }
    }

    /// Opacity of the image, useful for layering several images.
    ///
    /// Defaults to 1.0 (fully opaque).
    #[inline]
    pub fn with_opacity(mut self, opacity: impl Into<crate::components::Opacity>) -> Self {
        self.opacity = Some(opacity.into());
        self
    }

    /// An optional floating point value that specifies the 2D drawing order.
    ///
    /// Objects with higher values are drawn on top of those with lower values.
    #[inline]
    pub fn with_draw_order(mut self, draw_order: impl Into<crate::components::DrawOrder>) -> Self {
        self.draw_order = Some(draw_order.into());
        self
    }
}
