// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/archetypes/line_strips2d.fbs".

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

/// A batch of line strips with positions and optional colors, radii, labels, etc.
///
/// ## Examples
///
/// ```ignore
/// //! Log a simple line strip.
///
/// use rerun::{
///     archetypes::{Boxes2D, LineStrips2D},
///     RecordingStreamBuilder,
/// };
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (rec, storage) = RecordingStreamBuilder::new("rerun_example_line_strip2d").memory()?;
///
///     let points = [[0., 0.], [2., 1.], [4., -1.], [6., 0.]];
///     rec.log("strip", &LineStrips2D::new([points]))?;
///
///     // Log an extra rect to set the view bounds
///     rec.log(
///         "bounds",
///         &Boxes2D::from_centers_and_sizes([(3., 0.)], [(8., 6.)]),
///     )?;
///
///     rerun::native_viewer::show(storage.take())?;
///     Ok(())
/// }
/// ```
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/line_strip2d_simple/c4e6ce937544e66b497450fd64ac3ac2f244f0e1/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/line_strip2d_simple/c4e6ce937544e66b497450fd64ac3ac2f244f0e1/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/line_strip2d_simple/c4e6ce937544e66b497450fd64ac3ac2f244f0e1/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/line_strip2d_simple/c4e6ce937544e66b497450fd64ac3ac2f244f0e1/1200w.png">
///   <img src="https://static.rerun.io/line_strip2d_simple/c4e6ce937544e66b497450fd64ac3ac2f244f0e1/full.png">
/// </picture>
///
/// ```ignore
/// //! Log a couple 2D line segments using 2D line strips.
///
/// use rerun::{
///     archetypes::{Boxes2D, LineStrips2D},
///     RecordingStreamBuilder,
/// };
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (rec, storage) = RecordingStreamBuilder::new("rerun_example_line_segments2d").memory()?;
///
///     let points = [[0., 0.], [2., 1.], [4., -1.], [6., 0.]];
///     rec.log("segments", &LineStrips2D::new(points.chunks(2)))?;
///
///     // Log an extra rect to set the view bounds
///     rec.log(
///         "bounds",
///         &Boxes2D::from_centers_and_sizes([(3.0, 0.0)], [(8.0, 6.0)]),
///     )?;
///
///     rerun::native_viewer::show(storage.take())?;
///     Ok(())
/// }
/// ```
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/line_segment2d_simple/53df596662dd9ffaaea5d09d091ef95220346c83/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/line_segment2d_simple/53df596662dd9ffaaea5d09d091ef95220346c83/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/line_segment2d_simple/53df596662dd9ffaaea5d09d091ef95220346c83/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/line_segment2d_simple/53df596662dd9ffaaea5d09d091ef95220346c83/1200w.png">
///   <img src="https://static.rerun.io/line_segment2d_simple/53df596662dd9ffaaea5d09d091ef95220346c83/full.png">
/// </picture>
///
/// ```ignore
/// //! Log a batch of 2d line strips.
///
/// use rerun::{
///     archetypes::{Boxes2D, LineStrips2D},
///     RecordingStreamBuilder,
/// };
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (rec, storage) = RecordingStreamBuilder::new("rerun_example_line_strip2d").memory()?;
///
///     let strip1 = [[0., 0.], [2., 1.], [4., -1.], [6., 0.]];
///     #[rustfmt::skip]
///     let strip2 = [[0., 3.], [1., 4.], [2., 2.], [3., 4.], [4., 2.], [5., 4.], [6., 3.]];
///     rec.log(
///         "strips",
///         &LineStrips2D::new([strip1.to_vec(), strip2.to_vec()])
///             .with_colors([0xFF0000FF, 0x00FF00FF])
///             .with_radii([0.025, 0.005])
///             .with_labels(["one strip here", "and one strip there"]),
///     )?;
///
///     // Log an extra rect to set the view bounds
///     rec.log(
///         "bounds",
///         &Boxes2D::from_centers_and_sizes([(3.0, 1.5)], [(8.0, 9.0)]),
///     )?;
///
///     rerun::native_viewer::show(storage.take())?;
///     Ok(())
/// }
/// ```
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/line_strip2d_batch/d8aae7ca3d6c3b0e3b636de60b8067fa2f0b6db9/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/line_strip2d_batch/d8aae7ca3d6c3b0e3b636de60b8067fa2f0b6db9/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/line_strip2d_batch/d8aae7ca3d6c3b0e3b636de60b8067fa2f0b6db9/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/line_strip2d_batch/d8aae7ca3d6c3b0e3b636de60b8067fa2f0b6db9/1200w.png">
///   <img src="https://static.rerun.io/line_strip2d_batch/d8aae7ca3d6c3b0e3b636de60b8067fa2f0b6db9/full.png">
/// </picture>
#[derive(Clone, Debug, PartialEq)]
pub struct LineStrips2D {
    /// All the actual 2D line strips that make up the batch.
    pub strips: Vec<crate::components::LineStrip2D>,

