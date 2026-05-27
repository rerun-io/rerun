use re_renderer::renderer::{MeshDrawData, VoxelGridDrawData};

#[test]
fn voxel_grid_draw_data_uses_compact_instances() {
    assert_eq!(VoxelGridDrawData::vertices_per_voxel(), 36);
    assert_eq!(VoxelGridDrawData::gpu_instance_size_bytes(), 24);
    assert!(
        VoxelGridDrawData::gpu_instance_size_bytes() * 4 < MeshDrawData::gpu_instance_size_bytes()
    );
}
