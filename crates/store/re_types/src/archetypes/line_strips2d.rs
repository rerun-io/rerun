// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/line_strips2d.fbs".

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

/// **Archetype**: 2D line strips with positions and optional colors, radii, labels, etc.
///
/// ## Examples
///
/// ### `line_strips2d_batch`:
/// ```ignore
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let rec = rerun::RecordingStreamBuilder::new("rerun_example_line_strip2d_batch").spawn()?;
///
///     let strip1 = [[0., 0.], [2., 1.], [4., -1.], [6., 0.]];
///     #[rustfmt::skip]
///     let strip2 = [[0., 3.], [1., 4.], [2., 2.], [3., 4.], [4., 2.], [5., 4.], [6., 3.]];
///     rec.log(
///         "strips",
///         &rerun::LineStrips2D::new([strip1.to_vec(), strip2.to_vec()])
///             .with_colors([0xFF0000FF, 0x00FF00FF])
///             .with_radii([0.025, 0.005])
///             .with_labels(["one strip here", "and one strip there"]),
///     )?;
///
///     // TODO(#5521): log VisualBounds2D
///
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/line_strip2d_batch/c6f4062bcf510462d298a5dfe9fdbe87c754acee/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/line_strip2d_batch/c6f4062bcf510462d298a5dfe9fdbe87c754acee/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/line_strip2d_batch/c6f4062bcf510462d298a5dfe9fdbe87c754acee/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/line_strip2d_batch/c6f4062bcf510462d298a5dfe9fdbe87c754acee/1200w.png">
///   <img src="https://static.rerun.io/line_strip2d_batch/c6f4062bcf510462d298a5dfe9fdbe87c754acee/full.png" width="640">
/// </picture>
/// </center>
///
/// ### Lines with scene & UI radius each
/// ```ignore
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let rec = rerun::RecordingStreamBuilder::new("rerun_example_line_strip2d_ui_radius").spawn()?;
///
///     // A blue line with a scene unit radii of 0.01.
///     let points = [[0., 0.], [0., 1.], [1., 0.], [1., 1.]];
///     rec.log(
///         "scene_unit_line",
///         &rerun::LineStrips2D::new([points])
///             // By default, radii are interpreted as world-space units.
///             .with_radii([0.01])
///             .with_colors([rerun::Color::from_rgb(0, 0, 255)]),
///     )?;
///
///     // A red line with a ui point radii of 5.
///     // UI points are independent of zooming in Views, but are sensitive to the application UI scaling.
///     // For 100 % ui scaling, UI points are equal to pixels.
///     let points = [[3., 0.], [3., 1.], [4., 0.], [4., 1.]];
///     rec.log(
///         "ui_points_line",
///         &rerun::LineStrips2D::new([points])
///             // rerun::Radius::new_ui_points produces a radius that the viewer interprets as given in ui points.
///             .with_radii([rerun::Radius::new_ui_points(5.0)])
///             .with_colors([rerun::Color::from_rgb(255, 0, 0)]),
///     )?;
///
///     // TODO(#5520): log VisualBounds2D
///
///     Ok(())
/// }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct LineStrips2D {
    /// All the actual 2D line strips that make up the batch.
    pub strips: Vec<crate::components::LineStrip2D>,

    /// Optional radii for the line strips.
    pub radii: Option<Vec<crate::components::Radius>>,

    /// Optional colors for the line strips.
    pub colors: Option<Vec<crate::components::Color>>,

    /// Optional text labels for the line strips.
    ///
    /// If there's a single label present, it will be placed at the center of the entity.
    /// Otherwise, each instance will have its own label.
    pub labels: Option<Vec<crate::components::Text>>,

    /// Optional choice of whether the text labels should be shown by default.
    pub show_labels: Option<crate::components::ShowLabels>,

    /// An optional floating point value that specifies the 2D drawing order of each line strip.
    ///
    /// Objects with higher values are drawn on top of those with lower values.
    pub draw_order: Option<crate::components::DrawOrder>,

    /// Optional [`components::ClassId`][crate::components::ClassId]s for the lines.
    ///
    /// The [`components::ClassId`][crate::components::ClassId] provides colors and labels if not specified explicitly.
    pub class_ids: Option<Vec<crate::components::ClassId>>,
}

