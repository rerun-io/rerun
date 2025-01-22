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

use ::re_types_core::try_serialize_field;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, ComponentBatchCowWithDescriptor, SerializedComponentBatch};
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
#[derive(Clone, Debug, PartialEq, Default)]
pub struct LineStrips2D {
    /// All the actual 2D line strips that make up the batch.
    pub strips: Option<SerializedComponentBatch>,

    /// Optional radii for the line strips.
    pub radii: Option<SerializedComponentBatch>,

    /// Optional colors for the line strips.
    pub colors: Option<SerializedComponentBatch>,

    /// Optional text labels for the line strips.
    ///
    /// If there's a single label present, it will be placed at the center of the entity.
    /// Otherwise, each instance will have its own label.
    pub labels: Option<SerializedComponentBatch>,

    /// Optional choice of whether the text labels should be shown by default.
    pub show_labels: Option<SerializedComponentBatch>,

    /// An optional floating point value that specifies the 2D drawing order of each line strip.
    ///
    /// Objects with higher values are drawn on top of those with lower values.
    pub draw_order: Option<SerializedComponentBatch>,

    /// Optional [`components::ClassId`][crate::components::ClassId]s for the lines.
    ///
    /// The [`components::ClassId`][crate::components::ClassId] provides colors and labels if not specified explicitly.
    pub class_ids: Option<SerializedComponentBatch>,
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
        let strips = arrays_by_descr
            .get(&Self::descriptor_strips())
            .map(|array| SerializedComponentBatch::new(array.clone(), Self::descriptor_strips()));
        let radii = arrays_by_descr
            .get(&Self::descriptor_radii())
            .map(|array| SerializedComponentBatch::new(array.clone(), Self::descriptor_radii()));
        let colors = arrays_by_descr
            .get(&Self::descriptor_colors())
            .map(|array| SerializedComponentBatch::new(array.clone(), Self::descriptor_colors()));
        let labels = arrays_by_descr
            .get(&Self::descriptor_labels())
            .map(|array| SerializedComponentBatch::new(array.clone(), Self::descriptor_labels()));
        let show_labels = arrays_by_descr
            .get(&Self::descriptor_show_labels())
            .map(|array| {
                SerializedComponentBatch::new(array.clone(), Self::descriptor_show_labels())
            });
        let draw_order = arrays_by_descr
            .get(&Self::descriptor_draw_order())
            .map(|array| {
                SerializedComponentBatch::new(array.clone(), Self::descriptor_draw_order())
            });
        let class_ids = arrays_by_descr
            .get(&Self::descriptor_class_ids())
            .map(|array| {
                SerializedComponentBatch::new(array.clone(), Self::descriptor_class_ids())
            });
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
    #[inline]
    fn as_serialized_batches(&self) -> Vec<SerializedComponentBatch> {
        use ::re_types_core::Archetype as _;
        [
            Self::indicator().serialized(),
            self.strips.clone(),
            self.radii.clone(),
            self.colors.clone(),
            self.labels.clone(),
            self.show_labels.clone(),
            self.draw_order.clone(),
            self.class_ids.clone(),
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
            strips: try_serialize_field(Self::descriptor_strips(), strips),
            radii: None,
            colors: None,
            labels: None,
            show_labels: None,
            draw_order: None,
            class_ids: None,
        }
    }

    /// Update only some specific fields of a `LineStrips2D`.
    #[inline]
    pub fn update_fields() -> Self {
        Self::default()
    }

