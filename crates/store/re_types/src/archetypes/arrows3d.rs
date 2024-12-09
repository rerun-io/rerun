// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/arrows3d.fbs".

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

/// **Archetype**: 3D arrows with optional colors, radii, labels, etc.
///
/// ## Example
///
/// ### Simple batch of 3D arrows
/// ```ignore
/// use std::f32::consts::TAU;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let rec = rerun::RecordingStreamBuilder::new("rerun_example_arrow3d").spawn()?;
///
///     let origins = vec![rerun::Position3D::ZERO; 100];
///     let (vectors, colors): (Vec<_>, Vec<_>) = (0..100)
///         .map(|i| {
///             let angle = TAU * i as f32 * 0.01;
///             let length = ((i + 1) as f32).log2();
///             let c = (angle / TAU * 255.0).round() as u8;
///             (
///                 rerun::Vector3D::from([(length * angle.sin()), 0.0, (length * angle.cos())]),
///                 rerun::Color::from_unmultiplied_rgba(255 - c, c, 128, 128),
///             )
///         })
///         .unzip();
///
///     rec.log(
///         "arrows",
///         &rerun::Arrows3D::from_vectors(vectors)
///             .with_origins(origins)
///             .with_colors(colors),
///     )?;
///
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/arrow3d_simple/55e2f794a520bbf7527d7b828b0264732146c5d0/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/arrow3d_simple/55e2f794a520bbf7527d7b828b0264732146c5d0/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/arrow3d_simple/55e2f794a520bbf7527d7b828b0264732146c5d0/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/arrow3d_simple/55e2f794a520bbf7527d7b828b0264732146c5d0/1200w.png">
///   <img src="https://static.rerun.io/arrow3d_simple/55e2f794a520bbf7527d7b828b0264732146c5d0/full.png" width="640">
/// </picture>
/// </center>
#[derive(Clone, Debug, PartialEq)]
pub struct Arrows3D {
    /// All the vectors for each arrow in the batch.
    pub vectors: Vec<crate::components::Vector3D>,

    /// All the origin (base) positions for each arrow in the batch.
    ///
    /// If no origins are set, (0, 0, 0) is used as the origin for each arrow.
    pub origins: Option<Vec<crate::components::Position3D>>,

    /// Optional radii for the arrows.
    ///
    /// The shaft is rendered as a line with `radius = 0.5 * radius`.
    /// The tip is rendered with `height = 2.0 * radius` and `radius = 1.0 * radius`.
    pub radii: Option<Vec<crate::components::Radius>>,

    /// Optional colors for the points.
    pub colors: Option<Vec<crate::components::Color>>,

    /// Optional text labels for the arrows.
    ///
    /// If there's a single label present, it will be placed at the center of the entity.
    /// Otherwise, each instance will have its own label.
    pub labels: Option<Vec<crate::components::Text>>,

    /// Optional choice of whether the text labels should be shown by default.
    pub show_labels: Option<crate::components::ShowLabels>,

    /// Optional class Ids for the points.
    ///
    /// The [`components::ClassId`][crate::components::ClassId] provides colors and labels if not specified explicitly.
    pub class_ids: Option<Vec<crate::components::ClassId>>,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| {
        [ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.Arrows3D".into()),
            component_name: "rerun.components.Vector3D".into(),
            archetype_field_name: Some("vectors".into()),
        }]
    });

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.Arrows3D".into()),
                component_name: "rerun.components.Position3D".into(),
                archetype_field_name: Some("origins".into()),
            },
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.Arrows3D".into()),
                component_name: Arrows3DIndicator::DEFAULT.descriptor().component_name,
                archetype_field_name: None,
            },
        ]
    });

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 5usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.Arrows3D".into()),
                component_name: "rerun.components.Radius".into(),
                archetype_field_name: Some("radii".into()),
            },
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.Arrows3D".into()),
                component_name: "rerun.components.Color".into(),
                archetype_field_name: Some("colors".into()),
            },
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.Arrows3D".into()),
                component_name: "rerun.components.Text".into(),
                archetype_field_name: Some("labels".into()),
            },
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.Arrows3D".into()),
                component_name: "rerun.components.ShowLabels".into(),
                archetype_field_name: Some("show_labels".into()),
            },
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.Arrows3D".into()),
                component_name: "rerun.components.ClassId".into(),
                archetype_field_name: Some("class_ids".into()),
            },
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 8usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.Arrows3D".into()),
                component_name: "rerun.components.Vector3D".into(),
                archetype_field_name: Some("vectors".into()),
            },
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.Arrows3D".into()),
                component_name: "rerun.components.Position3D".into(),
                archetype_field_name: Some("origins".into()),
            },
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.Arrows3D".into()),
                component_name: Arrows3DIndicator::DEFAULT.descriptor().component_name,
                archetype_field_name: None,
            },
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.Arrows3D".into()),
                component_name: "rerun.components.Radius".into(),
                archetype_field_name: Some("radii".into()),
            },
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.Arrows3D".into()),
                component_name: "rerun.components.Color".into(),
                archetype_field_name: Some("colors".into()),
            },
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.Arrows3D".into()),
                component_name: "rerun.components.Text".into(),
                archetype_field_name: Some("labels".into()),
            },
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.Arrows3D".into()),
                component_name: "rerun.components.ShowLabels".into(),
                archetype_field_name: Some("show_labels".into()),
            },
            ComponentDescriptor {
                archetype_name: Some("rerun.archetypes.Arrows3D".into()),
                component_name: "rerun.components.ClassId".into(),
                archetype_field_name: Some("class_ids".into()),
            },
        ]
    });