impl LineStrips2D {
    /// Returns the [`ComponentDescriptor`] for [`Self::strips`].
    #[inline]
    pub fn descriptor_strips() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.LineStrips2D".into()),
            component_name: "rerun.components.LineStrip2D".into(),
            archetype_field_name: Some("strips".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::radii`].
    #[inline]
    pub fn descriptor_radii() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.LineStrips2D".into()),
            component_name: "rerun.components.Radius".into(),
            archetype_field_name: Some("radii".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::colors`].
    #[inline]
    pub fn descriptor_colors() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.LineStrips2D".into()),
            component_name: "rerun.components.Color".into(),
            archetype_field_name: Some("colors".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::labels`].
    #[inline]
    pub fn descriptor_labels() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.LineStrips2D".into()),
            component_name: "rerun.components.Text".into(),
            archetype_field_name: Some("labels".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::show_labels`].
    #[inline]
    pub fn descriptor_show_labels() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.LineStrips2D".into()),
            component_name: "rerun.components.ShowLabels".into(),
            archetype_field_name: Some("show_labels".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::draw_order`].
    #[inline]
    pub fn descriptor_draw_order() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.LineStrips2D".into()),
            component_name: "rerun.components.DrawOrder".into(),
            archetype_field_name: Some("draw_order".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::class_ids`].
    #[inline]
    pub fn descriptor_class_ids() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.LineStrips2D".into()),
            component_name: "rerun.components.ClassId".into(),
            archetype_field_name: Some("class_ids".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for the associated indicator component.
    #[inline]
    pub fn descriptor_indicator() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.LineStrips2D".into()),
            component_name: "rerun.components.LineStrips2DIndicator".into(),
            archetype_field_name: None,
        }
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| [LineStrips2D::descriptor_strips()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            LineStrips2D::descriptor_radii(),
            LineStrips2D::descriptor_colors(),
            LineStrips2D::descriptor_indicator(),
        ]
    });

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 4usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            LineStrips2D::descriptor_labels(),
            LineStrips2D::descriptor_show_labels(),
            LineStrips2D::descriptor_draw_order(),
            LineStrips2D::descriptor_class_ids(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 8usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            LineStrips2D::descriptor_strips(),
            LineStrips2D::descriptor_radii(),
            LineStrips2D::descriptor_colors(),
            LineStrips2D::descriptor_indicator(),
            LineStrips2D::descriptor_labels(),
            LineStrips2D::descriptor_show_labels(),
            LineStrips2D::descriptor_draw_order(),
            LineStrips2D::descriptor_class_ids(),
        ]
    });

impl LineStrips2D {
    /// The total number of components in the archetype: 1 required, 3 recommended, 4 optional
    pub const NUM_COMPONENTS: usize = 8usize;
}

/// Indicator component for the [`LineStrips2D`] [`::re_types_core::Archetype`]
pub type LineStrips2DIndicator = ::re_types_core::GenericIndicatorComponent<LineStrips2D>;

impl ::re_types_core::Archetype for LineStrips2D {
    type Indicator = LineStrips2DIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.archetypes.LineStrips2D".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Line strips 2D"
    }

    #[inline]
    fn indicator() -> ComponentBatchCowWithDescriptor<'static> {
        static INDICATOR: LineStrips2DIndicator = LineStrips2DIndicator::DEFAULT;
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
        let strips = {
            let array = arrays_by_descr
                .get(&Self::descriptor_strips())
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.LineStrips2D#strips")?;
            <crate::components::LineStrip2D>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.LineStrips2D#strips")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.archetypes.LineStrips2D#strips")?
        };
        let radii = if let Some(array) = arrays_by_descr.get(&Self::descriptor_radii()) {
            Some({
                <crate::components::Radius>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.LineStrips2D#radii")?
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.LineStrips2D#radii")?
            })
        } else {
            None
        };
        let colors = if let Some(array) = arrays_by_descr.get(&Self::descriptor_colors()) {
            Some({
                <crate::components::Color>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.LineStrips2D#colors")?
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.LineStrips2D#colors")?
            })
        } else {
            None
        };
        let labels = if let Some(array) = arrays_by_descr.get(&Self::descriptor_labels()) {
            Some({
                <crate::components::Text>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.LineStrips2D#labels")?
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.LineStrips2D#labels")?
            })
        } else {
            None
        };
        let show_labels = if let Some(array) = arrays_by_descr.get(&Self::descriptor_show_labels())
        {
            <crate::components::ShowLabels>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.LineStrips2D#show_labels")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let draw_order = if let Some(array) = arrays_by_descr.get(&Self::descriptor_draw_order()) {
            <crate::components::DrawOrder>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.LineStrips2D#draw_order")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let class_ids = if let Some(array) = arrays_by_descr.get(&Self::descriptor_class_ids()) {
            Some({
                <crate::components::ClassId>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.LineStrips2D#class_ids")?
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.LineStrips2D#class_ids")?
            })
        } else {
            None
        };
        Ok(Self {
            strips,
            radii,
            colors,
            labels,
            show_labels,
            draw_order,
            class_ids,
        })
    }
}

