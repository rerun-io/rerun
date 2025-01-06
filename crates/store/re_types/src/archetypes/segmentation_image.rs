// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/segmentation_image.fbs".

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

use ::re_types_core::external::arrow;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, ComponentBatchCowWithDescriptor};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Archetype**: An image made up of integer [`components::ClassId`][crate::components::ClassId]s.
///
/// Each pixel corresponds to a [`components::ClassId`][crate::components::ClassId] that will be mapped to a color based on annotation context.
///
/// In the case of floating point images, the label will be looked up based on rounding to the nearest
/// integer value.
///
/// See also [`archetypes::AnnotationContext`][crate::archetypes::AnnotationContext] to associate each class with a color and a label.
///
/// ## Example
///
/// ### Simple segmentation image
/// ```ignore
/// use ndarray::{s, Array, ShapeBuilder};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let rec = rerun::RecordingStreamBuilder::new("rerun_example_segmentation_image").spawn()?;
///
///     // create a segmentation image
///     let mut image = Array::<u8, _>::zeros((8, 12).f());
///     image.slice_mut(s![0..4, 0..6]).fill(1);
///     image.slice_mut(s![4..8, 6..12]).fill(2);
///
///     // create an annotation context to describe the classes
///     let annotation = rerun::AnnotationContext::new([
///         (1, "red", rerun::Rgba32::from_rgb(255, 0, 0)),
///         (2, "green", rerun::Rgba32::from_rgb(0, 255, 0)),
///     ]);
///
///     // log the annotation and the image
///     rec.log_static("/", &annotation)?;
///
///     rec.log("image", &rerun::SegmentationImage::try_from(image)?)?;
///
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/segmentation_image_simple/f8aac62abcf4c59c5d62f9ebc2d86fd0285c1736/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/segmentation_image_simple/f8aac62abcf4c59c5d62f9ebc2d86fd0285c1736/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/segmentation_image_simple/f8aac62abcf4c59c5d62f9ebc2d86fd0285c1736/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/segmentation_image_simple/f8aac62abcf4c59c5d62f9ebc2d86fd0285c1736/1200w.png">
///   <img src="https://static.rerun.io/segmentation_image_simple/f8aac62abcf4c59c5d62f9ebc2d86fd0285c1736/full.png" width="640">
/// </picture>
/// </center>
#[derive(Clone, Debug, PartialEq)]
pub struct SegmentationImage {
    /// The raw image data.
    pub buffer: crate::components::ImageBuffer,

    /// The format of the image.
    pub format: crate::components::ImageFormat,

    /// Opacity of the image, useful for layering the segmentation image on top of another image.
    ///
    /// Defaults to 0.5 if there's any other images in the scene, otherwise 1.0.
    pub opacity: Option<crate::components::Opacity>,

    /// An optional floating point value that specifies the 2D drawing order.
    ///
    /// Objects with higher values are drawn on top of those with lower values.
    pub draw_order: Option<crate::components::DrawOrder>,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.SegmentationImage".into()),
                component_name: "rerun.components.ImageBuffer".into(),
                archetype_field_name: Some("buffer".into()),
            },
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.SegmentationImage".into()),
                component_name: "rerun.components.ImageFormat".into(),
                archetype_field_name: Some("format".into()),
            },
        ]
    });

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| {
        [ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.SegmentationImage".into()),
            component_name: "rerun.components.SegmentationImageIndicator".into(),
            archetype_field_name: None,
        }]
    });

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.SegmentationImage".into()),
                component_name: "rerun.components.Opacity".into(),
                archetype_field_name: Some("opacity".into()),
            },
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.SegmentationImage".into()),
                component_name: "rerun.components.DrawOrder".into(),
                archetype_field_name: Some("draw_order".into()),
            },
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 5usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.SegmentationImage".into()),
                component_name: "rerun.components.ImageBuffer".into(),
                archetype_field_name: Some("buffer".into()),
            },
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.SegmentationImage".into()),
                component_name: "rerun.components.ImageFormat".into(),
                archetype_field_name: Some("format".into()),
            },
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.SegmentationImage".into()),
                component_name: "rerun.components.SegmentationImageIndicator".into(),
                archetype_field_name: None,
            },
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.SegmentationImage".into()),
                component_name: "rerun.components.Opacity".into(),
                archetype_field_name: Some("opacity".into()),
            },
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.SegmentationImage".into()),
                component_name: "rerun.components.DrawOrder".into(),
                archetype_field_name: Some("draw_order".into()),
            },
        ]
    });