    /// Optional radii for the line strips.
    pub radii: Option<Vec<crate::components::Radius>>,

    /// Optional colors for the line strips.
    pub colors: Option<Vec<crate::components::Color>>,

    /// Optional text labels for the line strips.
    pub labels: Option<Vec<crate::components::Text>>,

    /// An optional floating point value that specifies the 2D drawing order of each line strip.
    /// Objects with higher values are drawn on top of those with lower values.
    pub draw_order: Option<crate::components::DrawOrder>,

    /// Optional `ClassId`s for the lines.
    ///
    /// The class ID provides colors and labels if not specified explicitly.
    pub class_ids: Option<Vec<crate::components::ClassId>>,

    /// Unique identifiers for each individual line strip in the batch.
    pub instance_keys: Option<Vec<crate::components::InstanceKey>>,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.LineStrip2D".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.Color".into(),
            "rerun.components.LineStrips2DIndicator".into(),
            "rerun.components.Radius".into(),
        ]
    });

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 4usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.ClassId".into(),
            "rerun.components.DrawOrder".into(),
            "rerun.components.InstanceKey".into(),
            "rerun.components.Text".into(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 8usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.LineStrip2D".into(),
            "rerun.components.Color".into(),
            "rerun.components.LineStrips2DIndicator".into(),
            "rerun.components.Radius".into(),
            "rerun.components.ClassId".into(),
            "rerun.components.DrawOrder".into(),
            "rerun.components.InstanceKey".into(),
            "rerun.components.Text".into(),
        ]
    });

impl LineStrips2D {
    pub const NUM_COMPONENTS: usize = 8usize;
}

/// Indicator component for the [`LineStrips2D`] [`crate::Archetype`]
pub type LineStrips2DIndicator = crate::GenericIndicatorComponent<LineStrips2D>;

impl crate::Archetype for LineStrips2D {
    type Indicator = LineStrips2DIndicator;

    #[inline]
    fn name() -> crate::ArchetypeName {
        "rerun.archetypes.LineStrips2D".into()
    }