impl ::re_types_core::AsComponents for LineStrips2D {
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            (Some(&self.strips as &dyn ComponentBatch)).map(|batch| {
                ::re_types_core::ComponentBatchCowWithDescriptor {
                    batch: batch.into(),
                    descriptor_override: Some(Self::descriptor_strips()),
                }
            }),
            (self
                .radii
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch)))
            .map(|batch| ::re_types_core::ComponentBatchCowWithDescriptor {
                batch: batch.into(),
                descriptor_override: Some(Self::descriptor_radii()),
            }),
            (self
                .colors
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch)))
            .map(|batch| ::re_types_core::ComponentBatchCowWithDescriptor {
                batch: batch.into(),
                descriptor_override: Some(Self::descriptor_colors()),
            }),
            (self
                .labels
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch)))
            .map(|batch| ::re_types_core::ComponentBatchCowWithDescriptor {
                batch: batch.into(),
                descriptor_override: Some(Self::descriptor_labels()),
            }),
            (self
                .show_labels
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch)))
            .map(|batch| ::re_types_core::ComponentBatchCowWithDescriptor {
                batch: batch.into(),
                descriptor_override: Some(Self::descriptor_show_labels()),
            }),
            (self
                .draw_order
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch)))
            .map(|batch| ::re_types_core::ComponentBatchCowWithDescriptor {
                batch: batch.into(),
                descriptor_override: Some(Self::descriptor_draw_order()),
            }),
            (self
                .class_ids
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch)))
            .map(|batch| ::re_types_core::ComponentBatchCowWithDescriptor {
                batch: batch.into(),
                descriptor_override: Some(Self::descriptor_class_ids()),
            }),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for LineStrips2D {}

impl LineStrips2D {
    /// Create a new `LineStrips2D`.
    #[inline]
    pub fn new(
        strips: impl IntoIterator<Item = impl Into<crate::components::LineStrip2D>>,
    ) -> Self {
        Self {
            strips: strips.into_iter().map(Into::into).collect(),
            radii: None,
            colors: None,
            labels: None,
            show_labels: None,
            draw_order: None,
            class_ids: None,
        }
    }

    /// Optional radii for the line strips.
    #[inline]
    pub fn with_radii(
        mut self,
        radii: impl IntoIterator<Item = impl Into<crate::components::Radius>>,
    ) -> Self {
        self.radii = Some(radii.into_iter().map(Into::into).collect());
        self
    }

    /// Optional colors for the line strips.
    #[inline]
    pub fn with_colors(
        mut self,
        colors: impl IntoIterator<Item = impl Into<crate::components::Color>>,
    ) -> Self {
        self.colors = Some(colors.into_iter().map(Into::into).collect());
        self
    }

    /// Optional text labels for the line strips.
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

    /// An optional floating point value that specifies the 2D drawing order of each line strip.
    ///
    /// Objects with higher values are drawn on top of those with lower values.
    #[inline]
    pub fn with_draw_order(mut self, draw_order: impl Into<crate::components::DrawOrder>) -> Self {
        self.draw_order = Some(draw_order.into());
        self
    }

    /// Optional [`components::ClassId`][crate::components::ClassId]s for the lines.
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

impl ::re_byte_size::SizeBytes for LineStrips2D {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.strips.heap_size_bytes()
            + self.radii.heap_size_bytes()
            + self.colors.heap_size_bytes()
            + self.labels.heap_size_bytes()
            + self.show_labels.heap_size_bytes()
            + self.draw_order.heap_size_bytes()
            + self.class_ids.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Vec<crate::components::LineStrip2D>>::is_pod()
            && <Option<Vec<crate::components::Radius>>>::is_pod()
            && <Option<Vec<crate::components::Color>>>::is_pod()
            && <Option<Vec<crate::components::Text>>>::is_pod()
            && <Option<crate::components::ShowLabels>>::is_pod()
            && <Option<crate::components::DrawOrder>>::is_pod()
            && <Option<Vec<crate::components::ClassId>>>::is_pod()
    }
}
