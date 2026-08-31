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

                // The axis is expressed in the joint frame, so the slide has to be rotated by the
                // origin before it joins the parent-frame translation.
                let axis_vec =
                    glam::Vec3::new(axis.xyz[0] as f32, axis.xyz[1] as f32, axis.xyz[2] as f32);
                let translation = origin_translation + origin_quat * (axis_vec * value as f32);

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests that the origin rotation of prismatic joint axes is correctly applied.
    #[test]
    fn test_prismatic_axis_rotation() {
        // A parallel gripper with two `y+` prismatic axes pointing in opposite directions w.r.t. the parent link.
        const URDF: &str = r#"
            <robot name="gripper">
              <link name="hand"/><link name="left"/><link name="right"/>
              <joint name="left_joint" type="prismatic">
                <parent link="hand"/><child link="left"/>
                <origin xyz="0 0 0.0584" rpy="0 0 0"/><axis xyz="0 1 0"/>
                <limit lower="0" upper="0.04" effort="100" velocity="0.2"/>
              </joint>
              <joint name="right_joint" type="prismatic">
                <parent link="hand"/><child link="right"/>
                <origin xyz="0 0 0.0584" rpy="0 0 3.141592653589793"/><axis xyz="0 1 0"/>
                <limit lower="0" upper="0.04" effort="100" velocity="0.2"/>
              </joint>
            </robot>"#;

        let robot = urdf_rs::read_from_string(URDF).expect("the test URDF should parse");
        let slide = |name: &str| {
            let joint = robot
                .joints
                .iter()
                .find(|joint| joint.name == name)
                .expect("the test URDF declares this joint");
            internal::compute_joint_transform(joint, 0.04, false)
                .expect("a prismatic joint is supported")
                .translation
        };

        // We expect the two gripper parts to move in opposite direction w.r.t. the parent.
        let (left, right) = (slide("left_joint"), slide("right_joint"));
        assert!((left.y - 0.04).abs() < 1e-6, "left finger at {left:?}");
        assert!((right.y + 0.04).abs() < 1e-6, "right finger at {right:?}");
    }

    /// A revolute joint with a rotated origin keeps its translation at the origin.
    #[test]
    fn test_revolute_translation_stays_at_the_joint_origin() {
        const URDF: &str = r#"
            <robot name="arm">
              <link name="a"/><link name="b"/>
              <joint name="elbow" type="revolute">
                <parent link="a"/><child link="b"/>
                <origin xyz="0.088 0 0" rpy="1.5707963267948966 0 0"/><axis xyz="0 0 1"/>
                <limit lower="-2.9" upper="2.9" effort="100" velocity="2.0"/>
              </joint>
            </robot>"#;

        let robot = urdf_rs::read_from_string(URDF).expect("the test URDF should parse");
        let joint = &robot.joints[0];
        let transform =
            internal::compute_joint_transform(joint, 0.5, false).expect("revolute is supported");
        assert!((transform.translation - glam::Vec3::new(0.088, 0.0, 0.0)).length() < 1e-6);
    }
}
