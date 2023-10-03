// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/archetypes/annotation_context.fbs".

#![allow(trivial_numeric_casts)]
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

/// **Archetype**:  The `AnnotationContext` provides additional information on how to display entities.
///
/// Entities can use `ClassId`s and `KeypointId`s to provide annotations, and
/// the labels and colors will be looked up in the appropriate
/// `AnnotationContext`. We use the *first* annotation context we find in the
/// path-hierarchy when searching up through the ancestors of a given entity
/// path.
///
/// ## Examples
///
/// ### Rectangles
/// ```ignore
/// //! Log rectangles with different colors and labels using annotation context
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (rec, storage) =
///         rerun::RecordingStreamBuilder::new("rerun_example_annotation_context_rects").memory()?;
///
///     // Log an annotation context to assign a label and color to each class
///     rec.log_timeless(
///         "/",
///         &rerun::AnnotationContext::new([
///             (1, "red", rerun::Rgba32::from(0xFF0000FF)),
///             (2, "green", rerun::Rgba32::from(0x00FF00FF)),
///         ]),
///     )?;
///
///     // Log a batch of 2 rectangles with different class IDs
///     rec.log(
///         "detections",
///         &rerun::Boxes2D::from_mins_and_sizes([(-2., -2.), (0., 0.)], [(3., 3.), (2., 2.)])
///             .with_class_ids([1, 2]),
///     )?;
///
///     // Log an extra rect to set the view bounds
///     rec.log("bounds", &rerun::Boxes2D::from_half_sizes([(2.5, 2.5)]))?;
///
///     rerun::native_viewer::show(storage.take())?;
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/annotation_context_rects/9b446c36011ed30fce7dc6ed03d5fd9557460f70/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/annotation_context_rects/9b446c36011ed30fce7dc6ed03d5fd9557460f70/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/annotation_context_rects/9b446c36011ed30fce7dc6ed03d5fd9557460f70/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/annotation_context_rects/9b446c36011ed30fce7dc6ed03d5fd9557460f70/1200w.png">
///   <img src="https://static.rerun.io/annotation_context_rects/9b446c36011ed30fce7dc6ed03d5fd9557460f70/full.png" width="640">
/// </picture>
/// </center>
///
/// ### Segmentation
/// ```ignore
/// //! Log a segmentation image with annotations.
///
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
///             (1, "red", rerun::Rgba32::from(0xFF0000FF)),
///             (2, "green", rerun::Rgba32::from(0x00FF00FF)),
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
///
/// ### Connections
/// ```ignore
/// //! Log some very simple points.
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (rec, storage) =
///         rerun::RecordingStreamBuilder::new("rerun_example_annotation_context_connections")
///             .memory()?;
///
///     // Log an annotation context to assign a label and color to each class
///     // Create a class description with labels and color for each keypoint ID as well as some
///     // connections between keypoints.
///     rec.log_timeless(
///         "/",
///         &rerun::AnnotationContext::new([rerun::ClassDescription {
///             info: 0.into(),
///             keypoint_annotations: vec![
///                 (0, "zero", rerun::Rgba32::from(0xFF0000FF)).into(),
///                 (1, "one", rerun::Rgba32::from(0x00FF00FF)).into(),
///                 (2, "two", rerun::Rgba32::from(0x0000FFFF)).into(),
///                 (3, "three", rerun::Rgba32::from(0xFFFF00FF)).into(),
///             ],
///             keypoint_connections: rerun::KeypointPair::vec_from([(0, 2), (1, 2), (2, 3)]),
///         }]),
///     )?;
///
///     // Log some points with different keypoint IDs
///     rec.log(
///         "points",
///         &rerun::Points3D::new([
///             [0., 0., 0.],
///             [50., 0., 20.],
///             [100., 100., 30.],
///             [0., 50., 40.],
///         ])
///         .with_keypoint_ids([0, 1, 2, 3])
///         .with_class_ids([0]),
///     )?;
///
///     rerun::native_viewer::show(storage.take())?;
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/annotation_context_connections/4a8422bc154699c5334f574ff01b55c5cd1748e3/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/annotation_context_connections/4a8422bc154699c5334f574ff01b55c5cd1748e3/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/annotation_context_connections/4a8422bc154699c5334f574ff01b55c5cd1748e3/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/annotation_context_connections/4a8422bc154699c5334f574ff01b55c5cd1748e3/1200w.png">
///   <img src="https://static.rerun.io/annotation_context_connections/4a8422bc154699c5334f574ff01b55c5cd1748e3/full.png" width="640">
/// </picture>
/// </center>
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnnotationContext {
    /// List of class descriptions, mapping class indices to class names, colors etc.
    pub context: crate::components::AnnotationContext,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.AnnotationContext".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.AnnotationContextIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.InstanceKey".into()]);

static ALL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 3usize]> =
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

/// Indicator component for the [`AnnotationContext`] [`crate::Archetype`]
pub type AnnotationContextIndicator = crate::GenericIndicatorComponent<AnnotationContext>;

impl crate::Archetype for AnnotationContext {
    type Indicator = AnnotationContextIndicator;

    #[inline]
    fn name() -> crate::ArchetypeName {
        "rerun.archetypes.AnnotationContext".into()
    }

    #[inline]
    fn indicator() -> crate::MaybeOwnedComponentBatch<'static> {
        static INDICATOR: AnnotationContextIndicator = AnnotationContextIndicator::DEFAULT;
        crate::MaybeOwnedComponentBatch::Ref(&INDICATOR)
    }

    #[inline]
    fn required_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
        REQUIRED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn recommended_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
        RECOMMENDED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn optional_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
        OPTIONAL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn all_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
        ALL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn from_arrow(
        arrow_data: impl IntoIterator<
            Item = (::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>),
        >,
    ) -> crate::DeserializationResult<Self> {
        re_tracing::profile_function!();
        use crate::{Loggable as _, ResultExt as _};
        let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
            .into_iter()
            .map(|(field, array)| (field.name, array))
            .collect();
        let context = {
            let array = arrays_by_name
                .get("rerun.components.AnnotationContext")
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.AnnotationContext#context")?;
            <crate::components::AnnotationContext>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.AnnotationContext#context")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.AnnotationContext#context")?
        };
        Ok(Self { context })
    }
}

impl crate::AsComponents for AnnotationContext {
    fn as_component_batches(&self) -> Vec<crate::MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use crate::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.context as &dyn crate::ComponentBatch).into()),
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
