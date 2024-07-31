// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/leaf_transforms3d.fbs".

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
use ::re_types_core::ComponentName;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Archetype**: One or more transforms between the parent and the current entity which are *not* propagated in the transform hierarchy.
///
/// For transforms that are propagated in the transform hierarchy, see [`archetypes::Transform3D`][crate::archetypes::Transform3D].
///
/// If both [`archetypes::LeafTransforms3D`][crate::archetypes::LeafTransforms3D] and [`archetypes::Transform3D`][crate::archetypes::Transform3D] are present,
/// first the tree propagating [`archetypes::Transform3D`][crate::archetypes::Transform3D] is applied, then [`archetypes::LeafTransforms3D`][crate::archetypes::LeafTransforms3D].
///
/// Currently, most visualizers support only a single leaf transform per entity.
/// Check archetype documentations for details - if not otherwise specified, only the first leaf transform is applied.
///
/// From the point of view of the entity's coordinate system,
/// all components are applied in the inverse order they are listed here.
/// E.g. if both a translation and a max3x3 transform are present,
/// the 3x3 matrix is applied first, followed by the translation.
///
/// ## Example
///
/// ### Regular & leaf transform in tandom
/// ```ignore
/// use rerun::{
///     demo_util::grid,
///     external::{anyhow, glam},
/// };
///
/// fn main() -> anyhow::Result<()> {
///     let rec =
///         rerun::RecordingStreamBuilder::new("rerun_example_leaf_transform3d_combined").spawn()?;
///
///     rec.set_time_sequence("frame", 0);
///
///     // Log a box and points further down in the hierarchy.
///     rec.log(
///         "world/box",
///         &rerun::Boxes3D::from_half_sizes([[1.0, 1.0, 1.0]]),
///     )?;
///     rec.log(
///         "world/box/points",
///         &rerun::Points3D::new(grid(glam::Vec3::splat(-10.0), glam::Vec3::splat(10.0), 10)),
///     )?;
///
///     for i in 0..180 {
///         rec.set_time_sequence("frame", i);
///
///         // Log a regular transform which affects both the box and the points.
///         rec.log(
///             "world/box",
///             &rerun::Transform3D::from_rotation(rerun::RotationAxisAngle {
///                 axis: [0.0, 0.0, 1.0].into(),
///                 angle: rerun::Angle::from_degrees(i as f32 * 2.0),
///             }),
///         )?;
///
///         // Log an leaf transform which affects only the box.
///         let translation = [0.0, 0.0, (i as f32 * 0.1 - 5.0).abs() - 5.0];
///         rec.log(
///             "world/box",
///             &rerun::LeafTransforms3D::new().with_translations([translation]),
///         )?;
///     }
///
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/leaf_transform3d/41674f0082d6de489f8a1cd1583f60f6b5820ddf/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/leaf_transform3d/41674f0082d6de489f8a1cd1583f60f6b5820ddf/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/leaf_transform3d/41674f0082d6de489f8a1cd1583f60f6b5820ddf/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/leaf_transform3d/41674f0082d6de489f8a1cd1583f60f6b5820ddf/1200w.png">
///   <img src="https://static.rerun.io/leaf_transform3d/41674f0082d6de489f8a1cd1583f60f6b5820ddf/full.png" width="640">
/// </picture>
/// </center>
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LeafTransforms3D {
    /// Translation vectors.
    pub translations: Option<Vec<crate::components::LeafTranslation3D>>,

    /// Rotations via axis + angle.
    pub rotation_axis_angles: Option<Vec<crate::components::LeafRotationAxisAngle>>,

    /// Rotations via quaternion.
    pub quaternions: Option<Vec<crate::components::LeafRotationQuat>>,

    /// Scaling factors.
    pub scales: Option<Vec<crate::components::LeafScale3D>>,

    /// 3x3 transformation matrices.
    pub mat3x3: Option<Vec<crate::components::LeafTransformMat3x3>>,
}

impl ::re_types_core::SizeBytes for LeafTransforms3D {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.translations.heap_size_bytes()
            + self.rotation_axis_angles.heap_size_bytes()
            + self.quaternions.heap_size_bytes()
            + self.scales.heap_size_bytes()
            + self.mat3x3.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Option<Vec<crate::components::LeafTranslation3D>>>::is_pod()
            && <Option<Vec<crate::components::LeafRotationAxisAngle>>>::is_pod()
            && <Option<Vec<crate::components::LeafRotationQuat>>>::is_pod()
            && <Option<Vec<crate::components::LeafScale3D>>>::is_pod()
            && <Option<Vec<crate::components::LeafTransformMat3x3>>>::is_pod()
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.LeafTransforms3DIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 5usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.LeafTranslation3D".into(),
            "rerun.components.LeafRotationAxisAngle".into(),
            "rerun.components.LeafRotationQuat".into(),
            "rerun.components.LeafScale3D".into(),
            "rerun.components.LeafTransformMat3x3".into(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 6usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.LeafTransforms3DIndicator".into(),
            "rerun.components.LeafTranslation3D".into(),
            "rerun.components.LeafRotationAxisAngle".into(),
            "rerun.components.LeafRotationQuat".into(),
            "rerun.components.LeafScale3D".into(),
            "rerun.components.LeafTransformMat3x3".into(),
        ]
    });

impl LeafTransforms3D {
    /// The total number of components in the archetype: 0 required, 1 recommended, 5 optional
    pub const NUM_COMPONENTS: usize = 6usize;
}

