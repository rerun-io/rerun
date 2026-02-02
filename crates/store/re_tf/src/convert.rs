//! Conversion functions for transform components to double precision types.
//!
//! These conversions are used internally by `re_tf` for transform computations until
//! we have proper data type generics. We put them here to make future generic refactoring
//! easier.

use glam::{DAffine3, DMat3, DQuat, DVec3};
use re_sdk_types::{components, datatypes};

// ---------------------------------------------------------------------------
// Helper functions for datatypes

#[inline]
#[expect(clippy::result_unit_err)]
pub fn quaternion_to_dquat(q: datatypes::Quaternion) -> Result<DQuat, ()> {
    let q = q.0;
    glam::DVec4::new(q[0] as f64, q[1] as f64, q[2] as f64, q[3] as f64)
        .try_normalize()
        .map(DQuat::from_vec4)
        .ok_or(())
}

#[inline]
pub fn vec3d_to_dvec3(v: datatypes::Vec3D) -> DVec3 {
    let v = v.0;
    DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64)
}

// ---------------------------------------------------------------------------
// Component conversion functions

#[inline]
pub fn translation_3d_to_daffine3(v: components::Translation3D) -> DAffine3 {
    DAffine3 {
        matrix3: DMat3::IDENTITY,
        translation: vec3d_to_dvec3(v.0),
    }
}

#[inline]
#[expect(clippy::result_unit_err)]
pub fn rotation_axis_angle_to_daffine3(val: components::RotationAxisAngle) -> Result<DAffine3, ()> {
    vec3d_to_dvec3(val.0.axis)
        .try_normalize()
        .map(|normalized| DAffine3::from_axis_angle(normalized, val.0.angle.radians() as f64))
        .ok_or(())
}

#[inline]
#[expect(clippy::result_unit_err)]
pub fn rotation_quat_to_daffine3(val: components::RotationQuat) -> Result<DAffine3, ()> {
    Ok(DAffine3::from_quat(quaternion_to_dquat(val.0)?))
}

#[inline]
pub fn scale_3d_to_daffine3(v: components::Scale3D) -> DAffine3 {
    DAffine3 {
        matrix3: DMat3::from_diagonal(vec3d_to_dvec3(v.0)),
        translation: DVec3::ZERO,
    }
}

#[inline]
pub fn transform_mat3x3_to_daffine3(v: components::TransformMat3x3) -> DAffine3 {
    DAffine3 {
        matrix3: DMat3::from_cols_array(&v.0.0.map(|x| x as f64)),
        translation: DVec3::ZERO,
    }
}