impl Arrows3D {
    /// The total number of components in the archetype: 1 required, 2 recommended, 5 optional
    pub const NUM_COMPONENTS: usize = 8usize;
}

/// Indicator component for the [`Arrows3D`] [`::re_types_core::Archetype`]
pub type Arrows3DIndicator = ::re_types_core::GenericIndicatorComponent<Arrows3D>;

impl ::re_types_core::Archetype for Arrows3D {
    type Indicator = Arrows3DIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.archetypes.Arrows3D".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Arrows 3D"
    }

    #[inline]
    fn indicator() -> ComponentBatchCowWithDescriptor<'static> {
        static INDICATOR: Arrows3DIndicator = Arrows3DIndicator::DEFAULT;
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
        let vectors = {
            let array = arrays_by_name
                .get("rerun.components.Vector3D")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.Arrows3D#vectors")?;
            <crate::components::Vector3D>::from_arrow2_opt(&**array)
                .with_context("rerun.archetypes.Arrows3D#vectors")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.archetypes.Arrows3D#vectors")?
        };
        let origins = if let Some(array) = arrays_by_name.get("rerun.components.Position3D") {
            Some({
                <crate::components::Position3D>::from_arrow2_opt(&**array)
                    .with_context("rerun.archetypes.Arrows3D#origins")?
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.Arrows3D#origins")?
            })
        } else {
            None
        };
        let radii = if let Some(array) = arrays_by_name.get("rerun.components.Radius") {
            Some({
                <crate::components::Radius>::from_arrow2_opt(&**array)
                    .with_context("rerun.archetypes.Arrows3D#radii")?
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.Arrows3D#radii")?
            })
        } else {
            None
        };
        let colors = if let Some(array) = arrays_by_name.get("rerun.components.Color") {
            Some({
                <crate::components::Color>::from_arrow2_opt(&**array)
                    .with_context("rerun.archetypes.Arrows3D#colors")?
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.Arrows3D#colors")?
            })
        } else {
            None
        };
        let labels = if let Some(array) = arrays_by_name.get("rerun.components.Text") {
            Some({
                <crate::components::Text>::from_arrow2_opt(&**array)
                    .with_context("rerun.archetypes.Arrows3D#labels")?
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.Arrows3D#labels")?
            })
        } else {
            None
        };
        let show_labels = if let Some(array) = arrays_by_name.get("rerun.components.ShowLabels") {
            <crate::components::ShowLabels>::from_arrow2_opt(&**array)
                .with_context("rerun.archetypes.Arrows3D#show_labels")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let class_ids = if let Some(array) = arrays_by_name.get("rerun.components.ClassId") {
            Some({
                <crate::components::ClassId>::from_arrow2_opt(&**array)
                    .with_context("rerun.archetypes.Arrows3D#class_ids")?
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.Arrows3D#class_ids")?
            })
        } else {
            None
        };
        Ok(Self {
            vectors,
            origins,
            radii,
            colors,
            labels,
            show_labels,
            class_ids,
        })
    }
}

