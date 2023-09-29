// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/archetypes/line_strips3d.fbs".

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
/// ### Simple example
/// ```ignore
/// //! Log a simple line strip.
///
/// use rerun::{archetypes::LineStrips3D, RecordingStreamBuilder};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (rec, storage) = RecordingStreamBuilder::new("rerun_example_line_strip3d").memory()?;
///
///     let points = [
///         [0., 0., 0.],
///         [0., 0., 1.],
///         [1., 0., 0.],
///         [1., 0., 1.],
///         [1., 1., 0.],
///         [1., 1., 1.],
///         [0., 1., 0.],
///         [0., 1., 1.],
///     ];
///     rec.log("strip", &LineStrips3D::new([points]))?;
///
///     rerun::native_viewer::show(storage.take())?;
///     Ok(())
/// }
/// ```
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/line_strip3d_simple/13036c0e71f78d3cec37d5724f97b47c4cf3c429/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/line_strip3d_simple/13036c0e71f78d3cec37d5724f97b47c4cf3c429/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/line_strip3d_simple/13036c0e71f78d3cec37d5724f97b47c4cf3c429/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/line_strip3d_simple/13036c0e71f78d3cec37d5724f97b47c4cf3c429/1200w.png">
///   <img src="https://static.rerun.io/line_strip3d_simple/13036c0e71f78d3cec37d5724f97b47c4cf3c429/full.png">
/// </picture>
///
/// ### Many individual segments
/// ```ignore
/// //! Log a simple set of line segments.
///
/// use rerun::{archetypes::LineStrips3D, RecordingStreamBuilder};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (rec, storage) = RecordingStreamBuilder::new("rerun_example_line_segments3d").memory()?;
///
///     let points = [
///         [0., 0., 0.],
///         [0., 0., 1.],
///         [1., 0., 0.],
///         [1., 0., 1.],
///         [1., 1., 0.],
///         [1., 1., 1.],
///         [0., 1., 0.],
///         [0., 1., 1.],
///     ];
///     rec.log("segments", &LineStrips3D::new(points.chunks(2)))?;
///
///     rerun::native_viewer::show(storage.take())?;
///     Ok(())
/// }
/// ```
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/line_segment3d_simple/aa800b2a6e6a7b8e32e762b42861bae36f5014bb/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/line_segment3d_simple/aa800b2a6e6a7b8e32e762b42861bae36f5014bb/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/line_segment3d_simple/aa800b2a6e6a7b8e32e762b42861bae36f5014bb/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/line_segment3d_simple/aa800b2a6e6a7b8e32e762b42861bae36f5014bb/1200w.png">
///   <img src="https://static.rerun.io/line_segment3d_simple/aa800b2a6e6a7b8e32e762b42861bae36f5014bb/full.png">
/// </picture>
///
/// ### Many strips
/// ```ignore
/// //! Log a batch of 2d line strips.
///
/// use rerun::{archetypes::LineStrips3D, RecordingStreamBuilder};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (rec, storage) = RecordingStreamBuilder::new("rerun_example_line_strip3d").memory()?;
///
///     let strip1 = [[0., 0., 2.], [1., 0., 2.], [1., 1., 2.], [0., 1., 2.]];
///     let strip2 = [
///         [0., 0., 0.],
///         [0., 0., 1.],
///         [1., 0., 0.],
///         [1., 0., 1.],
///         [1., 1., 0.],
///         [1., 1., 1.],
///         [0., 1., 0.],
///         [0., 1., 1.],
///     ];
///     rec.log(
///         "strips",
///         &LineStrips3D::new([strip1.to_vec(), strip2.to_vec()])
///             .with_colors([0xFF0000FF, 0x00FF00FF])
///             .with_radii([0.025, 0.005])
///             .with_labels(["one strip here", "and one strip there"]),
///     )?;
///
///     rerun::native_viewer::show(storage.take())?;
///     Ok(())
/// }
/// ```
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/line_strip3d_batch/102e5ec5271475657fbc76b469267e4ec8e84337/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/line_strip3d_batch/102e5ec5271475657fbc76b469267e4ec8e84337/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/line_strip3d_batch/102e5ec5271475657fbc76b469267e4ec8e84337/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/line_strip3d_batch/102e5ec5271475657fbc76b469267e4ec8e84337/1200w.png">
///   <img src="https://static.rerun.io/line_strip3d_batch/102e5ec5271475657fbc76b469267e4ec8e84337/full.png">
/// </picture>
#[derive(Clone, Debug, PartialEq)]
pub struct LineStrips3D {
    /// All the actual 3D line strips that make up the batch.
    pub strips: Vec<crate::components::LineStrip3D>,

    /// Optional radii for the line strips.
    pub radii: Option<Vec<crate::components::Radius>>,