/// Indicator component for the [`LeafTransforms3D`] [`::re_types_core::Archetype`]
pub type LeafTransforms3DIndicator = ::re_types_core::GenericIndicatorComponent<LeafTransforms3D>;

impl ::re_types_core::Archetype for LeafTransforms3D {
    type Indicator = LeafTransforms3DIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.archetypes.LeafTransforms3D".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Leaf transforms 3D"
    }

    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        static INDICATOR: LeafTransforms3DIndicator = LeafTransforms3DIndicator::DEFAULT;
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
    fn from_arrow_components(
        arrow_data: impl IntoIterator<Item = (ComponentName, Box<dyn arrow2::array::Array>)>,
    ) -> DeserializationResult<Self> {
        re_tracing::profile_function!();
        use ::re_types_core::{Loggable as _, ResultExt as _};
        let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
            .into_iter()
            .map(|(name, array)| (name.full_name(), array))
            .collect();
        let translations =
            if let Some(array) = arrays_by_name.get("rerun.components.LeafTranslation3D") {
                Some({
                    <crate::components::LeafTranslation3D>::from_arrow_opt(&**array)
                        .with_context("rerun.archetypes.LeafTransforms3D#translations")?
                        .into_iter()
                        .map(|v| v.ok_or_else(DeserializationError::missing_data))
                        .collect::<DeserializationResult<Vec<_>>>()
                        .with_context("rerun.archetypes.LeafTransforms3D#translations")?
                })
            } else {
                None
            };
        let rotation_axis_angles =
            if let Some(array) = arrays_by_name.get("rerun.components.LeafRotationAxisAngle") {
                Some({
                    <crate::components::LeafRotationAxisAngle>::from_arrow_opt(&**array)
                        .with_context("rerun.archetypes.LeafTransforms3D#rotation_axis_angles")?
                        .into_iter()
                        .map(|v| v.ok_or_else(DeserializationError::missing_data))
                        .collect::<DeserializationResult<Vec<_>>>()
                        .with_context("rerun.archetypes.LeafTransforms3D#rotation_axis_angles")?
                })
            } else {
                None
            };
        let quaternions =
            if let Some(array) = arrays_by_name.get("rerun.components.LeafRotationQuat") {
                Some({
                    <crate::components::LeafRotationQuat>::from_arrow_opt(&**array)
                        .with_context("rerun.archetypes.LeafTransforms3D#quaternions")?
                        .into_iter()
                        .map(|v| v.ok_or_else(DeserializationError::missing_data))
                        .collect::<DeserializationResult<Vec<_>>>()
                        .with_context("rerun.archetypes.LeafTransforms3D#quaternions")?
                })
            } else {
                None
            };
        let scales = if let Some(array) = arrays_by_name.get("rerun.components.LeafScale3D") {
            Some({
                <crate::components::LeafScale3D>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.LeafTransforms3D#scales")?
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.LeafTransforms3D#scales")?
            })
        } else {
            None
        };
        let mat3x3 = if let Some(array) = arrays_by_name.get("rerun.components.LeafTransformMat3x3")
        {
            Some({
                <crate::components::LeafTransformMat3x3>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.LeafTransforms3D#mat3x3")?
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.LeafTransforms3D#mat3x3")?
            })
        } else {
            None
        };
        Ok(Self {
            translations,
            rotation_axis_angles,
            quaternions,
            scales,
            mat3x3,
        })
    }
}

impl ::re_types_core::AsComponents for LeafTransforms3D {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            self.translations
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch).into()),
            self.rotation_axis_angles
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch).into()),
            self.quaternions
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch).into()),
            self.scales
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch).into()),
            self.mat3x3
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for LeafTransforms3D {}

impl LeafTransforms3D {
    /// Create a new `LeafTransforms3D`.
    #[inline]
    pub fn new() -> Self {
        Self {
            translations: None,
            rotation_axis_angles: None,
            quaternions: None,
            scales: None,
            mat3x3: None,
        }
    }

    /// Translation vectors.
    #[inline]
    pub fn with_translations(
        mut self,
        translations: impl IntoIterator<Item = impl Into<crate::components::LeafTranslation3D>>,
    ) -> Self {
        self.translations = Some(translations.into_iter().map(Into::into).collect());
        self
    }

    /// Rotations via axis + angle.
    #[inline]
    pub fn with_rotation_axis_angles(
        mut self,
        rotation_axis_angles: impl IntoIterator<
            Item = impl Into<crate::components::LeafRotationAxisAngle>,
        >,
    ) -> Self {
        self.rotation_axis_angles =
            Some(rotation_axis_angles.into_iter().map(Into::into).collect());
        self
    }

    /// Rotations via quaternion.
    #[inline]
    pub fn with_quaternions(
        mut self,
        quaternions: impl IntoIterator<Item = impl Into<crate::components::LeafRotationQuat>>,
    ) -> Self {
        self.quaternions = Some(quaternions.into_iter().map(Into::into).collect());
        self
    }

    /// Scaling factors.
    #[inline]
    pub fn with_scales(
        mut self,
        scales: impl IntoIterator<Item = impl Into<crate::components::LeafScale3D>>,
    ) -> Self {
        self.scales = Some(scales.into_iter().map(Into::into).collect());
        self
    }

    /// 3x3 transformation matrices.
    #[inline]
    pub fn with_mat3x3(
        mut self,
        mat3x3: impl IntoIterator<Item = impl Into<crate::components::LeafTransformMat3x3>>,
    ) -> Self {
        self.mat3x3 = Some(mat3x3.into_iter().map(Into::into).collect());
        self
    }
}
