//! Computes the motion of single MJCF joints, the way MuJoCo's forward kinematics does.
//!
//! MuJoCo applies a body's joints in order, each in the body frame as moved by the joints before it.
//! A body's pose relative to its parent is therefore `reference · J₁(q₁) · … · Jₙ(qₙ)`, where each `Jᵢ`
//! only depends on its own joint value, so that every joint can have a transform of its own.

use mjcf_rs::Pose;
use re_sdk_types::external::glam::DQuat;

use super::model::{MjcfJoint, MjcfJointType};
use crate::{MultiDofJointType, UnsupportedJointType};

/// Computes the motion of `joint` at `value` (`qpos` in MuJoCo terms), which is the identity at the joint's reference value.
///
/// The motion goes from the frame before the joint to the frame after it.
/// If `clamp` is true, values outside a limited joint's range are clamped and a warning is returned.
pub fn compute_joint_motion(
    joint: &MjcfJoint,
    value: f64,
    clamp: bool,
) -> Result<(Pose, Vec<String>), UnsupportedJointType> {
    let unsupported = |joint_type| UnsupportedJointType {
        joint: joint.name.clone(),
        joint_type,
    };

    let (quantity, unit) = match joint.joint_type {
        MjcfJointType::Hinge => ("angle", "rad"),
        MjcfJointType::Slide => ("distance", "m"),
        MjcfJointType::Ball => return Err(unsupported(MultiDofJointType::Spherical)),
        MjcfJointType::Free => return Err(unsupported(MultiDofJointType::Floating)),
    };

    let mut value = value;
    let mut warnings = Vec::new();
    if clamp
        && let Some(range) = &joint.range
        && !range.contains(&value)
    {
        let (lower, upper) = (*range.start(), *range.end());
        warnings.push(format!(
            "Joint '{}' {quantity} {value:.4} {unit} is outside limits [{lower:.4}, {upper:.4}] {unit}. Clamping.",
            joint.name
        ));
        value = value.clamp(lower, upper);
    }

    let offset = value - joint.reference;
    let pose = match joint.joint_type {
        MjcfJointType::Hinge => {
            let rotation = DQuat::from_axis_angle(joint.axis, offset);
            Pose::from_parts(joint.anchor - rotation * joint.anchor, rotation)
        }
        MjcfJointType::Slide => Pose::from_translation(joint.axis * offset),
        MjcfJointType::Ball | MjcfJointType::Free => Pose::IDENTITY,
    };

    Ok((pose, warnings))
}