    /// Optional colors for the line strips.
    pub colors: Option<Vec<crate::components::Color>>,

    /// Optional text labels for the line strips.
    pub labels: Option<Vec<crate::components::Text>>,

    /// Optional `ClassId`s for the lines.
    ///
    /// The class ID provides colors and labels if not specified explicitly.
    pub class_ids: Option<Vec<crate::components::ClassId>>,

    /// Unique identifiers for each individual line strip in the batch.
    pub instance_keys: Option<Vec<crate::components::InstanceKey>>,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.LineStrip3D".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.Color".into(),
            "rerun.components.LineStrips3DIndicator".into(),
            "rerun.components.Radius".into(),
        ]
    });

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.ClassId".into(),
            "rerun.components.InstanceKey".into(),
            "rerun.components.Text".into(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 7usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.LineStrip3D".into(),
            "rerun.components.Color".into(),
            "rerun.components.LineStrips3DIndicator".into(),
            "rerun.components.Radius".into(),
            "rerun.components.ClassId".into(),
            "rerun.components.InstanceKey".into(),
            "rerun.components.Text".into(),
        ]
    });

impl LineStrips3D {
    pub const NUM_COMPONENTS: usize = 7usize;
}

/// Indicator component for the [`LineStrips3D`] [`crate::Archetype`]
pub type LineStrips3DIndicator = crate::GenericIndicatorComponent<LineStrips3D>;

impl crate::Archetype for LineStrips3D {
    type Indicator = LineStrips3DIndicator;

    #[inline]
    fn name() -> crate::ArchetypeName {
        "rerun.archetypes.LineStrips3D".into()
    }

    #[inline]
    fn indicator() -> crate::MaybeOwnedComponentBatch<'static> {
        static INDICATOR: LineStrips3DIndicator = LineStrips3DIndicator::DEFAULT;
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
        let strips = {
            let array = arrays_by_name
                .get("rerun.components.LineStrip3D")
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.LineStrips3D#strips")?;
            <crate::components::LineStrip3D>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.LineStrips3D#strips")?
                .into_iter()
                .map(|v| v.ok_or_else(crate::DeserializationError::missing_data))
                .collect::<crate::DeserializationResult<Vec<_>>>()
                .with_context("rerun.archetypes.LineStrips3D#strips")?
        };
        let radii = if let Some(array) = arrays_by_name.get("rerun.components.Radius") {
            Some({
                <crate::components::Radius>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.LineStrips3D#radii")?
                    .into_iter()
                    .map(|v| v.ok_or_else(crate::DeserializationError::missing_data))
                    .collect::<crate::DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.LineStrips3D#radii")?
            })
        } else {
            None
        };
        let colors = if let Some(array) = arrays_by_name.get("rerun.components.Color") {
            Some({
                <crate::components::Color>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.LineStrips3D#colors")?
                    .into_iter()
                    .map(|v| v.ok_or_else(crate::DeserializationError::missing_data))
                    .collect::<crate::DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.LineStrips3D#colors")?
            })
        } else {
            None
        };
        let labels = if let Some(array) = arrays_by_name.get("rerun.components.Text") {
            Some({
                <crate::components::Text>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.LineStrips3D#labels")?
                    .into_iter()
                    .map(|v| v.ok_or_else(crate::DeserializationError::missing_data))
                    .collect::<crate::DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.LineStrips3D#labels")?
            })
        } else {
            None
        };
        let class_ids = if let Some(array) = arrays_by_name.get("rerun.components.ClassId") {
            Some({
                <crate::components::ClassId>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.LineStrips3D#class_ids")?
                    .into_iter()
                    .map(|v| v.ok_or_else(crate::DeserializationError::missing_data))
                    .collect::<crate::DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.LineStrips3D#class_ids")?
            })
        } else {
            None
        };
        let instance_keys = if let Some(array) = arrays_by_name.get("rerun.components.InstanceKey")
        {
            Some({
                <crate::components::InstanceKey>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.LineStrips3D#instance_keys")?
                    .into_iter()
                    .map(|v| v.ok_or_else(crate::DeserializationError::missing_data))
                    .collect::<crate::DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.LineStrips3D#instance_keys")?
            })
        } else {
            None
        };
        Ok(Self {
            strips,
            radii,
            colors,
            labels,
            class_ids,
            instance_keys,
        })
    }
}

impl crate::AsComponents for LineStrips3D {
    fn as_component_batches(&self) -> Vec<crate::MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
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

impl LineStrips3D {
    pub fn new(
        strips: impl IntoIterator<Item = impl Into<crate::components::LineStrip3D>>,
    ) -> Self {
        Self {
            strips: strips.into_iter().map(Into::into).collect(),
            radii: None,
            colors: None,
            labels: None,
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
