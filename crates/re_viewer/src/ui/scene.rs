use std::sync::Arc;

use ahash::HashSet;
use glam::Vec3;
use re_data_store::{InstanceIdHash, ObjPath, TimeQuery, Timeline};
use re_log_types::Tensor;

use crate::misc::mesh_loader::CpuMesh;
use crate::view3d::scene::Size;

// ---

#[derive(Default)]
pub struct Scene {
    pub two_d: Scene2d,
    pub three_d: Scene3d,
    pub tensors: Vec<Tensor>,
    pub text: SceneText,
}

#[derive(Debug)]
pub struct SceneQuery {
    pub objects: HashSet<ObjPath>,
    pub timeline: Timeline,
    pub time_query: TimeQuery<i64>,
}

// --- 2D ---

#[derive(Default)]
pub struct Scene2d {
    // TODO
}

// --- 3D ---

// TODO: prob want to make some changes to these sub-types though.

pub struct Point {
    pub instance_id: InstanceIdHash,
    pub pos: [f32; 3],
    pub radius: Size,
    pub color: [u8; 4],
}

pub struct LineSegments {
    pub instance_id: InstanceIdHash,
    pub segments: Vec<[[f32; 3]; 2]>,
    pub radius: Size,
    pub color: [u8; 4],
}

#[cfg(feature = "glow")]
pub enum MeshSourceData {
    Mesh3D(re_log_types::Mesh3D),
    /// e.g. the camera mesh
    StaticGlb(&'static [u8]),
}

pub struct MeshSource {
    pub instance_id: InstanceIdHash,
    pub mesh_id: u64,
    pub world_from_mesh: glam::Affine3A,
    pub cpu_mesh: Arc<CpuMesh>,
    pub tint: Option<[u8; 4]>,
}

pub struct Label {
    pub(crate) text: String,
    /// Origin of the label
    pub(crate) origin: Vec3,
}

#[derive(Default)]
pub struct Scene3d {
    pub points: Vec<Point>,
    pub line_segments: Vec<LineSegments>,
    pub meshes: Vec<MeshSource>,
    pub labels: Vec<Label>,
}

// --- Text logs ---

#[derive(Default)]
pub struct SceneText {
    // TODO
}
