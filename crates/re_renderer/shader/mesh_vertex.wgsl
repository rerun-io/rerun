// See mesh.rs#MeshVertex
struct VertexIn {
    @location(0) position: Vec3,
    @location(1) color: Vec4, // gamma-space 0-1, unmultiplied
    @location(2) normal: Vec3,
    @location(3) texcoord: Vec2,
};

// See mesh_renderer.rs
struct InstanceIn {
    // We could alternatively store projection_from_mesh, but world position might be useful
    // in the future and this saves us a Vec4 and simplifies dataflow on the cpu side.
    @location(4) world_from_mesh_row_0: Vec4,
    @location(5) world_from_mesh_row_1: Vec4,
    @location(6) world_from_mesh_row_2: Vec4,
    @location(7) world_from_mesh_normal_row_0: Vec3,
    @location(8) world_from_mesh_normal_row_1: Vec3,
    @location(9) world_from_mesh_normal_row_2: Vec3,
    @location(10) additive_tint_srgb: Vec4,
    @location(11) outline_mask_ids: UVec2,
    @location(12) picking_layer_id: UVec4,
};
