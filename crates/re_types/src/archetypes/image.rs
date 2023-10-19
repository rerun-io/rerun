// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/archetypes/image.fbs".

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

/// **Archetype**: A monochrome or color image.
///
/// The shape of the `TensorData` must be mappable to:
/// - A `HxW` tensor, treated as a grayscale image.
/// - A `HxWx3` tensor, treated as an RGB image.
/// - A `HxWx4` tensor, treated as an RGBA image.
///
/// Leading and trailing unit-dimensions are ignored, so that
/// `1x640x480x3x1` is treated as a `640x480x3` RGB image.
///
/// ## Example
///
/// ### `image_simple`:
/// ```ignore
/// use ndarray::{s, Array, ShapeBuilder};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (rec, storage) =
///         rerun::RecordingStreamBuilder::new("rerun_example_image_simple").memory()?;
///
///     let mut image = Array::<u8, _>::zeros((8, 12, 3).f());
///     image.slice_mut(s![.., .., 0]).fill(255);
///     image.slice_mut(s![0..4, 0..6, 0]).fill(0);
///     image.slice_mut(s![0..4, 0..6, 1]).fill(255);
///
///     rec.log("image", &rerun::Image::try_from(image)?)?;
///
///     rerun::native_viewer::show(storage.take())?;
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
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    /// The image data. Should always be a rank-2 or rank-3 tensor.
    pub data: crate::components::TensorData,

    /// An optional floating point value that specifies the 2D drawing order.
    ///
    /// Objects with higher values are drawn on top of those with lower values.
    pub draw_order: Option<crate::components::DrawOrder>,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.TensorData".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.ImageIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.DrawOrder".into(),
            "rerun.components.InstanceKey".into(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 4usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.TensorData".into(),
            "rerun.components.ImageIndicator".into(),
            "rerun.components.DrawOrder".into(),
            "rerun.components.InstanceKey".into(),
        ]
    });

impl Image {
    pub const NUM_COMPONENTS: usize = 4usize;
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
    fn from_arrow(
        arrow_data: impl IntoIterator<Item = (arrow2::datatypes::Field, Box<dyn arrow2::array::Array>)>,
    ) -> DeserializationResult<Self> {
        re_tracing::profile_function!();
        use ::re_types_core::{Loggable as _, ResultExt as _};
        let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
            .into_iter()
            .map(|(field, array)| (field.name, array))
            .collect();
        let data = {
            let array = arrays_by_name
                .get("rerun.components.TensorData")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.Image#data")?;
            <crate::components::TensorData>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.Image#data")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.Image#data")?
        };
        let draw_order = if let Some(array) = arrays_by_name.get("rerun.components.DrawOrder") {
            Some({
                <crate::components::DrawOrder>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.Image#draw_order")?
                    .into_iter()
                    .next()
                    .flatten()
                    .ok_or_else(DeserializationError::missing_data)
                    .with_context("rerun.archetypes.Image#draw_order")?
            })
        } else {
            None
        };
        Ok(Self { data, draw_order })
    }
}

impl ::re_types_core::AsComponents for Image {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.data as &dyn ComponentBatch).into()),
            self.draw_order
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    #[inline]
    fn num_instances(&self) -> usize {
        1
    }
}

impl Image {
    pub fn new(data: impl Into<crate::components::TensorData>) -> Self {
        Self {
            data: data.into(),
            draw_order: None,
        }
    }

    pub fn with_draw_order(mut self, draw_order: impl Into<crate::components::DrawOrder>) -> Self {
        self.draw_order = Some(draw_order.into());
        self
    }
}
