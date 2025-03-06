// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/instance_poses3d.fbs".

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
use ::re_types_core::{ComponentBatch as _, SerializedComponentBatch};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Archetype**: One or more transforms between the current entity and its parent. Unlike [`archetypes::Transform3D`][crate::archetypes::Transform3D], it is *not* propagated in the transform hierarchy.
///
/// If both [`archetypes::InstancePoses3D`][crate::archetypes::InstancePoses3D] and [`archetypes::Transform3D`][crate::archetypes::Transform3D] are present,
/// first the tree propagating [`archetypes::Transform3D`][crate::archetypes::Transform3D] is applied, then [`archetypes::InstancePoses3D`][crate::archetypes::InstancePoses3D].
///
/// From the point of view of the entity's coordinate system,
/// all components are applied in the inverse order they are listed here.
/// E.g. if both a translation and a max3x3 transform are present,
/// the 3x3 matrix is applied first, followed by the translation.
///
/// Currently, many visualizers support only a single instance transform per entity.
/// Check archetype documentations for details - if not otherwise specified, only the first instance transform is applied.
/// Some visualizers like the mesh visualizer used for [`archetypes::Mesh3D`][crate::archetypes::Mesh3D],
/// will draw an object for every pose, a behavior also known as "instancing".
///
/// ## Example
///
/// ### Regular & instance transforms in tandem
/// ```ignore
/// use rerun::{
///     demo_util::grid,
///     external::{anyhow, glam},
/// };
///
/// fn main() -> anyhow::Result<()> {
///     let rec =
///         rerun::RecordingStreamBuilder::new("rerun_example_instance_pose3d_combined").spawn()?;
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
///         // Log an instance pose which affects only the box.
///         let translation = [0.0, 0.0, (i as f32 * 0.1 - 5.0).abs() - 5.0];
///         rec.log(
///             "world/box",
///             &rerun::InstancePoses3D::new().with_translations([translation]),
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
#[derive(Clone, Debug, PartialEq, Default)]
pub struct InstancePoses3D {
    /// Translation vectors.
    pub translations: Option<SerializedComponentBatch>,

    /// Rotations via axis + angle.
    pub rotation_axis_angles: Option<SerializedComponentBatch>,

    /// Rotations via quaternion.
    pub quaternions: Option<SerializedComponentBatch>,

    /// Scaling factors.
    pub scales: Option<SerializedComponentBatch>,

    /// 3x3 transformation matrices.
    pub mat3x3: Option<SerializedComponentBatch>,
}

impl InstancePoses3D {
    /// Returns the [`ComponentDescriptor`] for [`Self::translations`].
    #[inline]
    pub fn descriptor_translations() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.InstancePoses3D".into()),
            component_name: "rerun.components.PoseTranslation3D".into(),
            archetype_field_name: Some("translations".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::rotation_axis_angles`].
    #[inline]
    pub fn descriptor_rotation_axis_angles() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.InstancePoses3D".into()),
            component_name: "rerun.components.PoseRotationAxisAngle".into(),
            archetype_field_name: Some("rotation_axis_angles".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::quaternions`].
    #[inline]
    pub fn descriptor_quaternions() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.InstancePoses3D".into()),
            component_name: "rerun.components.PoseRotationQuat".into(),
            archetype_field_name: Some("quaternions".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::scales`].
    #[inline]
    pub fn descriptor_scales() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.InstancePoses3D".into()),
            component_name: "rerun.components.PoseScale3D".into(),
            archetype_field_name: Some("scales".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::mat3x3`].
    #[inline]
    pub fn descriptor_mat3x3() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.InstancePoses3D".into()),
            component_name: "rerun.components.PoseTransformMat3x3".into(),
            archetype_field_name: Some("mat3x3".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for the associated indicator component.
    #[inline]
    pub fn descriptor_indicator() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.InstancePoses3D".into()),
            component_name: "rerun.components.InstancePoses3DIndicator".into(),
            archetype_field_name: None,
        }
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| [InstancePoses3D::descriptor_indicator()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 5usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            InstancePoses3D::descriptor_translations(),
            InstancePoses3D::descriptor_rotation_axis_angles(),
            InstancePoses3D::descriptor_quaternions(),
            InstancePoses3D::descriptor_scales(),
            InstancePoses3D::descriptor_mat3x3(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 6usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            InstancePoses3D::descriptor_indicator(),
            InstancePoses3D::descriptor_translations(),
            InstancePoses3D::descriptor_rotation_axis_angles(),
            InstancePoses3D::descriptor_quaternions(),
            InstancePoses3D::descriptor_scales(),
            InstancePoses3D::descriptor_mat3x3(),
        ]
    });

