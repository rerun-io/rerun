// See mesh.rs#MeshVertex and mesh_renderer.rs pipeline creation
struct VertexIn {
    @location(0) position: Vec3,
    @location(1) normal: Vec3,
    @location(2) texcoord: Vec2,
};
