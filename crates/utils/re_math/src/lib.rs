//! An opinionated 3D math library built on [`glam`](https://github.com/bitshifter/glam-rs).
//!
//! `re_math` was originally based on [macaw](https://crates.io/crates/macaw).

mod bounding_box;
mod conformal;
mod float_ext;
mod iso_transform;
mod mesh_gen;
mod plane3;
mod quat_ext;
mod ray3;
mod utils;
mod vec2_ext;
mod vec3_ext;
mod vec4_ext;

pub use self::bounding_box::*;
pub use self::conformal::*;
pub use self::float_ext::*;
pub use self::iso_transform::*;
pub use self::plane3::*;
pub use self::ray3::*;
pub use self::utils::*;
pub use self::vec2_ext::Vec2Ext;
pub use self::vec3_ext::Vec3Ext;
pub use self::vec4_ext::Vec4Ext;

pub use mesh_gen::*;
pub use quat_ext::*;

/// Prelude module with extension traits
pub mod prelude {
    pub use crate::FloatExt;
    pub use crate::Vec2Ext;
    pub use crate::Vec2Swizzles;
    pub use crate::Vec3Ext;
    pub use crate::Vec3Swizzles;
    pub use crate::Vec4Ext;
    pub use crate::Vec4Swizzles;

    pub use crate::QuatExt;
}

// Re-export main glam types.
// i32
pub use glam::ivec2;
pub use glam::ivec3;
pub use glam::IVec2;
pub use glam::IVec3;
pub use glam::IVec4;
// u32
pub use glam::uvec2;
pub use glam::uvec3;
pub use glam::uvec4;
pub use glam::UVec2;
pub use glam::UVec3;
pub use glam::UVec4;
// f32
pub use glam::mat2;
pub use glam::mat3;
pub use glam::mat3a;
pub use glam::mat4;
pub use glam::quat;
pub use glam::vec2;
pub use glam::vec3;
pub use glam::vec3a;
pub use glam::vec4;
pub use glam::Affine3A;
pub use glam::Mat2;
pub use glam::Mat3;
pub use glam::Mat3A;
pub use glam::Mat4;
pub use glam::Quat;
pub use glam::Vec2;
pub use glam::Vec3;
pub use glam::Vec3A;
pub use glam::Vec4;
// f64
pub use glam::dmat2;
pub use glam::dmat3;
pub use glam::dmat4;
pub use glam::dquat;
pub use glam::dvec2;
pub use glam::dvec3;
pub use glam::dvec4;
pub use glam::DAffine2;
pub use glam::DAffine3;
pub use glam::DMat2;
pub use glam::DMat3;
pub use glam::DMat4;
pub use glam::DQuat;
pub use glam::DVec2;
pub use glam::DVec3;
pub use glam::DVec4;
// other
pub use glam::EulerRot;
pub use glam::Vec2Swizzles;
pub use glam::Vec3Swizzles;
pub use glam::Vec4Swizzles;