impl SegmentationImage {
    /// The total number of components in the archetype: 2 required, 1 recommended, 2 optional
    pub const NUM_COMPONENTS: usize = 5usize;
}

/// Indicator component for the [`SegmentationImage`] [`::re_types_core::Archetype`]
pub type SegmentationImageIndicator = ::re_types_core::GenericIndicatorComponent<SegmentationImage>;

impl ::re_types_core::Archetype for SegmentationImage {
    type Indicator = SegmentationImageIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.archetypes.SegmentationImage".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Segmentation image"
    }

    #[inline]
    fn indicator() -> ComponentBatchCowWithDescriptor<'static> {
        static INDICATOR: SegmentationImageIndicator = SegmentationImageIndicator::DEFAULT;
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
        arrow_data: impl IntoIterator<Item = (ComponentName, arrow::array::ArrayRef)>,
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
                .with_context("rerun.archetypes.SegmentationImage#buffer")?;
            <crate::components::ImageBuffer>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.SegmentationImage#buffer")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.SegmentationImage#buffer")?
        };
        let format = {
            let array = arrays_by_name
                .get("rerun.components.ImageFormat")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.SegmentationImage#format")?;
            <crate::components::ImageFormat>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.SegmentationImage#format")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.SegmentationImage#format")?
        };
        let opacity = if let Some(array) = arrays_by_name.get("rerun.components.Opacity") {
            <crate::components::Opacity>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.SegmentationImage#opacity")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let draw_order = if let Some(array) = arrays_by_name.get("rerun.components.DrawOrder") {
            <crate::components::DrawOrder>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.SegmentationImage#draw_order")?
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

impl ::re_types_core::AsComponents for SegmentationImage {
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            (Some(&self.buffer as &dyn ComponentBatch)).map(|batch| {
                ::re_types_core::ComponentBatchCowWithDescriptor {
                    batch: batch.into(),
                    descriptor_override: Some(ComponentDescriptor {
                        archetype_name: Some("rerun.archetypes.SegmentationImage".into()),
                        archetype_field_name: Some(("buffer").into()),
                        component_name: ("rerun.components.ImageBuffer").into(),
                    }),
                }
            }),
            (Some(&self.format as &dyn ComponentBatch)).map(|batch| {
                ::re_types_core::ComponentBatchCowWithDescriptor {
                    batch: batch.into(),
                    descriptor_override: Some(ComponentDescriptor {
                        archetype_name: Some("rerun.archetypes.SegmentationImage".into()),
                        archetype_field_name: Some(("format").into()),
                        component_name: ("rerun.components.ImageFormat").into(),
                    }),
                }
            }),
            (self
                .opacity
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch)))
            .map(|batch| ::re_types_core::ComponentBatchCowWithDescriptor {
                batch: batch.into(),
                descriptor_override: Some(ComponentDescriptor {
                    archetype_name: Some("rerun.archetypes.SegmentationImage".into()),
                    archetype_field_name: Some(("opacity").into()),
                    component_name: ("rerun.components.Opacity").into(),
                }),
            }),
            (self
                .draw_order
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch)))
            .map(|batch| ::re_types_core::ComponentBatchCowWithDescriptor {
                batch: batch.into(),
                descriptor_override: Some(ComponentDescriptor {
                    archetype_name: Some("rerun.archetypes.SegmentationImage".into()),
                    archetype_field_name: Some(("draw_order").into()),
                    component_name: ("rerun.components.DrawOrder").into(),
                }),
            }),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for SegmentationImage {}

impl SegmentationImage {
    /// Create a new `SegmentationImage`.
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

    /// Opacity of the image, useful for layering the segmentation image on top of another image.
    ///
    /// Defaults to 0.5 if there's any other images in the scene, otherwise 1.0.
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

impl ::re_byte_size::SizeBytes for SegmentationImage {
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
