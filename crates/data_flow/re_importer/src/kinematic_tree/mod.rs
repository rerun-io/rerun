//! The interface shared by the robot model trees ([`crate::UrdfTree`], [`crate::MjcfTree`]).

#[cfg(any(feature = "mjcf", feature = "urdf"))]
mod log_paths;

use std::sync::Arc;

use anyhow::{Context as _, bail};
use arrow::array::{Array as _, Float64Array, ListArray, StringArray, StructArray};
use arrow::buffer::OffsetBuffer;
use arrow::compute::cast;
use arrow::datatypes::{DataType, Field, Fields};
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::{Chunk, TimePoint};
use re_sdk_types::ToArrow as _;
use re_sdk_types::archetypes::Transform3D;
use re_sdk_types::encodings::{Quaternion, Vec3D};
use re_sdk_types::external::glam::{Quat, Vec3};

#[cfg(any(feature = "mjcf", feature = "urdf"))]
pub use self::log_paths::{LogPaths, emit_archetype, emit_static_transforms_batch};

/// The transform from a parent frame to a child frame, for some set of joint values.
#[derive(Clone, Debug, PartialEq)]
pub struct JointTransform {
    /// The child frame's origin, in the parent frame.
    pub translation: Vec3,

    /// The child frame's rotation relative to the parent frame.
    pub quaternion: Quat,

    /// The frame ID of the parent, including the model's frame prefix if it was applied.
    pub parent_frame: String,

    /// The frame ID of the child, including the model's frame prefix if it was applied.
    pub child_frame: String,

    /// E.g. that a value outside the joint's limits was clamped.
    pub warnings: Vec<String>,
}

impl JointTransform {
    /// Prepends `frame_prefix`, if set, to both frame IDs.
    pub fn with_frame_prefix(mut self, frame_prefix: Option<&str>) -> Self {
        self.parent_frame = prefixed_frame(frame_prefix, &self.parent_frame);
        self.child_frame = prefixed_frame(frame_prefix, &self.child_frame);
        self
    }
}

fn prefixed_frame(frame_prefix: Option<&str>, name: &str) -> String {
    match frame_prefix {
        Some(prefix) => format!("{prefix}{name}"),
        None => name.to_owned(),
    }
}

impl From<JointTransform> for Transform3D {
    fn from(transform: JointTransform) -> Self {
        Self::update_fields()
            .with_translation(transform.translation.to_array())
            .with_quaternion(transform.quaternion.to_array())
            .with_parent_frame(transform.parent_frame)
            .with_child_frame(transform.child_frame)
    }
}

/// The joint's type has no single value to pose it with, e.g. a ball or floating joint.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
#[error("Joint {joint:?} has type {joint_type}, which is not supported for transform computation")]
pub struct UnsupportedJointType {
    /// The name of the joint.
    pub joint: String,

    /// The joint's type.
    pub joint_type: MultiDofJointType,
}

/// A joint type with more than one degree of freedom, so that no single value can pose it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, strum::Display)]
#[strum(serialize_all = "lowercase")]
pub enum MultiDofJointType {
    /// Translates and rotates freely along all three axes, like URDF `floating` or MJCF `free`.
    Floating,

    /// Translates within a plane and rotates about its normal, like URDF `planar`.
    Planar,

    /// Rotates freely about all three axes, like URDF `spherical` or MJCF `ball`.
    Spherical,
}

/// The file format a robot model was loaded from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, strum::Display)]
#[strum(serialize_all = "UPPERCASE")]
pub enum ModelFormat {
    /// [Unified Robot Description Format](https://en.wikipedia.org/wiki/URDF), common in ROS.
    Urdf,

    /// [MuJoCo's XML model format](https://mujoco.readthedocs.io/en/stable/XMLreference.html).
    Mjcf,
}

/// A robot model whose frames can be posed from joint values.
pub trait KinematicTree: Send + Sync {
    /// The file format the model was loaded from.
    fn model_format(&self) -> ModelFormat;

    /// The prefix prepended to every frame ID of the model, if set.
    fn frame_prefix(&self) -> Option<&str>;

    /// Applies [`Self::frame_prefix`] to the given frame name, if set.
    fn apply_frame_prefix(&self, name: &str) -> String {
        prefixed_frame(self.frame_prefix(), name)
    }

    /// Emits the model (geometry, and static joint transforms if `include_joint_transforms`) as [`Chunk`]s.
    fn emit(
        &self,
        emit: &mut dyn FnMut(Chunk),
        timepoint: &TimePoint,
        include_joint_transforms: bool,
    ) -> anyhow::Result<()>;

    /// Computes the transforms that pose the model for one set of `(joint name, value)` pairs.
    ///
    /// Values are angles in radians or distances in meters.
    /// Returns one transform per named joint, in order, each moving a frame of its own.
    /// If `clamp` is true, values outside joint limits are clamped and a warning is added to the transform.
    /// A joint that cannot be posed fails with an [`UnsupportedJointType`] that callers can downcast to.
    fn compute_joint_transforms(
        &self,
        joint_values: &[(&str, f64)],
        clamp: bool,
    ) -> anyhow::Result<Vec<JointTransform>>;

