// See mesh.rs#MeshVertex
struct VertexIn {
    @location(0) position: Vec3,
    @location(1) normal: Vec3,
    @location(2) texcoord: Vec2,
};

// See mesh_renderer.rs
struct InstanceIn {
    // We could alternatively store projection_from_mesh, but world position might be useful
    // in the future and this saves us a Vec4 and simplifies dataflow on the cpu side.
    @location(3) world_from_mesh_row_0: Vec4,
    @location(4) world_from_mesh_row_1: Vec4,
    @location(5) world_from_mesh_row_2: Vec4,
    @location(6) world_from_mesh_normal_row_0: Vec3,
    @location(7) world_from_mesh_normal_row_1: Vec3,
    @location(8) world_from_mesh_normal_row_2: Vec3,
    @location(9) additive_tint_srgb: Vec4,
};