    /// Clear all the fields of a `LineStrips2D`.
    #[inline]
    pub fn clear_fields() -> Self {
        use ::re_types_core::Loggable as _;
        Self {
            strips: Some(SerializedComponentBatch::new(
                crate::components::LineStrip2D::arrow_empty(),
                Self::descriptor_strips(),
            )),
            radii: Some(SerializedComponentBatch::new(
                crate::components::Radius::arrow_empty(),
                Self::descriptor_radii(),
            )),
            colors: Some(SerializedComponentBatch::new(
                crate::components::Color::arrow_empty(),
                Self::descriptor_colors(),
            )),
            labels: Some(SerializedComponentBatch::new(
                crate::components::Text::arrow_empty(),
                Self::descriptor_labels(),
            )),
            show_labels: Some(SerializedComponentBatch::new(
                crate::components::ShowLabels::arrow_empty(),
                Self::descriptor_show_labels(),
            )),
            draw_order: Some(SerializedComponentBatch::new(
                crate::components::DrawOrder::arrow_empty(),
                Self::descriptor_draw_order(),
            )),
            class_ids: Some(SerializedComponentBatch::new(
                crate::components::ClassId::arrow_empty(),
                Self::descriptor_class_ids(),
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
            self.strips
                .map(|strips| strips.partitioned(_lengths.clone()))
                .transpose()?,
            self.radii
                .map(|radii| radii.partitioned(_lengths.clone()))
                .transpose()?,
            self.colors
                .map(|colors| colors.partitioned(_lengths.clone()))
                .transpose()?,
            self.labels
                .map(|labels| labels.partitioned(_lengths.clone()))
                .transpose()?,
            self.show_labels
                .map(|show_labels| show_labels.partitioned(_lengths.clone()))
                .transpose()?,
            self.draw_order
                .map(|draw_order| draw_order.partitioned(_lengths.clone()))
                .transpose()?,
            self.class_ids
                .map(|class_ids| class_ids.partitioned(_lengths.clone()))
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
        let len_strips = self.strips.as_ref().map(|b| b.array.len());
        let len_radii = self.radii.as_ref().map(|b| b.array.len());
        let len_colors = self.colors.as_ref().map(|b| b.array.len());
        let len_labels = self.labels.as_ref().map(|b| b.array.len());
        let len_show_labels = self.show_labels.as_ref().map(|b| b.array.len());
        let len_draw_order = self.draw_order.as_ref().map(|b| b.array.len());
        let len_class_ids = self.class_ids.as_ref().map(|b| b.array.len());
        let len = None
            .or(len_strips)
            .or(len_radii)
            .or(len_colors)
            .or(len_labels)
            .or(len_show_labels)
            .or(len_draw_order)
            .or(len_class_ids)
            .unwrap_or(0);
        self.columns(std::iter::repeat(1).take(len))
    }

    /// All the actual 2D line strips that make up the batch.
    #[inline]
    pub fn with_strips(
        mut self,
        strips: impl IntoIterator<Item = impl Into<crate::components::LineStrip2D>>,
    ) -> Self {
        self.strips = try_serialize_field(Self::descriptor_strips(), strips);
        self
    }

    /// Optional radii for the line strips.
    #[inline]
    pub fn with_radii(
        mut self,
        radii: impl IntoIterator<Item = impl Into<crate::components::Radius>>,
    ) -> Self {
        self.radii = try_serialize_field(Self::descriptor_radii(), radii);
        self
    }

    /// Optional colors for the line strips.
    #[inline]
    pub fn with_colors(
        mut self,
        colors: impl IntoIterator<Item = impl Into<crate::components::Color>>,
    ) -> Self {
        self.colors = try_serialize_field(Self::descriptor_colors(), colors);
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
        self.labels = try_serialize_field(Self::descriptor_labels(), labels);
        self
    }

    /// Optional choice of whether the text labels should be shown by default.
    #[inline]
    pub fn with_show_labels(
        mut self,
        show_labels: impl Into<crate::components::ShowLabels>,
    ) -> Self {
        self.show_labels = try_serialize_field(Self::descriptor_show_labels(), [show_labels]);
        self
    }

    /// This method makes it possible to pack multiple [`crate::components::ShowLabels`] in a single component batch.
    ///
    /// This only makes sense when used in conjunction with [`Self::columns`]. [`Self::with_show_labels`] should
    /// be used when logging a single row's worth of data.
    #[inline]
    pub fn with_many_show_labels(
        mut self,
        show_labels: impl IntoIterator<Item = impl Into<crate::components::ShowLabels>>,
    ) -> Self {
        self.show_labels = try_serialize_field(Self::descriptor_show_labels(), show_labels);
        self
    }

    /// An optional floating point value that specifies the 2D drawing order of each line strip.
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

    /// Optional [`components::ClassId`][crate::components::ClassId]s for the lines.
    ///
    /// The [`components::ClassId`][crate::components::ClassId] provides colors and labels if not specified explicitly.
    #[inline]
    pub fn with_class_ids(
        mut self,
        class_ids: impl IntoIterator<Item = impl Into<crate::components::ClassId>>,
    ) -> Self {
        self.class_ids = try_serialize_field(Self::descriptor_class_ids(), class_ids);
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
}