impl ::re_types_core::AsComponents for Arrows3D {
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            (Some(&self.vectors as &dyn ComponentBatch)).map(|batch| {
                ::re_types_core::ComponentBatchCowWithDescriptor {
                    batch: batch.into(),
                    descriptor_override: Some(ComponentDescriptor {
                        archetype_name: Some("rerun.archetypes.Arrows3D".into()),
                        archetype_field_name: Some(("vectors").into()),
                        component_name: ("rerun.components.Vector3D").into(),
                    }),
                }
            }),
            (self
                .origins
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch)))
            .map(|batch| ::re_types_core::ComponentBatchCowWithDescriptor {
                batch: batch.into(),
                descriptor_override: Some(ComponentDescriptor {
                    archetype_name: Some("rerun.archetypes.Arrows3D".into()),
                    archetype_field_name: Some(("origins").into()),
                    component_name: ("rerun.components.Position3D").into(),
                }),
            }),
            (self
                .radii
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch)))
            .map(|batch| ::re_types_core::ComponentBatchCowWithDescriptor {
                batch: batch.into(),
                descriptor_override: Some(ComponentDescriptor {
                    archetype_name: Some("rerun.archetypes.Arrows3D".into()),
                    archetype_field_name: Some(("radii").into()),
                    component_name: ("rerun.components.Radius").into(),
                }),
            }),
            (self
                .colors
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch)))
            .map(|batch| ::re_types_core::ComponentBatchCowWithDescriptor {
                batch: batch.into(),
                descriptor_override: Some(ComponentDescriptor {
                    archetype_name: Some("rerun.archetypes.Arrows3D".into()),
                    archetype_field_name: Some(("colors").into()),
                    component_name: ("rerun.components.Color").into(),
                }),
            }),
            (self
                .labels
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch)))
            .map(|batch| ::re_types_core::ComponentBatchCowWithDescriptor {
                batch: batch.into(),
                descriptor_override: Some(ComponentDescriptor {
                    archetype_name: Some("rerun.archetypes.Arrows3D".into()),
                    archetype_field_name: Some(("labels").into()),
                    component_name: ("rerun.components.Text").into(),
                }),
            }),
            (self
                .show_labels
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch)))
            .map(|batch| ::re_types_core::ComponentBatchCowWithDescriptor {
                batch: batch.into(),
                descriptor_override: Some(ComponentDescriptor {
                    archetype_name: Some("rerun.archetypes.Arrows3D".into()),
                    archetype_field_name: Some(("show_labels").into()),
                    component_name: ("rerun.components.ShowLabels").into(),
                }),
            }),
            (self
                .class_ids
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch)))
            .map(|batch| ::re_types_core::ComponentBatchCowWithDescriptor {
                batch: batch.into(),
                descriptor_override: Some(ComponentDescriptor {
                    archetype_name: Some("rerun.archetypes.Arrows3D".into()),
                    archetype_field_name: Some(("class_ids").into()),
                    component_name: ("rerun.components.ClassId").into(),
                }),
            }),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for Arrows3D {}

impl Arrows3D {
    /// Create a new `Arrows3D`.
    #[inline]
    pub(crate) fn new(
        vectors: impl IntoIterator<Item = impl Into<crate::components::Vector3D>>,
    ) -> Self {
        Self {
            vectors: vectors.into_iter().map(Into::into).collect(),
            origins: None,
            radii: None,
            colors: None,
            labels: None,
            show_labels: None,
            class_ids: None,
        }
    }

    /// All the origin (base) positions for each arrow in the batch.
    ///
    /// If no origins are set, (0, 0, 0) is used as the origin for each arrow.
    #[inline]
    pub fn with_origins(
        mut self,
        origins: impl IntoIterator<Item = impl Into<crate::components::Position3D>>,
    ) -> Self {
        self.origins = Some(origins.into_iter().map(Into::into).collect());
        self
    }

    /// Optional radii for the arrows.
    ///
    /// The shaft is rendered as a line with `radius = 0.5 * radius`.
    /// The tip is rendered with `height = 2.0 * radius` and `radius = 1.0 * radius`.
    #[inline]
    pub fn with_radii(
        mut self,
        radii: impl IntoIterator<Item = impl Into<crate::components::Radius>>,
    ) -> Self {
        self.radii = Some(radii.into_iter().map(Into::into).collect());
        self
    }

    /// Optional colors for the points.
    #[inline]
    pub fn with_colors(
        mut self,
        colors: impl IntoIterator<Item = impl Into<crate::components::Color>>,
    ) -> Self {
        self.colors = Some(colors.into_iter().map(Into::into).collect());
        self
    }

    /// Optional text labels for the arrows.
    ///
    /// If there's a single label present, it will be placed at the center of the entity.
    /// Otherwise, each instance will have its own label.
    #[inline]
    pub fn with_labels(
        mut self,
        labels: impl IntoIterator<Item = impl Into<crate::components::Text>>,
    ) -> Self {
        self.labels = Some(labels.into_iter().map(Into::into).collect());
        self
    }

    /// Optional choice of whether the text labels should be shown by default.
    #[inline]
    pub fn with_show_labels(
        mut self,
        show_labels: impl Into<crate::components::ShowLabels>,
    ) -> Self {
        self.show_labels = Some(show_labels.into());
        self
    }

    /// Optional class Ids for the points.
    ///
    /// The [`components::ClassId`][crate::components::ClassId] provides colors and labels if not specified explicitly.
    #[inline]
    pub fn with_class_ids(
        mut self,
        class_ids: impl IntoIterator<Item = impl Into<crate::components::ClassId>>,
    ) -> Self {
        self.class_ids = Some(class_ids.into_iter().map(Into::into).collect());
        self
    }
}

impl ::re_types_core::SizeBytes for Arrows3D {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.vectors.heap_size_bytes()
            + self.origins.heap_size_bytes()
            + self.radii.heap_size_bytes()
            + self.colors.heap_size_bytes()
            + self.labels.heap_size_bytes()
            + self.show_labels.heap_size_bytes()
            + self.class_ids.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Vec<crate::components::Vector3D>>::is_pod()
            && <Option<Vec<crate::components::Position3D>>>::is_pod()
            && <Option<Vec<crate::components::Radius>>>::is_pod()
            && <Option<Vec<crate::components::Color>>>::is_pod()
            && <Option<Vec<crate::components::Text>>>::is_pod()
            && <Option<crate::components::ShowLabels>>::is_pod()
            && <Option<Vec<crate::components::ClassId>>>::is_pod()
    }
}
