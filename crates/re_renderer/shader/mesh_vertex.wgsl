// See mesh.rs#MeshVertex
struct VertexIn {
    @location(0) position: Vec3,
    @location(1) normal: Vec3,
    @location(2) texcoord: Vec2,
};

// See mesh_renderer.rs
struct InstanceIn {
    @location(3) position_and_scale: Vec4,
    @location(4) rotation: Vec4,
};
