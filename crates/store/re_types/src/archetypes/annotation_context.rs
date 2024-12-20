// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/annotation_context.fbs".

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
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, ComponentBatchCowWithDescriptor};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Archetype**: The annotation context provides additional information on how to display entities.
///
/// Entities can use [`components::ClassId`][crate::components::ClassId]s and [`components::KeypointId`][crate::components::KeypointId]s to provide annotations, and
/// the labels and colors will be looked up in the appropriate
/// annotation context. We use the *first* annotation context we find in the
/// path-hierarchy when searching up through the ancestors of a given entity
/// path.
///
/// See also [`datatypes::ClassDescription`][crate::datatypes::ClassDescription].
///
/// ## Example
///
/// ### Segmentation
/// ```ignore
/// use ndarray::{s, Array, ShapeBuilder};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let rec = rerun::RecordingStreamBuilder::new("rerun_example_annotation_context_segmentation")
///         .spawn()?;
///
///     // create an annotation context to describe the classes
///     rec.log_static(
///         "segmentation",
///         &rerun::AnnotationContext::new([
///             (1, "red", rerun::Rgba32::from_rgb(255, 0, 0)),
///             (2, "green", rerun::Rgba32::from_rgb(0, 255, 0)),
///         ]),
///     )?;
///
///     // create a segmentation image
///     let mut data = Array::<u8, _>::zeros((200, 300).f());
///     data.slice_mut(s![50..100, 50..120]).fill(1);
///     data.slice_mut(s![100..180, 130..280]).fill(2);
///
///     rec.log(
///         "segmentation/image",
///         &rerun::SegmentationImage::try_from(data)?,
///     )?;
///
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/annotation_context_segmentation/6c9e88fc9d44a08031cadd444c2e58a985cc1208/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/annotation_context_segmentation/6c9e88fc9d44a08031cadd444c2e58a985cc1208/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/annotation_context_segmentation/6c9e88fc9d44a08031cadd444c2e58a985cc1208/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/annotation_context_segmentation/6c9e88fc9d44a08031cadd444c2e58a985cc1208/1200w.png">
///   <img src="https://static.rerun.io/annotation_context_segmentation/6c9e88fc9d44a08031cadd444c2e58a985cc1208/full.png" width="640">
/// </picture>
/// </center>
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnnotationContext {
    /// List of class descriptions, mapping class indices to class names, colors etc.
    pub context: crate::components::AnnotationContext,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| {
        [ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.AnnotationContext".into()),
            component_name: "rerun.components.AnnotationContext".into(),
            archetype_field_name: Some("context".into()),
        }]
    });

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| {
        [ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.AnnotationContext".into()),
            component_name: "rerun.components.AnnotationContextIndicator".into(),
            archetype_field_name: None,
        }]
    });

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.AnnotationContext".into()),
                component_name: "rerun.components.AnnotationContext".into(),
                archetype_field_name: Some("context".into()),
            },
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.AnnotationContext".into()),
                component_name: "rerun.components.AnnotationContextIndicator".into(),
                archetype_field_name: None,
            },
        ]
    });

impl AnnotationContext {
    /// The total number of components in the archetype: 1 required, 1 recommended, 0 optional
    pub const NUM_COMPONENTS: usize = 2usize;
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
    fn display_name() -> &'static str {
        "Annotation context"
    }

    #[inline]
    fn indicator() -> ComponentBatchCowWithDescriptor<'static> {
        static INDICATOR: AnnotationContextIndicator = AnnotationContextIndicator::DEFAULT;
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
    fn from_arrow2_components(
        arrow_data: impl IntoIterator<Item = (ComponentName, Box<dyn arrow2::array::Array>)>,
    ) -> DeserializationResult<Self> {
        re_tracing::profile_function!();
        use ::re_types_core::{Loggable as _, ResultExt as _};
        let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
            .into_iter()
            .map(|(name, array)| (name.full_name(), array))
            .collect();
        let context = {
            let array = arrays_by_name
                .get("rerun.components.AnnotationContext")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.AnnotationContext#context")?;
            <crate::components::AnnotationContext>::from_arrow2_opt(&**array)
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
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            (Some(&self.context as &dyn ComponentBatch)).map(|batch| {
                ::re_types_core::ComponentBatchCowWithDescriptor {
                    batch: batch.into(),
                    descriptor_override: Some(ComponentDescriptor {
                        archetype_name: Some("rerun.archetypes.AnnotationContext".into()),
                        archetype_field_name: Some(("context").into()),
                        component_name: ("rerun.components.AnnotationContext").into(),
                    }),
                }
            }),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for AnnotationContext {}

impl AnnotationContext {
    /// Create a new `AnnotationContext`.
    #[inline]
    pub fn new(context: impl Into<crate::components::AnnotationContext>) -> Self {
        Self {
            context: context.into(),
        }
    }
}

impl ::re_byte_size::SizeBytes for AnnotationContext {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.context.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::components::AnnotationContext>::is_pod()
    }
}