impl InstancePoses3D {
    /// The total number of components in the archetype: 0 required, 1 recommended, 5 optional
    pub const NUM_COMPONENTS: usize = 6usize;
}

/// Indicator component for the [`InstancePoses3D`] [`::re_types_core::Archetype`]
pub type InstancePoses3DIndicator = ::re_types_core::GenericIndicatorComponent<InstancePoses3D>;

impl ::re_types_core::Archetype for InstancePoses3D {
    type Indicator = InstancePoses3DIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.archetypes.InstancePoses3D".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Instance poses 3D"
    }

    #[inline]
    fn indicator() -> SerializedComponentBatch {
        #[allow(clippy::unwrap_used)]
        InstancePoses3DIndicator::DEFAULT.serialized().unwrap()
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
        let translations = arrays_by_descr
            .get(&Self::descriptor_translations())
            .map(|array| {
                SerializedComponentBatch::new(array.clone(), Self::descriptor_translations())
            });
        let rotation_axis_angles = arrays_by_descr
            .get(&Self::descriptor_rotation_axis_angles())
            .map(|array| {
                SerializedComponentBatch::new(
                    array.clone(),
                    Self::descriptor_rotation_axis_angles(),
                )
            });
        let quaternions = arrays_by_descr
            .get(&Self::descriptor_quaternions())
            .map(|array| {
                SerializedComponentBatch::new(array.clone(), Self::descriptor_quaternions())
            });
        let scales = arrays_by_descr
            .get(&Self::descriptor_scales())
            .map(|array| SerializedComponentBatch::new(array.clone(), Self::descriptor_scales()));
        let mat3x3 = arrays_by_descr
            .get(&Self::descriptor_mat3x3())
            .map(|array| SerializedComponentBatch::new(array.clone(), Self::descriptor_mat3x3()));
        Ok(Self {
            translations,
            rotation_axis_angles,
            quaternions,
            scales,
            mat3x3,
        })
    }
}

