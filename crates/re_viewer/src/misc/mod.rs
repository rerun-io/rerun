mod mesh_cache;
pub(crate) mod mesh_loader;
pub mod queries;
pub(crate) mod space_info;
mod space_view_highlights;
mod transform_cache;

pub mod instance_hash_conversions;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod profiler;

pub use mesh_cache::MeshCache;
pub use transform_cache::{TransformCache, UnreachableTransform};

pub use space_view_highlights::{
    highlights_for_space_view, OptionalSpaceViewEntityHighlight, SpaceViewHighlights,
    SpaceViewOutlineMasks,
};