    #[inline]
    fn indicator() -> crate::MaybeOwnedComponentBatch<'static> {
        static INDICATOR: LineStrips2DIndicator = LineStrips2DIndicator::DEFAULT;
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
        use crate::{Loggable as _, ResultExt as _};
        let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
            .into_iter()
            .map(|(field, array)| (field.name, array))
            .collect();
        let strips = {
            let array = arrays_by_name
                .get("rerun.components.LineStrip2D")
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.LineStrips2D#strips")?;
            <crate::components::LineStrip2D>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.LineStrips2D#strips")?
                .into_iter()
                .map(|v| v.ok_or_else(crate::DeserializationError::missing_data))
                .collect::<crate::DeserializationResult<Vec<_>>>()
                .with_context("rerun.archetypes.LineStrips2D#strips")?
        };
        let radii = if let Some(array) = arrays_by_name.get("rerun.components.Radius") {
            Some({
                <crate::components::Radius>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.LineStrips2D#radii")?
                    .into_iter()
                    .map(|v| v.ok_or_else(crate::DeserializationError::missing_data))
                    .collect::<crate::DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.LineStrips2D#radii")?
            })
        } else {
            None
        };
        let colors = if let Some(array) = arrays_by_name.get("rerun.components.Color") {
            Some({
                <crate::components::Color>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.LineStrips2D#colors")?
                    .into_iter()
                    .map(|v| v.ok_or_else(crate::DeserializationError::missing_data))
                    .collect::<crate::DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.LineStrips2D#colors")?
            })
        } else {
            None
        };
        let labels = if let Some(array) = arrays_by_name.get("rerun.components.Text") {
            Some({
                <crate::components::Text>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.LineStrips2D#labels")?
                    .into_iter()
                    .map(|v| v.ok_or_else(crate::DeserializationError::missing_data))
                    .collect::<crate::DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.LineStrips2D#labels")?
            })
        } else {
            None
        };
        let draw_order = if let Some(array) = arrays_by_name.get("rerun.components.DrawOrder") {
            Some({
                <crate::components::DrawOrder>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.LineStrips2D#draw_order")?
                    .into_iter()
                    .next()
                    .flatten()
                    .ok_or_else(crate::DeserializationError::missing_data)
                    .with_context("rerun.archetypes.LineStrips2D#draw_order")?
            })
        } else {
            None
        };
        let class_ids = if let Some(array) = arrays_by_name.get("rerun.components.ClassId") {
            Some({
                <crate::components::ClassId>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.LineStrips2D#class_ids")?
                    .into_iter()
                    .map(|v| v.ok_or_else(crate::DeserializationError::missing_data))
                    .collect::<crate::DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.LineStrips2D#class_ids")?
            })
        } else {
            None
        };
        let instance_keys = if let Some(array) = arrays_by_name.get("rerun.components.InstanceKey")
        {
            Some({
                <crate::components::InstanceKey>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.LineStrips2D#instance_keys")?
                    .into_iter()
                    .map(|v| v.ok_or_else(crate::DeserializationError::missing_data))
                    .collect::<crate::DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.LineStrips2D#instance_keys")?
            })
        } else {
            None
        };
        Ok(Self {
            strips,
            radii,
            colors,
            labels,
            draw_order,
            class_ids,
            instance_keys,
        })
    }
}

impl crate::AsComponents for LineStrips2D {
    fn as_component_batches(&self) -> Vec<crate::MaybeOwnedComponentBatch<'_>> {
        use crate::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.strips as &dyn crate::ComponentBatch).into()),
            self.radii
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn crate::ComponentBatch).into()),
            self.colors
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn crate::ComponentBatch).into()),
            self.labels
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn crate::ComponentBatch).into()),
            self.draw_order
                .as_ref()
                .map(|comp| (comp as &dyn crate::ComponentBatch).into()),
            self.class_ids
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn crate::ComponentBatch).into()),
            self.instance_keys
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn crate::ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    #[inline]
    fn num_instances(&self) -> usize {
        self.strips.len()
    }
}

impl LineStrips2D {
    pub fn new(
        strips: impl IntoIterator<Item = impl Into<crate::components::LineStrip2D>>,
    ) -> Self {
        Self {
            strips: strips.into_iter().map(Into::into).collect(),
            radii: None,
            colors: None,
            labels: None,
            draw_order: None,
            class_ids: None,
            instance_keys: None,
        }
    }

    pub fn with_radii(
        mut self,
        radii: impl IntoIterator<Item = impl Into<crate::components::Radius>>,
    ) -> Self {
        self.radii = Some(radii.into_iter().map(Into::into).collect());
        self
    }

    pub fn with_colors(
        mut self,
        colors: impl IntoIterator<Item = impl Into<crate::components::Color>>,
    ) -> Self {
        self.colors = Some(colors.into_iter().map(Into::into).collect());
        self
    }

    pub fn with_labels(
        mut self,
        labels: impl IntoIterator<Item = impl Into<crate::components::Text>>,
    ) -> Self {
        self.labels = Some(labels.into_iter().map(Into::into).collect());
        self
    }

    pub fn with_draw_order(mut self, draw_order: impl Into<crate::components::DrawOrder>) -> Self {
        self.draw_order = Some(draw_order.into());
        self
    }

    pub fn with_class_ids(
        mut self,
        class_ids: impl IntoIterator<Item = impl Into<crate::components::ClassId>>,
    ) -> Self {
        self.class_ids = Some(class_ids.into_iter().map(Into::into).collect());
        self
    }

    pub fn with_instance_keys(
        mut self,
        instance_keys: impl IntoIterator<Item = impl Into<crate::components::InstanceKey>>,
    ) -> Self {
        self.instance_keys = Some(instance_keys.into_iter().map(Into::into).collect());
        self
    }
}