impl ::re_types_core::AsComponents for InstancePoses3D {
    #[inline]
    fn as_serialized_batches(&self) -> Vec<SerializedComponentBatch> {
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            self.translations.clone(),
            self.rotation_axis_angles.clone(),
            self.quaternions.clone(),
            self.scales.clone(),
            self.mat3x3.clone(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for InstancePoses3D {}

impl InstancePoses3D {
    /// Create a new `InstancePoses3D`.
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

    /// Update only some specific fields of a `InstancePoses3D`.
    #[inline]
    pub fn update_fields() -> Self {
        Self::default()
    }

    /// Clear all the fields of a `InstancePoses3D`.
    #[inline]
    pub fn clear_fields() -> Self {
        use ::re_types_core::Loggable as _;
        Self {
            translations: Some(SerializedComponentBatch::new(
                crate::components::PoseTranslation3D::arrow_empty(),
                Self::descriptor_translations(),
            )),
            rotation_axis_angles: Some(SerializedComponentBatch::new(
                crate::components::PoseRotationAxisAngle::arrow_empty(),
                Self::descriptor_rotation_axis_angles(),
            )),
            quaternions: Some(SerializedComponentBatch::new(
                crate::components::PoseRotationQuat::arrow_empty(),
                Self::descriptor_quaternions(),
            )),
            scales: Some(SerializedComponentBatch::new(
                crate::components::PoseScale3D::arrow_empty(),
                Self::descriptor_scales(),
            )),
            mat3x3: Some(SerializedComponentBatch::new(
                crate::components::PoseTransformMat3x3::arrow_empty(),
                Self::descriptor_mat3x3(),
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
            self.translations
                .map(|translations| translations.partitioned(_lengths.clone()))
                .transpose()?,
            self.rotation_axis_angles
                .map(|rotation_axis_angles| rotation_axis_angles.partitioned(_lengths.clone()))
                .transpose()?,
            self.quaternions
                .map(|quaternions| quaternions.partitioned(_lengths.clone()))
                .transpose()?,
            self.scales
                .map(|scales| scales.partitioned(_lengths.clone()))
                .transpose()?,
            self.mat3x3
                .map(|mat3x3| mat3x3.partitioned(_lengths.clone()))
                .transpose()?,
        ];
        Ok(columns
            .into_iter()
            .flatten()
            .chain([::re_types_core::indicator_column::<Self>(
                _lengths.into_iter().count(),
            )?]))
    }

    /// Helper to partition the component data into unit-length sub-batches.
    ///
    /// This is semantically similar to calling [`Self::columns`] with `std::iter::take(1).repeat(n)`,
    /// where `n` is automatically guessed.
    #[inline]
    pub fn columns_of_unit_batches(
        self,
    ) -> SerializationResult<impl Iterator<Item = ::re_types_core::SerializedComponentColumn>> {
        let len_translations = self.translations.as_ref().map(|b| b.array.len());
        let len_rotation_axis_angles = self.rotation_axis_angles.as_ref().map(|b| b.array.len());
        let len_quaternions = self.quaternions.as_ref().map(|b| b.array.len());
        let len_scales = self.scales.as_ref().map(|b| b.array.len());
        let len_mat3x3 = self.mat3x3.as_ref().map(|b| b.array.len());
        let len = None
            .or(len_translations)
            .or(len_rotation_axis_angles)
            .or(len_quaternions)
            .or(len_scales)
            .or(len_mat3x3)
            .unwrap_or(0);
        self.columns(std::iter::repeat(1).take(len))
    }

    /// Translation vectors.
    #[inline]
    pub fn with_translations(
        mut self,
        translations: impl IntoIterator<Item = impl Into<crate::components::PoseTranslation3D>>,
    ) -> Self {
        self.translations = try_serialize_field(Self::descriptor_translations(), translations);
        self
    }

    /// Rotations via axis + angle.
    #[inline]
    pub fn with_rotation_axis_angles(
        mut self,
        rotation_axis_angles: impl IntoIterator<
            Item = impl Into<crate::components::PoseRotationAxisAngle>,
        >,
    ) -> Self {
        self.rotation_axis_angles = try_serialize_field(
            Self::descriptor_rotation_axis_angles(),
            rotation_axis_angles,
        );
        self
    }

    /// Rotations via quaternion.
    #[inline]
    pub fn with_quaternions(
        mut self,
        quaternions: impl IntoIterator<Item = impl Into<crate::components::PoseRotationQuat>>,
    ) -> Self {
        self.quaternions = try_serialize_field(Self::descriptor_quaternions(), quaternions);
        self
    }

    /// Scaling factors.
    #[inline]
    pub fn with_scales(
        mut self,
        scales: impl IntoIterator<Item = impl Into<crate::components::PoseScale3D>>,
    ) -> Self {
        self.scales = try_serialize_field(Self::descriptor_scales(), scales);
        self
    }

    /// 3x3 transformation matrices.
    #[inline]
    pub fn with_mat3x3(
        mut self,
        mat3x3: impl IntoIterator<Item = impl Into<crate::components::PoseTransformMat3x3>>,
    ) -> Self {
        self.mat3x3 = try_serialize_field(Self::descriptor_mat3x3(), mat3x3);
        self
    }
}

impl ::re_byte_size::SizeBytes for InstancePoses3D {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.translations.heap_size_bytes()
            + self.rotation_axis_angles.heap_size_bytes()
            + self.quaternions.heap_size_bytes()
            + self.scales.heap_size_bytes()
            + self.mat3x3.heap_size_bytes()
    }
}
