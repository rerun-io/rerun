// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/archetypes/annotation_context.fbs".

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

/// **Archetype**: The `AnnotationContext` provides additional information on how to display entities.
///
/// Entities can use `ClassId`s and `KeypointId`s to provide annotations, and
/// the labels and colors will be looked up in the appropriate
/// `AnnotationContext`. We use the *first* annotation context we find in the
/// path-hierarchy when searching up through the ancestors of a given entity
/// path.
///
/// See also [`ClassDescription`][crate::datatypes::ClassDescription].
///
/// ## Example
///
/// ### Segmentation
/// ```ignore
/// use ndarray::{s, Array, ShapeBuilder};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (rec, storage) =
///         rerun::RecordingStreamBuilder::new("rerun_example_annotation_context_segmentation")
///             .memory()?;
///
///     // create an annotation context to describe the classes
///     rec.log_timeless(
///         "segmentation",
///         &rerun::AnnotationContext::new([
///             (1, "red", rerun::Rgba32::from_rgb(255, 0, 0)),
///             (2, "green", rerun::Rgba32::from_rgb(0, 255, 0)),
///         ]),
///     )?;
///
///     // create a segmentation image
///     let mut data = Array::<u8, _>::zeros((8, 12).f());
///     data.slice_mut(s![0..4, 0..6]).fill(1);
///     data.slice_mut(s![4..8, 6..12]).fill(2);
///
///     rec.log(
///         "segmentation/image",
///         &rerun::SegmentationImage::try_from(data)?,
///     )?;
///
///     rerun::native_viewer::show(storage.take())?;
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/1200w.png">
///   <img src="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/full.png" width="640">
/// </picture>
/// </center>
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnnotationContext {
    /// List of class descriptions, mapping class indices to class names, colors etc.
    pub context: crate::components::AnnotationContext,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.AnnotationContext".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.AnnotationContextIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.InstanceKey".into()]);

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.AnnotationContext".into(),
            "rerun.components.AnnotationContextIndicator".into(),
            "rerun.components.InstanceKey".into(),
        ]
    });

impl AnnotationContext {
    pub const NUM_COMPONENTS: usize = 3usize;
}

/// Indicator component for the [`AnnotationContext`] [`::re_types_core::Archetype`]
pub type AnnotationContextIndicator = ::re_types_core::GenericIndicatorComponent<AnnotationContext>;

impl ::re_types_core::Archetype for AnnotationContext {
    type Indicator = AnnotationContextIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.archetypes.AnnotationContext".into()
    }

    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        static INDICATOR: AnnotationContextIndicator = AnnotationContextIndicator::DEFAULT;
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
        let context = {
            let array = arrays_by_name
                .get("rerun.components.AnnotationContext")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.AnnotationContext#context")?;
            <crate::components::AnnotationContext>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.AnnotationContext#context")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.AnnotationContext#context")?
        };
        Ok(Self { context })
    }
}

impl ::re_types_core::AsComponents for AnnotationContext {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.context as &dyn ComponentBatch).into()),
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

impl AnnotationContext {
    pub fn new(context: impl Into<crate::components::AnnotationContext>) -> Self {
        Self {
            context: context.into(),
        }
    }
}
