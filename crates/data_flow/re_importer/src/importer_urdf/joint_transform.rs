//! Utilities for computing joint transforms with an URDF.

use re_sdk_types::archetypes::Transform3D;
use re_sdk_types::external::glam;
use urdf_rs::{Joint, JointType};

use super::{quat_from_rpy, unit_joint_axis};
use crate::{JointTransform, MultiDofJointType, UnsupportedJointType};

/// The error returned by [`compute_transform3d`].
#[deprecated(
    since = "0.44.0",
    note = "Use `re_importer::UnsupportedJointType` instead"
)]
pub type Error = UnsupportedJointType;

/// Computes a [`Transform3D`] for a joint at the given value, and logs any warnings.
///
/// See [`compute_joint_transform`] for the meaning of `value` and `clamp`.
#[deprecated(
    since = "0.44.0",
    note = "Use `compute_joint_transform` and convert the result with `Transform3D::from`"
)]
pub fn compute_transform3d(
    joint: &Joint,
    value: f64,
    clamp: bool,
) -> Result<Transform3D, UnsupportedJointType> {
    let transform = compute_joint_transform(joint, value, clamp)?;
    for warning in &transform.warnings {
        re_log::warn!("{warning}");
    }
    Ok(transform.into())
}

/// Deprecated paths to [`compute_joint_transform`] and [`JointTransform`].
pub mod internal {
    pub use super::compute_joint_transform;

    #[deprecated(since = "0.44.0", note = "Use `re_importer::JointTransform` instead")]
    pub type JointTransform = crate::JointTransform;
}

/// Computes the transform from the joint's parent link to its child link, for the joint at `value`.
///
/// `value` is either an angle in radians (revolute/continuous joint)
/// or a distance in meters (prismatic joint).
/// The frames are the plain link names, without any frame prefix.
///
/// If `clamp` is true, values outside joint limits are clamped and a warning is added.
/// If `clamp` is false, values outside limits are used as-is without warnings.
///
/// The joint axis does not have to be a unit vector.
/// A joint whose axis is zero-length or not finite does not move.
pub fn compute_joint_transform(
    joint: &Joint,
    value: f64,
    clamp: bool,
) -> Result<JointTransform, UnsupportedJointType> {
    let Joint {
        name,
        joint_type,
        origin,
        parent,
        child,
        axis: _,
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

    let origin_quat = quat_from_rpy(origin_rpy);
    let origin_translation = glam::Vec3::new(
        origin_xyz[0] as f32,
        origin_xyz[1] as f32,
        origin_xyz[2] as f32,
    );

    let axis = unit_joint_axis(joint).unwrap_or(glam::Vec3::ZERO);
    let mut warnings = Vec::new();
    let mut clamped = |quantity: &str, unit: &str| {
        if clamp && !(limit.lower <= value && value <= limit.upper) {
            warnings.push(format!(
                "Joint '{name}' {quantity} {value:.4} {unit} is outside limits [{:.4}, {:.4}] {unit}. Clamping.",
                limit.lower, limit.upper
            ));
            value.clamp(limit.lower, limit.upper)
        } else {
            value
        }
    };

    let unsupported = |joint_type| UnsupportedJointType {
        joint: name.clone(),
        joint_type,
    };

    let (translation, rotation) = match joint_type {
        // Continuous joints have no limits.
        JointType::Continuous => (
            origin_translation,
            origin_quat * glam::Quat::from_scaled_axis(axis * value as f32),
        ),
        JointType::Revolute => {
            let value = clamped("angle", "rad");
            (
                origin_translation,
                origin_quat * glam::Quat::from_scaled_axis(axis * value as f32),
            )
        }

        // The axis is expressed in the joint frame, so the slide has to be rotated by the
        // origin before it joins the parent-frame translation.
        JointType::Prismatic => {
            let value = clamped("distance", "m");
            (
                origin_translation + origin_quat * (axis * value as f32),
                origin_quat,
            )
        }

        JointType::Fixed => (origin_translation, origin_quat),

        JointType::Floating => return Err(unsupported(MultiDofJointType::Floating)),
        JointType::Planar => return Err(unsupported(MultiDofJointType::Planar)),
        JointType::Spherical => return Err(unsupported(MultiDofJointType::Spherical)),
    };

    Ok(JointTransform {
        translation,
        quaternion: rotation,
        parent_frame: parent.link.clone(),
        child_frame: child.link.clone(),
        warnings,
    })
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
            compute_joint_transform(joint, 0.04, false)
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
        let transform = compute_joint_transform(joint, 0.5, false).expect("revolute is supported");
        assert!((transform.translation - glam::Vec3::new(0.088, 0.0, 0.0)).length() < 1e-6);
    }

    /// Non-unit axes, which are common in URDF files, act like their unit-length direction.
    #[test]
    fn test_non_unit_axis_is_normalized() {
        const URDF: &str = r#"
            <robot name="arm">
              <link name="a"/><link name="b"/><link name="c"/>
              <joint name="hinge" type="continuous">
                <parent link="a"/><child link="b"/><axis xyz="0 0 0.1"/>
              </joint>
              <joint name="slide" type="prismatic">
                <parent link="b"/><child link="c"/><axis xyz="0 2 2"/>
                <limit lower="0" upper="1" effort="1" velocity="1"/>
              </joint>
            </robot>"#;

        let robot = urdf_rs::read_from_string(URDF).expect("the test URDF should parse");

        for angle in [0.5, -0.5] {
            let hinge = compute_joint_transform(&robot.joints[0], angle, false).expect("supported");
            let expected = glam::Quat::from_rotation_z(angle as f32);
            assert!(
                hinge.quaternion.angle_between(expected) < 1e-6,
                "hinge rotation at {angle}: {:?}",
                hinge.quaternion
            );
        }

        let slide = compute_joint_transform(&robot.joints[1], 1.0, false).expect("supported");
        let expected = glam::Vec3::new(0.0, 1.0, 1.0) / 2.0_f32.sqrt();
        assert!(
            (slide.translation - expected).length() < 1e-6,
            "slide translation {:?}",
            slide.translation
        );
    }
}
