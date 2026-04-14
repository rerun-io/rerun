//! Utilities for computing joint transforms with an URDF.

use re_sdk_types::archetypes::Transform3D;
use re_sdk_types::external::glam;
use urdf_rs::{Joint, JointType};

use super::quat_from_rpy;

/// Errors that can occur when computing a joint transform.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// The joint type is not supported for transform computation.
    #[error("Joint type '{0:?}' is not supported for transform computation")]
    UnsupportedJointType(JointType),
}

/// Computes a [`Transform3D`] for a joint at the given value.
///
/// `value` is either an angle in radians (revolute/continuous joint)
/// or a distance in meters (prismatic joint).
///
/// If `clamp` is true, values outside joint limits will be clamped and a warning is logged.
/// If `clamp` is false, values outside limits are used as-is without warnings.
pub fn compute_transform3d(joint: &Joint, value: f64, clamp: bool) -> Result<Transform3D, Error> {
    let result = internal::compute_joint_transform(joint, value, clamp)?;

    if let Some(warning) = &result.warning {
        re_log::warn!("{}", warning);
    }

    Ok(Transform3D::update_fields()
        .with_translation(result.translation.to_array())
        .with_quaternion(result.quaternion.to_array())
        .with_parent_frame(result.parent_frame)
        .with_child_frame(result.child_frame))
}

/// Internal utilities for joint transform computation.
// Note: these are exposed for use in bindings.
pub mod internal {

    use super::{Error, Joint, JointType, glam, quat_from_rpy};

    /// Internal result of computing a joint transform.
    ///
    /// Uses glam types for easier use in bindings.
    pub struct JointTransform {
        pub quaternion: glam::Quat,
        pub translation: glam::Vec3,
        pub parent_frame: String,
        pub child_frame: String,

        /// Optional warning message (e.g., if angle was clamped to limits).
        pub warning: Option<String>,
    }

    /// Computes a [`JointTransform`] for a joint at the given value.
    ///
    /// `value` is either an angle in radians (revolute/continuous joint)
    /// or a distance in meters (prismatic joint).
    ///
    /// If `clamp` is true, values outside joint limits will be clamped and a warning is generated.
    /// If `clamp` is false, values outside limits are used as-is without warnings.
    pub fn compute_joint_transform(
        joint: &Joint,
        value: f64,
        clamp: bool,
    ) -> Result<JointTransform, Error> {
        let Joint {
            name,
            joint_type,
            origin,
            parent,
            child,
            axis,
            limit,
            calibration: _,
            dynamics: _,
            mimic: _,
            safety_controller: _,
        } = joint;

        let urdf_rs::Pose {
            xyz: origin_xyz,
            rpy: origin_rpy,
        } = origin;

        let parent_frame = parent.link.clone();
        let child_frame = child.link.clone();

        let origin_quat = quat_from_rpy(origin_rpy);
        let origin_translation = glam::Vec3::new(
            origin_xyz[0] as f32,
            origin_xyz[1] as f32,
            origin_xyz[2] as f32,
        );

        match joint_type {
            JointType::Revolute | JointType::Continuous => {
                let mut warning = None;
                let mut value = value;

                // Check limits only for revolute (continuous has no limits).
                if clamp
                    && *joint_type == JointType::Revolute
                    && !(limit.lower <= value && value <= limit.upper)
                {
                    warning = Some(format!(
                        "Joint '{}' angle {:.4} rad is outside limits [{:.4}, {:.4}] rad. Clamping.",
                        name, value, limit.lower, limit.upper
                    ));
                    value = value.clamp(limit.lower, limit.upper);
                }

                // Combine origin rotation with dynamic rotation (axis-angle).
                let axis_vec =
                    glam::Vec3::new(axis.xyz[0] as f32, axis.xyz[1] as f32, axis.xyz[2] as f32);
                let quat_dynamic = glam::Quat::from_axis_angle(axis_vec, value as f32);
                let combined_quat = origin_quat * quat_dynamic;

                Ok(JointTransform {
                    quaternion: combined_quat,
                    translation: origin_translation,
                    parent_frame,
                    child_frame,
                    warning,
                })
            }

            JointType::Prismatic => {
                let mut warning = None;
                let mut value = value;

                if clamp && !(limit.lower <= value && value <= limit.upper) {
                    warning = Some(format!(
                        "Joint '{}' distance {:.4} m is outside limits [{:.4}, {:.4}] m. Clamping.",
                        name, value, limit.lower, limit.upper
                    ));
                    value = value.clamp(limit.lower, limit.upper);
                }

                // For a prismatic joint, we translate along the axis by `value` and use the origin rotation.
                let translation = glam::Vec3::new(
                    (origin_xyz[0] + axis.xyz[0] * value) as f32,
                    (origin_xyz[1] + axis.xyz[1] * value) as f32,
                    (origin_xyz[2] + axis.xyz[2] * value) as f32,
                );

                Ok(JointTransform {
                    quaternion: origin_quat,
                    translation,
                    parent_frame,
                    child_frame,
                    warning,
                })
            }

            // Fixed joint: just use the origin transform.
            JointType::Fixed => Ok(JointTransform {
                quaternion: origin_quat,
                translation: origin_translation,
                parent_frame,
                child_frame,
                warning: None,
            }),

            JointType::Floating | JointType::Planar | JointType::Spherical => {
                Err(Error::UnsupportedJointType(joint_type.clone()))
            }
        }
    }
}
