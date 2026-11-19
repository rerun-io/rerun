---
title: Format-independent kinematics API for URDF
hidden: true
type: misc
---

### Format-independent kinematics API for URDF

`re_importer::UrdfTree` now implements the new `re_importer::KinematicTree` trait, which returns its transforms as `re_importer::JointTransform`.
Existing code keeps working, with these deprecations:

- In Rust, `re_importer::urdf_joint_transform::compute_transform3d` is deprecated in favor of `urdf_joint_transform::compute_joint_transform` (convert the result with `Transform3D::from`), or `UrdfTree::compute_joint_transform` to apply the frame prefix.
  `urdf_joint_transform::Error` is now a deprecated alias of `re_importer::UnsupportedJointType`, and `urdf_joint_transform::internal::JointTransform` of `re_importer::JointTransform` (`warning` is now `warnings`).
- In Python, `rerun.urdf.UrdfTree.log_urdf_to_recording` is deprecated in favor of `log_to_recording`.

`compute_joint_transform_batches` now logs clamping warnings once per call instead of once per clamped value.
The single log line contains the first warning and the number of further warnings in the batch.
