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

pub use self::{
    bounding_box::BoundingBox,
    conformal::Conformal3,
    float_ext::FloatExt,
    iso_transform::IsoTransform,
    mesh_gen::MeshGen,
    plane3::Plane3,
    quat_ext::QuatExt,
    ray3::Ray3,
    utils::{lerp, remap, remap_clamp},
    vec2_ext::Vec2Ext,
    vec3_ext::Vec3Ext,
    vec4_ext::Vec4Ext,
};

/// Prelude module with extension traits
pub mod prelude {
    pub use crate::FloatExt;
    pub use crate::QuatExt;
    pub use crate::Vec2Ext;
    pub use crate::Vec3Ext;
    pub use crate::Vec4Ext;

    pub use glam::Vec2Swizzles;
    pub use glam::Vec3Swizzles;
    pub use glam::Vec4Swizzles;
}