    /// Computes batches of 3D transform components from Arrow list arrays containing joint names and values.
    ///
    /// `names` must be a `ListArray` with `Utf8` values.
    /// `values` must be a `ListArray` with values castable to `Float64`.
    ///
    /// The output is a `ListArray` with `translation`, `quaternion`, `parent_frame`, and
    /// `child_frame` fields and the same outer row count as the inputs.
    /// Each output row holds the transforms that [`Self::compute_joint_transforms`] returns for the input row.
    ///
    /// Warnings are logged once per call, so that a long batch of clamped values does not flood the log.
    ///
    /// Note: this is intended as a helper for lens pipelines, where you would usually pipe this output
    /// through an additional lens that scatters each batch into final [`Transform3D`] rows.
    fn compute_joint_transform_batches(
        &self,
        names: &ListArray,
        values: &ListArray,
        clamp: bool,
    ) -> anyhow::Result<ListArray> {
        if names.len() != values.len() {
            bail!(
                "joint name and value arrays must have the same number of rows, got {} and {}",
                names.len(),
                values.len()
            );
        }

        let joint_names = names
            .values()
            .try_downcast_array_ref::<StringArray>()
            .with_context(|| {
                format!(
                    "joint names must be list<utf8>, got {:?}",
                    names.data_type()
                )
            })?;

        let joint_values = cast(values.values().as_ref(), &DataType::Float64)
            .context("failed to cast joint values to float64")?
            .try_downcast_array::<Float64Array>()?;

        let mut offsets = Vec::<i32>::with_capacity(names.len() + 1);
        let mut translations = Vec::<Vec3D>::new();
        let mut quaternions = Vec::<Quaternion>::new();
        let mut parent_frames = Vec::<String>::new();
        let mut child_frames = Vec::<String>::new();
        let mut row_values = Vec::new();
        let mut first_warning = None;
        let mut num_warnings = 0;

        offsets.push(0);

        for row in 0..names.len() {
            if names.is_null(row) || values.is_null(row) {
                bail!(
                    "joint name and value lists must not contain null rows, got null at row {row}"
                );
            }

            let names_start = names.value_offsets()[row] as usize;
            let names_end = names.value_offsets()[row + 1] as usize;
            let values_start = values.value_offsets()[row] as usize;
            let values_end = values.value_offsets()[row + 1] as usize;

            let num_names = names_end - names_start;
            let num_values = values_end - values_start;
            if num_names != num_values {
                bail!(
                    "joint name and value lists must have the same length at row {row}, got {num_names} and {num_values}"
                );
            }

            row_values.clear();
            for (name_index, value_index) in
                std::iter::zip(names_start..names_end, values_start..values_end)
            {
                if joint_names.is_null(name_index) || joint_values.is_null(value_index) {
                    bail!(
                        "joint name and value lists must not contain null values, got null at row {row}"
                    );
                }
                row_values.push((
                    joint_names.value(name_index),
                    joint_values.value(value_index),
                ));
            }

            for transform in self.compute_joint_transforms(&row_values, clamp)? {
                num_warnings += transform.warnings.len();
                if first_warning.is_none() {
                    first_warning = transform.warnings.into_iter().next();
                }
                translations.push(Vec3D(transform.translation.to_array()));
                quaternions.push(Quaternion(transform.quaternion.to_array()));
                parent_frames.push(transform.parent_frame);
                child_frames.push(transform.child_frame);
            }

            offsets.push(
                i32::try_from(parent_frames.len())
                    .context("too many joint transforms for Arrow list offsets")?,
            );
        }

        if let Some(warning) = first_warning {
            match num_warnings - 1 {
                0 => re_log::warn!("{warning}"),
                more => re_log::warn!("{warning} ({more} more warnings in this batch)"),
            }
        }

        let translation_array = Vec3D::to_arrow(translations.iter())?;
        let quaternion_array = Quaternion::to_arrow(quaternions.iter())?;

        let struct_fields = Fields::from(vec![
            Field::new("translation", translation_array.data_type().clone(), false),
            Field::new("quaternion", quaternion_array.data_type().clone(), false),
            Field::new("parent_frame", DataType::Utf8, false),
            Field::new("child_frame", DataType::Utf8, false),
        ]);

        let struct_array = StructArray::try_new(
            struct_fields.clone(),
            vec![
                translation_array,
                quaternion_array,
                Arc::new(StringArray::from(parent_frames)),
                Arc::new(StringArray::from(child_frames)),
            ],
            None,
        )?;

        Ok(ListArray::new(
            Arc::new(Field::new_list_field(
                DataType::Struct(struct_fields),
                false,
            )),
            OffsetBuffer::new(offsets.into()),
            Arc::new(struct_array),
            None,
        ))
    }
}
